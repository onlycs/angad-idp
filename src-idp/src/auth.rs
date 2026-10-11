use idp_proto::{
    app::Membership,
    auth::{AccessLevel, Authentication, AuthenticationStrict},
    error::Denied,
};
use sqlx::PgPool;
use transit_core::InternalError;

use crate::{common, token, user};

async fn authenticate_integration(
    key: String,
    level: AccessLevel,
    pg: &PgPool,
) -> Result<Result<(), Denied>, InternalError> {
    let res = sqlx::query!(
        r#"SELECT read_only FROM integrations WHERE key_hash = $1"#,
        common::sha256(key.as_bytes())
    )
    .fetch_one(pg)
    .await;

    let auth = match res {
        Ok(auth) => auth,
        Err(sqlx::Error::RowNotFound) => return Ok(Err(Denied)),
        Err(other) => {
            return Err(InternalError!(ctx(display other), common::ERROR_DB));
        }
    };

    if auth.read_only && level == AccessLevel::ReadWrite {
        return Ok(Err(Denied));
    }

    Ok(Ok(()))
}

pub(crate) async fn authenticate(
    auth: Authentication,
    level: AccessLevel,
    pg: &PgPool,
) -> Result<Result<(), Denied>, InternalError> {
    match auth {
        Authentication::Token(token) => {
            let token = match token::parse(&token)? {
                Ok(token) => token,
                Err(denied) => return Ok(Err(denied)),
            };

            let perms = match sqlx::query!(
                r#"
                SELECT updated_at FROM users
                WHERE id = $1;
                "#,
                token.uid,
            )
            .fetch_one(pg)
            .await
            {
                Ok(perms) => perms,
                Err(sqlx::Error::RowNotFound) => return Ok(Err(Denied)),
                Err(other) => {
                    return Err(InternalError!(ctx(display other), common::ERROR_DB));
                }
            };

            if perms.updated_at > token.issued
                || token.memberships.iter().any(Membership::is_domain_admin)
            {
                return Ok(Err(Denied));
            }

            Ok(Ok(()))
        }
        Authentication::Integration(key) => authenticate_integration(key, level, pg).await,
    }
}

pub(crate) async fn authenticate_strict(
    auth: AuthenticationStrict,
    level: AccessLevel,
    pg: &PgPool,
) -> Result<Result<(), Denied>, InternalError> {
    match auth {
        AuthenticationStrict::Password { uid, password } => {
            let hashed = match sqlx::query!(
                r#"
                SELECT password_hash
                FROM users
                WHERE id = $1
                "#,
                uid,
            )
            .fetch_one(pg)
            .await
            {
                Ok(hashed) => hashed,
                Err(sqlx::Error::RowNotFound) => return Ok(Err(Denied)),
                Err(other) => {
                    return Err(InternalError!(ctx(display other), common::ERROR_DB));
                }
            };

            if let Err(_incorrect) = user::verify_password(&hashed.password_hash, &password) {
                return Ok(Err(Denied));
            }

            Ok(Ok(()))
        }
        AuthenticationStrict::Integration(key) => authenticate_integration(key, level, pg).await,
    }
}
