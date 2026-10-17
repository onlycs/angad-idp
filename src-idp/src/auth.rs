use idp_proto::{
    auth::{AccessLevel, Authentication, AuthenticationStrict},
    error::Denied,
};
use snafu::{OptionExt, ResultExt};
use sqlx::PgPool;
use transit_core::{InternalErrorContext, error};

use crate::{app, common, token, user};

error! {
    AuthError = Denied;
}

async fn authenticate_integration(
    key: String,
    level: AccessLevel,
    pg: &PgPool,
) -> Result<(), AuthError> {
    let read_only = sqlx::query_scalar!(
        r#"SELECT read_only FROM integrations WHERE key_hash = $1"#,
        common::sha256(key.as_bytes())
    )
    .fetch_optional(pg)
    .await
    .context(InternalErrorContext!(via(display), common::ERROR_DB))?
    .context(Denied)?;

    if read_only && level == AccessLevel::ReadWrite {
        return Err(Denied.into());
    }

    Ok(())
}

pub(crate) async fn authenticate(
    auth: Authentication,
    level: AccessLevel,
    pg: &PgPool,
) -> Result<(), AuthError> {
    match auth {
        Authentication::Token(token) => {
            let token = token::parse(&token)?;

            let perms = sqlx::query!(
                r#"
                SELECT updated_at FROM users
                WHERE id = $1;
                "#,
                token.uid,
            )
            .fetch_optional(pg)
            .await
            .context(InternalErrorContext!(via(display), common::ERROR_DB))?
            .context(Denied)?;

            if perms.updated_at > token.issued
                || token
                    .memberships
                    .iter()
                    .any(|ship| ship.slug == app::DOMAIN_SLUG && ship.admin)
            {
                return Err(Denied.into());
            }

            Ok(())
        }
        Authentication::Integration(key) => authenticate_integration(key, level, pg).await,
    }
}

pub(crate) async fn authenticate_strict(
    auth: AuthenticationStrict,
    level: AccessLevel,
    pg: &PgPool,
) -> Result<(), AuthError> {
    match auth {
        AuthenticationStrict::Password { uid, password } => {
            let hashed = sqlx::query_scalar!(
                r#"
                SELECT password_hash
                FROM users
                WHERE id = $1
                "#,
                uid,
            )
            .fetch_optional(pg)
            .await
            .context(InternalErrorContext!(via(display), common::ERROR_DB))?
            .context(Denied)?;

            if let Err(_incorrect) = user::verify_password(&hashed, &password) {
                return Err(Denied.into());
            }

            Ok(())
        }
        AuthenticationStrict::Integration(key) => authenticate_integration(key, level, pg).await,
    }
}
