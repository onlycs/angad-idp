use std::sync::Arc;

use idp_proto::{app::*, auth::AccessLevel};
use snafu::prelude::*;
use sqlx::PgPool;
use transit_core::InternalErrorMessage;

use crate::{
    auth,
    strings::{self, NAME_RE, SLUG_RE, URI_RE, URL_RE},
};

#[tracing::instrument(skip(pg))]
pub(crate) async fn create(
    ApplicationCreateRequest { auth, app }: ApplicationCreateRequest,
    pg: Arc<PgPool>,
) -> Result<ApplicationCreateResponse, ApplicationCreateError> {
    let mut res = ApplicationCreateResponse {
        client_secret: None,
    };

    auth::authenticate(auth, AccessLevel::ReadWrite, &pg).await??;

    let slug_valid = SLUG_RE.is_match(&app.slug);
    let name_valid = NAME_RE.is_match(&app.name);
    let url_valid = URL_RE.is_match(&app.url);
    let invalid_redirect_uri = app
        .oidc
        .as_ref()
        .and_then(|oidc| oidc.redirect_uris.iter().find(|uri| !URI_RE.is_match(uri)));

    if !slug_valid {
        return Err(InvalidSlug.into());
    }

    if !name_valid {
        return Err(InvalidAppName.into());
    }

    if !url_valid {
        return Err(InvalidUrl.into());
    }

    if let Some(uri) = invalid_redirect_uri {
        return Err(InvalidRedirectUri {
            uri: uri.to_string(),
        }
        .into());
    }

    let result = sqlx::query!(
        r#"
        INSERT INTO applications (slug, name, url)
        VALUES ($1, $2, $3)
        ON CONFLICT DO NOTHING
        "#,
        app.slug,
        app.name,
        app.url,
    )
    .execute(&*pg)
    .await
    .context(InternalErrorMessage!(ctx:display, strings::ERROR_DB))?;

    if result.rows_affected() == 0 {
        return Err(SlugInUse.into());
    }

    if let Some(oidc) = app.oidc {
        let mut buf = [0u8; 16];

        getrandom::fill(&mut buf)
            .context(InternalErrorMessage!("Could not generate client secret"))?;

        let client_secret = hex::encode(buf);

        sqlx::query!(
            r#"
            INSERT INTO applications_oidc (slug, client_secret, redirect_uris)
            VALUES ($1, $2, $3)
            "#,
            app.slug,
            client_secret,
            oidc.redirect_uris.as_slice(),
        )
        .execute(&*pg)
        .await
        .context(InternalErrorMessage!(ctx:display, strings::ERROR_DB))?;

        res.client_secret = Some(client_secret);
    }

    Ok(res)
}
