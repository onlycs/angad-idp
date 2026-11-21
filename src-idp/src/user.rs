use std::sync::Arc;

use argon2::{PasswordHasher, PasswordVerifier};
use idp_proto::{
    app::Membership,
    auth::{AccessLevel, AuthenticateSelfStrict, Authentication},
    invite::InsecurePassword,
    user::*,
};
use snafu::prelude::*;
use sqlx::PgPool;
use transit_core::{InternalError, InternalErrorContext, TransitErrorContext};

use crate::{
    app::{self, VerifyErrorCast},
    auth::{self, AuthErrorCast},
    common,
};

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

#[tracing::instrument(ret, err, skip(auth, pg))]
pub(crate) async fn list(
    auth: Authentication,
    pg: Arc<PgPool>,
) -> Result<Vec<User>, UserListError> {
    auth::authenticate(auth, AccessLevel::Read, &pg)
        .await
        .context(AuthErrorCast::<UserListError>)?;

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

#[tracing::instrument(ret, err, skip(req, pg))]
pub(crate) async fn update(req: UserUpdateRequest, pg: Arc<PgPool>) -> Result<(), UserUpdateError> {
    match req {
        UserUpdateRequest::UpdateOther {
            auth,
            uid,
            mut memberships,
        } => {
            let user_token = auth::authenticate(auth, AccessLevel::ReadWrite, &pg)
                .await
                .context(AuthErrorCast::<UserUpdateError>)?;

            snafu::ensure!(
                user_token.is_none() || memberships.iter().any(Membership::is_domain_admin),
                RemoveOwnDomainAdmin
            );

            let (slugs, admins) = app::split_and_fix(&mut memberships);

            app::verify(slugs.as_slice(), &pg)
                .await
                .context(VerifyErrorCast::<UserUpdateError>)?;

            let mut tx = pg
                .begin()
                .await
                .context(InternalErrorContext!(via(display), common::ERROR_DB))?;

            match sqlx::query!(
                r#"
                UPDATE users
                SET updated_at = now()
                WHERE id = $1
                "#,
                uid,
            )
            .execute(&mut *tx)
            .await
            {
                Ok(res) if res.rows_affected() > 0 => {}
                Ok(_) => return Err(NoUser.into()),
                Err(sqlx::Error::Database(db)) if db.is_unique_violation() => match db.constraint()
                {
                    Some("users_username_key") => return Err(UsernameInUse.into()),
                    _ => return Err(EmailInUse.into()),
                },
                Err(other) => {
                    return Err(InternalError!(ctx(display other), common::ERROR_DB).into());
                }
            };

            sqlx::query!(
                r#"
                DELETE FROM roles
                WHERE uid = $1
                "#,
                uid
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
                uid,
                slugs.as_slice(),
                admins.as_slice()
            )
            .execute(&mut *tx)
            .await
            .context(InternalErrorContext!(via(display), common::ERROR_DB))?;

            tx.commit()
                .await
                .context(InternalErrorContext!(via(display), common::ERROR_DB))?;

            Ok(())
        }
        UserUpdateRequest::UpdateSelf {
            auth,
            username,
            email,
            password,
        } => {
            auth::authenticate_strict(&auth, &pg)
                .await
                .context(AuthErrorCast::<UserUpdateError>)?;

            let AuthenticateSelfStrict { uid, .. } = auth;

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

            let mut update_password = None;

            if let Some(pw) = password {
                ensure_password_strength(&pw, &[])?;
                update_password = Some(hash_password(&pw)?)
            }

            match sqlx::query!(
                r#"
                UPDATE users
                SET
                    username = COALESCE($2, username),
                    email = COALESCE($3, email),
                    password_hash = COALESCE($4, password_hash),
                    updated_at = now()
                WHERE
                    id = $1
                "#,
                uid,
                username,
                email,
                update_password
            )
            .execute(&*pg)
            .await
            {
                Ok(res) if res.rows_affected() > 0 => Ok(()),
                Ok(_) => Err(NoUser.into()),
                Err(sqlx::Error::Database(db)) if db.is_unique_violation() => match db.constraint()
                {
                    Some("users_username_key") => Err(UsernameInUse.into()),
                    _ => Err(EmailInUse.into()),
                },
                Err(other) => Err(InternalError!(ctx(display other), common::ERROR_DB).into()),
            }
        }
    }
}

#[tracing::instrument(ret, err, skip_all)]
pub(crate) async fn delete(req: UserDeleteRequest, pg: Arc<PgPool>) -> Result<(), UserDeleteError> {
    let uid = match req {
        UserDeleteRequest::DeleteSelf { auth } => {
            auth::authenticate_strict(&auth, &pg)
                .await
                .context(AuthErrorCast::<UserDeleteError>)?;

            auth.uid
        }
        UserDeleteRequest::DeleteOther { auth, uid } => {
            auth::authenticate(auth, AccessLevel::ReadWrite, &pg)
                .await
                .context(AuthErrorCast::<UserDeleteError>)?;

            uid
        }
    };

    let mut tx = pg
        .begin()
        .await
        .context(InternalErrorContext!(via(display), common::ERROR_DB))?;

    sqlx::query("SET LOCAL lock_timeout = '5s'")
        .execute(&mut *tx)
        .await
        .context(InternalErrorContext!(via(display), common::ERROR_DB))?;

    sqlx::query("SELECT pg_advisory_xact_lock(42, hashtext($1))")
        .bind(app::DOMAIN_SLUG)
        .execute(&mut *tx)
        .await
        .context(InternalErrorContext!(via(display), common::ERROR_DB))?;

    let res = sqlx::query_scalar!(
        r#"
        SELECT r.uid FROM roles r
        WHERE
            r.application = $1 AND
            r.admin
        "#,
        app::DOMAIN_SLUG
    )
    .fetch_all(&mut *tx)
    .await
    .context(InternalErrorContext!(via(display), common::ERROR_DB))?;

    snafu::ensure!(!res.contains(&uid) || res.len() != 1, RemoveOwnDomainAdmin);

    let res = sqlx::query!(
        r#"
        DELETE FROM users
        WHERE id = $1
        "#,
        uid,
    )
    .execute(&mut *tx)
    .await
    .context(InternalErrorContext!(via(display), common::ERROR_DB))?;

    if res.rows_affected() == 0 {
        return Err(NoUser.into());
    }

    tx.commit()
        .await
        .context(InternalErrorContext!(via(display), common::ERROR_DB))?;

    Ok(())
}
