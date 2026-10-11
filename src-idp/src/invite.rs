use std::sync::Arc;

use chrono::Utc;
use idp_proto::{
    app::Membership,
    auth::{AccessLevel, Authentication},
    invite::*,
    user::{EmailInUse, InvalidEmail, InvalidUsername, User, UsernameInUse},
};
use lettre::{
    AsyncTransport, Message,
    message::{Mailbox, header::ContentType},
};
use snafu::prelude::*;
use sqlx::{PgConnection, PgPool, types::Json};
use transit_core::{InternalError, InternalErrorContext, TransitErrorContext};

use crate::{
    app, auth,
    common::{self, EMAIL_RE},
    user,
};

fn parse_expiry(exp: u64) -> Result<chrono::prelude::DateTime<Utc>, InvalidExpiry> {
    chrono::DateTime::<Utc>::from_timestamp_millis(exp as i64).context(InvalidExpiry)
}

async fn add_roles(
    key_hash: &[u8],
    role_slugs: &[String],
    role_admins: &[bool],
    conn: &mut PgConnection,
) -> Result<(), InternalError> {
    sqlx::query!(
        r#"
        INSERT INTO invite_roles (invite, application, admin)
        SELECT $1, app, admin
        FROM UNNEST($2::text[], $3::boolean[]) AS t(app, admin)
        "#,
        key_hash,
        role_slugs,
        role_admins
    )
    .execute(conn)
    .await
    .context(InternalErrorContext!(via(display), common::ERROR_DB))?;

    Ok(())
}

pub(crate) async fn create_impl(
    email: String,
    memberships: Vec<Membership>,
    expiry: chrono::DateTime<Utc>,
    pg: &PgPool,
) -> Result<Invite, InviteCreateError> {
    let key = common::generate_secret()?;
    let key_hash = common::sha256(&key);
    let token = hex::encode(key);

    snafu::ensure!(EMAIL_RE.is_match(&email), InvalidEmail);
    snafu::ensure!(expiry > chrono::Utc::now(), InvalidExpiry);

    let (role_slugs, role_admins) = app::split_roles(&memberships);

    let existing = sqlx::query!("SELECT email FROM users WHERE email = $1", email)
        .fetch_optional(pg)
        .await
        .context(InternalErrorContext!(via(display), common::ERROR_DB))?;

    snafu::ensure!(existing.is_none(), EmailInUse);

    app::verify(role_slugs.as_slice(), pg).await??;

    let mut tx = pg
        .begin()
        .await
        .context(InternalErrorContext!(via(display), common::ERROR_DB))?;

    let op_invite_add = sqlx::query!(
        r#"
        INSERT INTO invites (key_hash, email, expiry)
        VALUES ($1, $2, $3)
        ON CONFLICT DO NOTHING
        "#,
        key_hash,
        email,
        expiry
    )
    .execute(&mut *tx)
    .await
    .context(InternalErrorContext!(via(display), common::ERROR_DB))?;

    snafu::ensure!(op_invite_add.rows_affected() > 0, EmailInUse);

    add_roles(&key_hash, &role_slugs, &role_admins, &mut tx).await?;

    tx.commit()
        .await
        .context(InternalErrorContext!(via(display), common::ERROR_DB))?;

    let message = Message::builder()
        .from(Mailbox::new(None, common::SMTP_FROM.clone()))
        .to(Mailbox::new(None, common::parse_email(&email)?))
        .subject("Invitiation to Angad Auth")
        .header(ContentType::TEXT_HTML)
        .body(
            include_str!("../assets/invite-email-template.html").replace(
                "{{invite_url}}",
                format!("{}/invite?token={token}", *common::FRONTEND_URL).as_str(),
            ),
        )
        .context(TransitErrorContext!(EmailSend))?;

    common::SMTP_TRANSPORT
        .send(message)
        .await
        .context(TransitErrorContext!(EmailSend))?;

    Ok(Invite {
        id: hex::encode(&key_hash),
        email,
        memberships,
        exp: expiry.timestamp_millis() as u64,
    })
}

#[tracing::instrument(skip(pg, auth))]
pub(crate) async fn create(
    InviteCreateRequest {
        auth,
        email,
        memberships,
        exp,
    }: InviteCreateRequest,
    pg: Arc<PgPool>,
) -> Result<Invite, InviteCreateError> {
    auth::authenticate(auth, AccessLevel::ReadWrite, &pg).await??;
    create_impl(email, memberships, parse_expiry(exp)?, &pg).await
}

#[tracing::instrument(skip(pg, auth))]
pub(crate) async fn list(
    auth: Authentication,
    pg: Arc<PgPool>,
) -> Result<Vec<Invite>, InviteListError> {
    auth::authenticate(auth, AccessLevel::Read, &pg).await??;

    let invites = sqlx::query!(
        r#"
        SELECT
            i.key_hash,
            i.email,
            i.expiry,
            COALESCE(
                JSONB_AGG(
                    JSONB_BUILD_OBJECT(
                        'slug', r.application,
                        'admin', r.admin
                    )
                ),
                '[]'::jsonb
            ) AS "roles!: sqlx::types::Json<Vec<Membership>>"
        FROM invites i
        LEFT JOIN invite_roles r ON r.invite = i.key_hash
        GROUP BY i.key_hash;
        "#,
    )
    .fetch_all(&*pg)
    .await
    .context(InternalErrorContext!(via(display), common::ERROR_DB))?
    .into_iter()
    .map(|res| Invite {
        id: hex::encode(res.key_hash),
        email: res.email,
        exp: res.expiry.timestamp_millis() as u64,
        memberships: res.roles.0,
    })
    .collect();

    Ok(invites)
}

