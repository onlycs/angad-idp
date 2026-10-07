use std::{sync::Arc, time::Duration};

use argon2::{Argon2, PasswordVerifier};
use idp_proto::{
    auth::{
        AccessLevel, AuthenticateError, AuthenticateRequest, Authentication, BadPassword, Token,
        TokenRaw,
    },
    error::Denied,
    user::NoUser,
};
use snafu::{OptionExt, ResultExt};
use sqlx::{PgPool, types::chrono};
use transit_core::{InternalError, InternalErrorContext, TransitErrorContext};

use crate::{strings, token, user};

pub(crate) async fn authenticate(
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
                Err(other) => {
                    return Err(InternalError!(ctx(display other), strings::ERROR_DB));
                }
            };

            let updated = updated.updated_at.timestamp_millis() as u64;
            let exp = token.raw.exp;

            if updated > exp {
                return Ok(Err(Denied));
            }

            Ok(token::verify(&token)
                .context(InternalErrorContext!("Failed to verify token"))?
                .map(|_| ()))
        }
        Authentication::Integration(int) => {
            let res = sqlx::query!(r#"SELECT read_only FROM integrations WHERE name = $1"#, int)
                .fetch_one(pg)
                .await;

            let auth = match res {
                Ok(auth) => auth,
                Err(sqlx::Error::RowNotFound) => return Ok(Err(Denied)),
                Err(other) => {
                    return Err(InternalError!(ctx(display other), strings::ERROR_DB));
                }
            };

            if auth.read_only && level == AccessLevel::ReadWrite {
                return Ok(Err(Denied));
            }

            Ok(Ok(()))
        }
    }
}

pub(crate) async fn route(
    AuthenticateRequest { query, password }: AuthenticateRequest,
    pg: Arc<PgPool>,
) -> Result<Token, AuthenticateError> {
    let user = user::query(&query, &pg).await?.context(NoUser)?;
    let hash = user.password_hash;

    let argon2 = Argon2::new(
        argon2::Algorithm::Argon2id,
        argon2::Version::V0x13,
        argon2::Params::DEFAULT,
    );

    argon2
        .verify_password(password.as_bytes(), hash.as_str())
        .context(TransitErrorContext!(BadPassword))?;

    let raw = TokenRaw {
        uid: user.id,
        username: user.username,
        email: user.email,
        exp: (chrono::Utc::now() + Duration::from_days(7)).timestamp_millis() as u64,
    };

    Ok(token::sign(raw).context(InternalErrorContext!("Failed to sign token"))?)
}
