use idp_proto::{
    auth::{AccessLevel, Authentication},
    error::Denied,
};
use snafu::ResultExt;
use sqlx::PgPool;
use transit_core::{InternalError, InternalErrorMessage};

use crate::token;

pub async fn authenticate(
    auth: Authentication,
    level: AccessLevel,
    pg: &PgPool,
) -> Result<Result<(), Denied>, InternalError> {
    match auth {
        Authentication::Token(token) => {
            let updated = match sqlx::query!(
                r#"SELECT updated_at FROM users WHERE id = $1"#,
                token.raw.uid
            )
            .fetch_one(pg)
            .await
            {
                Ok(updated) => updated,
                Err(sqlx::Error::RowNotFound) => return Ok(Err(Denied)),
                other => {
                    other.context(InternalErrorMessage!("Error communicating with DB"))?;
                    snafu::whatever!("Unknown error")
                }
            };

            let updated = updated.updated_at.timestamp_millis() as u64;
            let exp = token.raw.exp;

            if updated > exp {
                return Ok(Err(Denied));
            }

            Ok(token::verify(&token)
                .context(InternalErrorMessage!("Failed to verify token"))?
                .map(|_| ()))
        }
        Authentication::Integration(int) => {
            let res = sqlx::query!(r#"SELECT read_only FROM integrations WHERE name = $1"#, int)
                .fetch_one(pg)
                .await;

            let auth = match res {
                Ok(auth) => auth,
                Err(sqlx::Error::RowNotFound) => return Ok(Err(Denied)),
                other => {
                    other.context(InternalErrorMessage!("Error communicating with DB"))?;
                    snafu::whatever!("Unknown error")
                }
            };

            if auth.read_only && level == AccessLevel::ReadWrite {
                return Ok(Err(Denied));
            }

            Ok(Ok(()))
        }
    }
}
