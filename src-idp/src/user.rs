use std::sync::Arc;

use argon2::{PasswordHasher, PasswordVerifier};
use idp_proto::{
    app::Membership,
    auth::{AccessLevel, Authentication, AuthenticationStrict},
    invite::InsecurePassword,
    user::*,
};
use snafu::prelude::*;
use sqlx::{PgPool, types::Json};
use transit_core::{InternalError, InternalErrorContext, TransitErrorContext};

use crate::{app, auth, common, token};

pub(crate) fn ensure_password_strength(
    password: &str,
    inputs: &[&str],
) -> Result<(), InsecurePassword> {
    let entropy = zxcvbn::zxcvbn(password, inputs);

    if entropy.score() >= zxcvbn::Score::Three {
        return Ok(());
    }

    let feedback = entropy.feedback();

    Err(InsecurePassword {
        warning: feedback
            .and_then(|f| f.warning())
            .map(|w| w.to_string())
            .unwrap_or_default(),
        suggestions: feedback
            .map(|f| f.suggestions().iter().map(ToString::to_string).collect())
            .unwrap_or_default(),
    })
}

pub(crate) fn hash_password(pass: &str) -> Result<String, InternalError> {
    common::argon2()
        .hash_password(pass.as_bytes())
        .context(InternalErrorContext!(via(display), common::ERROR_GENERIC))
        .map(|it| it.to_string())
}

pub(crate) fn verify_password(hash: &str, password: &str) -> Result<(), IncorrectPassword> {
    common::argon2()
        .verify_password(password.as_bytes(), hash)
        .context(TransitErrorContext!(IncorrectPassword))
}

pub(crate) async fn check_password(
    CheckPasswordRequest {
        unknown_identity: query_unknown,
        password,
    }: CheckPasswordRequest,
    pg: Arc<PgPool>,
) -> Result<(), CheckPasswordError> {
    let hash = sqlx::query_scalar!(
        r#"
        SELECT password_hash FROM users
        WHERE
            id = $1 OR
            username = $1 OR
            email = $1
        "#,
        query_unknown
    )
    .fetch_optional(&*pg)
    .await
    .context(InternalErrorContext!(via(display), common::ERROR_DB))?
    .context(NoUser)?;

    verify_password(&hash, &password)?;

    Ok(())
}

#[tracing::instrument(skip(pg))]
pub(crate) async fn list(
    auth: Authentication,
    pg: Arc<PgPool>,
) -> Result<Vec<User>, UserListError> {
    auth::authenticate(auth, AccessLevel::Read, &pg).await??;

    let users = sqlx::query!(
        r#"
        SELECT
            u.id,
            u.email,
            u.username,
            COALESCE(
                JSONB_AGG(
                    JSONB_BUILD_OBJECT(
                        'slug', r.application,
                        'admin', r.admin
                    )
                ),
                '[]'::jsonb
            ) AS "roles!: sqlx::types::Json<Vec<Membership>>"
        FROM users u
        LEFT JOIN roles r ON r.uid = u.id
        GROUP BY u.id;
        "#,
    )
    .fetch_all(&*pg)
    .await
    .context(InternalErrorContext!(via(display), common::ERROR_DB))?
    .into_iter()
    .map(|res| User {
        id: res.id,
        username: res.username,
        email: res.email,
        memberships: res.roles.0,
    })
    .collect();

    Ok(users)
}

