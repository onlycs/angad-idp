use std::sync::{Arc, LazyLock};

use argon2::PasswordHasher;
use ed25519_dalek::{
    Signature,
    ed25519::signature::{AsyncSigner, AsyncVerifier},
};
use idp_proto::{
    forgot::{
        ForgotCaptchaResponse, InvalidCaptcha, InvalidReset, ResetError, ResetRequest,
        SendEmailError, SendEmailRequest, VerifyFail,
    },
    invite::EmailSend,
    user::InvalidEmail,
};
use lettre::{
    AsyncTransport, Message,
    message::{Mailbox, header::ContentType},
};
use serde::Deserialize;
use snafu::prelude::*;
use sqlx::PgPool;
use transit_core::{InternalError, InternalErrorContext, TransitErrorContext};

use crate::{authelia, common, user};

#[derive(bitcode::Encode, bitcode::Decode)]
struct EmailPayload {
    uid: String,
    email: String,
    exp: i64,
    iat: i64,
}

#[tracing::instrument(ret, err)]
pub(crate) async fn captcha(_: ()) -> Result<ForgotCaptchaResponse, InternalError> {
    Ok(ForgotCaptchaResponse {
        site_key: common::TURNSTILE_PUBLIC.to_string(),
    })
}

#[tracing::instrument(ret, err, skip(token, pg))]
pub(crate) async fn send_email(
    SendEmailRequest { email, token }: SendEmailRequest,
    pg: Arc<PgPool>,
) -> Result<(), SendEmailError> {
    static CLIENT: LazyLock<reqwest::Client> = LazyLock::new(reqwest::Client::new);

    #[derive(Deserialize)]
    struct TurnstileResponse {
        success: bool,
    }

    let result: TurnstileResponse = CLIENT
        .post("https://challenges.cloudflare.com/turnstile/v0/siteverify")
        .json(&serde_json::json!({
            "secret": &*common::TURNSTILE_SECRET,
            "response": token,
        }))
        .send()
        .await
        .context(TransitErrorContext!(VerifyFail))?
        .error_for_status()
        .context(TransitErrorContext!(VerifyFail))?
        .json()
        .await
        .context(TransitErrorContext!(VerifyFail))?;

    snafu::ensure!(result.success, InvalidCaptcha);

    let email_parsed = common::parse_email(&email).context(TransitErrorContext!(InvalidEmail))?;
    let Some(uid) =
        sqlx::query_scalar!("SELECT id FROM users WHERE lower(email) = lower($1)", email)
            .fetch_optional(&*pg)
            .await
            .context(InternalErrorContext!(via(display), common::ERROR_DB))?
    else {
        return Ok(());
    };

    let now = chrono::Utc::now().timestamp_millis();
    let payload = bitcode::encode(&EmailPayload {
        uid,
        email,
        exp: now + (1000 * 60 * 15), // 15m
        iat: now,
    });

    let sk = authelia::sign_key()
        .await
        .context(InternalErrorContext!(common::ERROR_GENERIC))?;

    let sig = sk
        .sign_async(&payload)
        .await
        .context(InternalErrorContext!(common::ERROR_GENERIC))?;

    let sig = sig.to_bytes();
    let key = hex::encode(sig) + hex::encode(&payload).as_str();

    let message = Message::builder()
        .from(Mailbox::new(None, common::SMTP_FROM.clone()))
        .to(Mailbox::new(None, email_parsed))
        .subject("Angad Account Password Reset")
        .header(ContentType::TEXT_HTML)
        .body(
            include_str!("../templates/reset-email-template.html").replace(
                "{{reset_url}}",
                format!("{}forgot?key={key}", *common::FRONTEND_URL).as_str(),
            ),
        )
        .context(TransitErrorContext!(EmailSend))?;

    common::SMTP_TRANSPORT
        .send(message)
        .await
        .context(TransitErrorContext!(EmailSend))?;

    Ok(())
}

#[tracing::instrument(ret, err, skip(key, password, pg))]
pub(crate) async fn reset(
    ResetRequest { key, password }: ResetRequest,
    pg: Arc<PgPool>,
) -> Result<(), ResetError> {
    let bytes = hex::decode(&key).context(TransitErrorContext!(InvalidReset))?;
    snafu::ensure!(bytes.len() >= 64, InvalidReset);

    let sig = <&[u8; 64]>::try_from(&bytes[..64]).expect("unreachable");
    let pl = &bytes[64..];

    let pk = authelia::sign_key()
        .await
        .context(InternalErrorContext!(common::ERROR_GENERIC))?
        .verifying_key();

    pk.verify_async(pl, &Signature::from_bytes(sig))
        .await
        .context(TransitErrorContext!(InvalidReset))?;

    let pl = bitcode::decode::<EmailPayload>(pl).context(TransitErrorContext!(InvalidReset))?;
    snafu::ensure!(
        chrono::Utc::now().timestamp_millis() <= pl.exp,
        InvalidReset
    );

    user::ensure_password_strength(&password, &[&pl.uid, &pl.email])?;

    let hash = common::argon2()
        .hash_password(password.as_bytes())
        .context(InternalErrorContext!(common::ERROR_GENERIC))?
        .to_string();

    let res = sqlx::query!(
        r#"
        UPDATE users
        SET
            password_hash = $1,
            updated_at = NOW()
        WHERE
            id = $2 AND
            updated_at < $3
        "#,
        hash,
        pl.uid,
        chrono::DateTime::from_timestamp_millis(pl.iat).context(InvalidReset)?
    )
    .execute(&*pg)
    .await
    .context(InternalErrorContext!(common::ERROR_DB))?;

    snafu::ensure!(res.rows_affected() > 0, InvalidReset);

    Ok(())
}
