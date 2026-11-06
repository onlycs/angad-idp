use idp_proto::{
    auth::{AccessLevel, Authentication, AuthenticationStrict},
    error::Denied,
};
use snafu::{OptionExt, ResultExt};
use sqlx::PgPool;
use tracing::info;
use transit_core::{InternalErrorContext, error};

use crate::{authelia::AutheliaToken, common, user};

error! {
    AuthError = Denied;
}

pub(crate) async fn authenticate_integration(
    key: impl AsRef<str>,
    level: AccessLevel,
    pg: &PgPool,
) -> Result<(), AuthError> {
    let read_only = sqlx::query_scalar!(
        r#"SELECT read_only FROM integrations WHERE key_hash = $1"#,
        common::sha256(key.as_ref().as_bytes())
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

pub(crate) async fn authenticate_user_any(
    token: impl AsRef<str>,
    pg: &PgPool,
) -> Result<AutheliaToken, AuthError> {
    let token = AutheliaToken::parse(token).await?;

    let updated = sqlx::query_scalar!(
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

    info!("{updated}");

    snafu::ensure!(updated < token.issued, Denied);

    Ok(token)
}

pub(crate) async fn authenticate_user_admin(
    token: impl AsRef<str>,
    pg: &PgPool,
) -> Result<AutheliaToken, AuthError> {
    let token = authenticate_user_any(token, pg).await?;
    snafu::ensure!(token.is_domain_admin(), Denied);

    Ok(token)
}

pub(crate) async fn authenticate(
    auth: Authentication,
    level: AccessLevel,
    pg: &PgPool,
) -> Result<(), AuthError> {
    match auth {
        Authentication::Token(token) => authenticate_user_admin(token, pg).await.map(|_| ()),
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