#[tracing::instrument(skip(pg, auth))]
pub(crate) async fn update(
    UserUpdateRequest {
        auth,
        id,
        username,
        email,
        memberships,
    }: UserUpdateRequest,
    pg: Arc<PgPool>,
) -> Result<User, UserUpdateError> {
    let wants_update_self = if let Authentication::Token(token) = &auth {
        let identity = token::parse(token)??;

        id == *identity.uid
    } else {
        false
    };

    if !wants_update_self {
        auth::authenticate(auth, AccessLevel::ReadWrite, &pg).await??;
    }

    snafu::ensure!(
        username
            .as_ref()
            .is_none_or(|u| common::USERNAME_RE.is_match(u)),
        InvalidUsername
    );
    snafu::ensure!(
        email.as_ref().is_none_or(|e| common::EMAIL_RE.is_match(e)),
        InvalidEmail
    );

    let split_roles = memberships.as_ref().map(app::split_roles);

    if let Some((slugs, _)) = &split_roles {
        app::verify(slugs.as_slice(), &pg).await??;
    }

    let mut tx = pg
        .begin()
        .await
        .context(InternalErrorContext!(via(display), common::ERROR_DB))?;

    let user_update = match sqlx::query!(
        r#"
        UPDATE users
        SET
            username = COALESCE($2, username),
            email = COALESCE($3, email),
            updated_at = now()
        WHERE
            id = $1
        RETURNING username, email
        "#,
        id,
        username,
        email
    )
    .fetch_optional(&mut *tx)
    .await
    {
        Ok(Some(res)) => res,
        Ok(None) => {
            return Err(NoUser.into());
        }
        Err(sqlx::Error::Database(db)) if db.is_unique_violation() => match db.constraint() {
            Some("users_username_key") => return Err(UsernameInUse.into()),
            _ => return Err(EmailInUse.into()),
        },
        Err(other) => {
            return Err(InternalError!(ctx(display other), common::ERROR_DB).into());
        }
    };

    if let Some((role_slugs, role_admins)) = split_roles {
        sqlx::query!(
            r#"
            DELETE FROM roles
            WHERE uid = $1
            "#,
            id
        )
        .execute(&mut *tx)
        .await
        .context(InternalErrorContext!(via(display), common::ERROR_DB))?;

        sqlx::query!(
            r#"
            INSERT INTO roles (uid, application, admin)
            SELECT $1, app, admin
            FROM UNNEST($2::text[], $3::boolean[]) AS t(app, admin)
            "#,
            id,
            role_slugs.as_slice(),
            role_admins.as_slice()
        )
        .execute(&mut *tx)
        .await
        .context(InternalErrorContext!(via(display), common::ERROR_DB))?;
    }

    let memberships = match memberships {
        Some(m) => m,
        None => {
            sqlx::query_scalar!(
                r#"
                SELECT
                    COALESCE(
                        JSONB_AGG(
                            JSONB_BUILD_OBJECT('slug', r.application, 'admin', r.admin)
                        ),
                        '[]'::jsonb
                    ) AS "roles!: Json<Vec<Membership>>"
                FROM users u
                LEFT JOIN roles r ON r.uid = u.id
                WHERE u.id = $1
                "#,
                id
            )
            .fetch_one(&mut *tx)
            .await
            .context(InternalErrorContext!(via(display), common::ERROR_DB))?
            .0
        }
    };

    tx.commit()
        .await
        .context(InternalErrorContext!(via(display), common::ERROR_DB))?;

    Ok(User {
        id,
        username: user_update.username,
        email: user_update.email,
        memberships,
    })
}

#[tracing::instrument(skip_all)]
pub(crate) async fn update_password(
    UpdatePasswordRequest { auth, id, password }: UpdatePasswordRequest,
    pg: Arc<PgPool>,
) -> Result<(), UpdatePasswordError> {
    auth::authenticate_strict(auth, AccessLevel::ReadWrite, &pg).await??;

    let user = sqlx::query!(r#"SELECT username, email FROM users WHERE id = $1"#, id)
        .fetch_optional(&*pg)
        .await
        .context(InternalErrorContext!(via(display), common::ERROR_DB))?
        .context(NoUser)?;

    let hash = hash_password(&password)?;

    ensure_password_strength(&password, &[&user.username, &user.email])?;

    let res = sqlx::query!(
        r#"
        UPDATE users
        SET password_hash = $2, updated_at = now()
        WHERE id = $1
        "#,
        id,
        hash,
    )
    .execute(&*pg)
    .await
    .context(InternalErrorContext!(via(display), common::ERROR_DB))?;

    snafu::ensure!(res.rows_affected() > 0, NoUser);

    Ok(())
}

#[tracing::instrument(skip_all)]
pub(crate) async fn delete(req: UserDeleteRequest, pg: Arc<PgPool>) -> Result<(), UserDeleteError> {
    let uid = match req {
        UserDeleteRequest::DeleteSelf { uid, password } => {
            auth::authenticate_strict(
                AuthenticationStrict::Password {
                    uid: uid.clone(),
                    password,
                },
                AccessLevel::ReadWrite,
                &pg,
            )
            .await??;

            uid
        }
        UserDeleteRequest::DeleteOther { auth, uid } => {
            auth::authenticate(auth, AccessLevel::ReadWrite, &pg).await??;

            uid
        }
    };

    let res = sqlx::query!(
        r#"
        DELETE FROM users
        WHERE id = $1
        "#,
        uid,
    )
    .execute(&*pg)
    .await
    .context(InternalErrorContext!(via(display), common::ERROR_DB))?;

    if res.rows_affected() == 0 {
        return Err(NoUser.into());
    }

    Ok(())
}