#[tracing::instrument(skip(pg, auth))]
pub(crate) async fn update(
    InviteUpdateRequest {
        auth,
        id,
        memberships,
        exp,
    }: InviteUpdateRequest,
    pg: Arc<PgPool>,
) -> Result<Invite, InviteUpdateError> {
    auth::authenticate(auth, AccessLevel::ReadWrite, &pg).await??;

    let key_hash = hex::decode(&id).context(TransitErrorContext!(NoInvite))?;
    let exp = exp.map(parse_expiry).transpose()?;
    let split_roles = memberships.as_ref().map(app::split_roles);

    let mut tx = pg
        .begin()
        .await
        .context(InternalErrorContext!(via(display), common::ERROR_DB))?;

    let res = sqlx::query!(
        r#"
        UPDATE invites
        SET expiry = COALESCE($2, expiry)
        WHERE key_hash = $1
        RETURNING *
        "#,
        key_hash,
        exp,
    )
    .fetch_optional(&mut *tx)
    .await
    .context(InternalErrorContext!(via(display), common::ERROR_DB))?
    .context(NoInvite)?;

    if let Some((role_slugs, role_admins)) = split_roles {
        sqlx::query!(
            r#"
            DELETE FROM invite_roles
            WHERE invite = $1
            "#,
            key_hash,
        )
        .execute(&mut *tx)
        .await
        .context(InternalErrorContext!(via(display), common::ERROR_DB))?;

        add_roles(&key_hash, &role_slugs, &role_admins, &mut tx).await?;
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
                FROM invites i
                LEFT JOIN invite_roles r ON r.invite = i.key_hash
                WHERE i.key_hash = $1
                "#,
                key_hash
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

    Ok(Invite {
        id,
        email: res.email,
        memberships,
        exp: res.expiry.timestamp_millis() as u64,
    })
}

#[tracing::instrument(skip(pg, auth))]
pub(crate) async fn delete(
    InviteDeleteRequest { auth, id }: InviteDeleteRequest,
    pg: Arc<PgPool>,
) -> Result<(), InviteDeleteError> {
    auth::authenticate(auth, AccessLevel::ReadWrite, &pg).await??;

    let res = sqlx::query!(
        r#"
        DELETE FROM invites
        WHERE key_hash = $1
        "#,
        hex::decode(id).context(TransitErrorContext!(NoInvite))?,
    )
    .execute(&*pg)
    .await
    .context(InternalErrorContext!(via(display), common::ERROR_DB))?;

    snafu::ensure!(res.rows_affected() > 0, NoInvite);

    Ok(())
}

#[tracing::instrument(skip(token, password))]
pub(crate) async fn redeem(
    InviteUseRequest {
        token,
        username,
        password,
    }: InviteUseRequest,
    pg: Arc<PgPool>,
) -> Result<User, InviteUseError> {
    let mut tx = pg
        .begin()
        .await
        .context(InternalErrorContext!(via(display), common::ERROR_DB))?;

    let key = hex::decode(&token).context(TransitErrorContext!(NoInvite))?;
    let key_hash = common::sha256(&key);

    let invite = sqlx::query!(
        r#"
        SELECT email, expiry
        FROM invites
        WHERE key_hash = $1
        FOR UPDATE
        "#,
        key_hash.as_slice(),
    )
    .fetch_optional(&mut *tx)
    .await
    .context(InternalErrorContext!(via(display), common::ERROR_DB))?
    .context(NoInvite)?;

    if invite.expiry > Utc::now() {
        return Err(InviteExpired.into());
    }

    snafu::ensure!(common::USERNAME_RE.is_match(&username), InvalidUsername);
    user::ensure_password_strength(&password, &[&username, &invite.email])?;

    let hash = user::hash_password(&password)?;
    let uid = cuid2::cuid();

    match sqlx::query!(
        r#"
        INSERT INTO users (id, username, email, password_hash)
        VALUES ($1, $2, $3, $4)
        "#,
        uid,
        username,
        invite.email,
        hash,
    )
    .execute(&mut *tx)
    .await
    {
        Ok(_) => {}
        Err(sqlx::Error::Database(db)) if db.is_unique_violation() => {
            return Err(UsernameInUse.into());
        }
        Err(other) => {
            return Err(InternalError!(ctx(display other), common::ERROR_DB).into());
        }
    }

    let memberships = sqlx::query_as!(
        Membership,
        r#"
        INSERT INTO roles (uid, application, admin)
        SELECT $1, application, admin
        FROM invite_roles
        WHERE invite = $2
        RETURNING application AS slug, admin
        "#,
        uid,
        key_hash,
    )
    .fetch_all(&mut *tx)
    .await
    .context(InternalErrorContext!(via(display), common::ERROR_DB))?;

    sqlx::query!(
        r#"
        DELETE FROM invites
        WHERE key_hash = $1
        "#,
        key_hash,
    )
    .execute(&mut *tx)
    .await
    .context(InternalErrorContext!(via(display), common::ERROR_DB))?;

    tx.commit()
        .await
        .context(InternalErrorContext!(via(display), common::ERROR_DB))?;

    Ok(User {
        id: uid,
        username,
        email: invite.email,
        memberships,
    })
}
