use std::sync::Arc;

use idp_proto::{
    app::*,
    auth::{AccessLevel, Authentication},
};
use snafu::prelude::*;
use sqlx::PgPool;
use transit_core::{InternalError, InternalErrorMessage};

use crate::{
    auth,
    strings::{self, NAME_RE, SLUG_RE, URI_RE, URL_RE},
};

fn gen_client_secret() -> Result<String, InternalError> {
    let mut buf = [0u8; 16];
    getrandom::fill(&mut buf).context(InternalErrorMessage!("Could not generate client secret"))?;
    Ok(hex::encode(buf))
}

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
    .context(InternalErrorMessage!(ctx(display), strings::ERROR_DB))?;

    if result.rows_affected() == 0 {
        return Err(SlugInUse.into());
    }

    if let Some(oidc) = app.oidc {
        let client_secret = gen_client_secret()?;

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
        .context(InternalErrorMessage!(ctx(display), strings::ERROR_DB))?;

        res.client_secret = Some(client_secret);
    }

    Ok(res)
}

#[tracing::instrument(skip(pg))]
pub(crate) async fn list(
    auth: Authentication,
    pg: Arc<PgPool>,
) -> Result<Vec<Application>, ApplicationListError> {
    auth::authenticate(auth, AccessLevel::Read, &pg).await??;

    let apps = sqlx::query!(
        r#"
        SELECT
            a.slug,
            a.name,
            a.url,
            o.redirect_uris
        FROM applications a
        LEFT JOIN applications_oidc o
            ON a.slug = o.slug
        "#,
    )
    .fetch_all(&*pg)
    .await
    .context(InternalErrorMessage!(ctx(display), strings::ERROR_DB))?;

    Ok(apps
        .into_iter()
        .map(|a| Application {
            slug: a.slug,
            name: a.name,
            url: a.url,
            oidc: a.redirect_uris.map(|uris| ApplicationOidc {
                redirect_uris: uris,
            }),
        })
        .collect())
}

#[tracing::instrument(skip(pg, auth))]
pub(crate) async fn update(
    ApplicationUpdateRequest {
        auth,
        slug,
        name,
        url,
        redirect_uris,
        roll_client_secret,
    }: ApplicationUpdateRequest,
    pg: Arc<PgPool>,
) -> Result<ApplicationUpdateResponse, ApplicationUpdateError> {
    auth::authenticate(auth, AccessLevel::ReadWrite, &pg).await??;

    struct UpdateOidc<'a> {
        current_uris: &'a mut Option<Vec<String>>,
        update_uris: Option<Vec<String>>,
        client_secret: bool,
    }

    let mut res_redirect_uris = sqlx::query!(
        r#"
        SELECT redirect_uris
        FROM applications_oidc
        WHERE slug = $1
        "#,
        slug,
    )
    .fetch_optional(&*pg)
    .await
    .context(InternalErrorMessage!(ctx(display), strings::ERROR_DB))?
    .map(|res| res.redirect_uris);

    let mut res_client_secret = None;

    match (UpdateOidc {
        current_uris: &mut res_redirect_uris,
        update_uris: redirect_uris,
        client_secret: roll_client_secret,
    }) {
        // they ask to roll, but no oidc exists
        UpdateOidc {
            current_uris: None,
            client_secret: true,
            update_uris: _,
        } => {
            return Err(NoApplicationOidc { slug }.into());
        }

        // they ask to only roll with an existing oidc,
        // no redirect uris to update
        UpdateOidc {
            update_uris: None,
            client_secret: true,
            current_uris: Some(_),
        } => {
            let client_secret = gen_client_secret()?;

            sqlx::query!(
                r#"
                UPDATE applications_oidc
                SET client_secret = $1
                WHERE slug = $2
                "#,
                client_secret,
                slug,
            )
            .execute(&*pg)
            .await
            .context(InternalErrorMessage!(ctx(display), strings::ERROR_DB))?;

            res_client_secret = Some(client_secret);
        }

        // they ask to update redirect uris with an existing oidc
        // we can handle roll request dynamically
        UpdateOidc {
            current_uris: Some(_),
            update_uris: Some(uris),
            client_secret: roll,
        } => {
            if roll {
                res_client_secret = Some(gen_client_secret()?);
            }

            match sqlx::query!(
                r#"
                UPDATE applications_oidc
                SET redirect_uris = $1
                WHERE slug = $2
                "#,
                uris.as_slice(),
                slug,
            )
            .execute(&*pg)
            .await
            {
                Ok(_) => {}
                Err(sqlx::Error::Database(db)) if db.is_foreign_key_violation() => {
                    return Err(NoApplication { slug }.into());
                }
                Err(other) => {
                    return Err(InternalErrorMessage!(ctx(display other), strings::ERROR_DB).into());
                }
            }

            res_redirect_uris = Some(uris);
        }

        // they ask to update redirect uris without an existing oidc
        // roll request handled in first arm (should error)
        UpdateOidc {
            current_uris: None,
            update_uris: Some(uris),
            client_secret: false,
        } => {
            let client_secret = gen_client_secret()?;

            sqlx::query!(
                r#"
                INSERT INTO applications_oidc (slug, redirect_uris, client_secret)
                VALUES ($1, $2, $3)
                "#,
                slug,
                uris.as_slice(),
                client_secret,
            )
            .execute(&*pg)
            .await
            .context(InternalErrorMessage!(ctx(display), strings::ERROR_DB))?;

            res_client_secret = Some(client_secret);
            res_redirect_uris = Some(uris);
        }

        // they don't ask to update anything
        UpdateOidc {
            current_uris: _,
            update_uris: None,
            client_secret: _,
        } => {}
    }

    let app_update = match sqlx::query!(
        r#"
        UPDATE applications
        SET
            name = COALESCE($2, name),
            url = COALESCE($3, url)
        WHERE slug = $1
        RETURNING name, url
        "#,
        slug,
        name,
        url,
    )
    .fetch_one(&*pg)
    .await
    {
        Ok(res) => res,
        Err(sqlx::Error::RowNotFound) => {
            return Err(NoApplication { slug }.into());
        }
        Err(other) => {
            return Err(InternalErrorMessage!(ctx(display other), strings::ERROR_DB).into());
        }
    };

    Ok(ApplicationUpdateResponse {
        app: Application {
            slug: slug.clone(),
            name: app_update.name,
            url: app_update.url,
            oidc: res_redirect_uris.map(|uris| ApplicationOidc {
                redirect_uris: uris,
            }),
        },
        client_secret: res_client_secret,
    })
}

#[tracing::instrument(skip(pg, auth))]
pub(crate) async fn delete(
    ApplicationDeleteRequest { auth, slug }: ApplicationDeleteRequest,
    pg: Arc<PgPool>,
) -> Result<(), ApplicationDeleteError> {
    auth::authenticate(auth, AccessLevel::ReadWrite, &pg).await??;

    let res = sqlx::query!(
        r#"
        DELETE FROM applications
        WHERE slug = $1
        "#,
        slug,
    )
    .execute(&*pg)
    .await
    .context(InternalErrorMessage!(ctx(display), strings::ERROR_DB))?;

    if res.rows_affected() == 0 {
        return Err(NoApplication { slug }.into());
    }

    Ok(())
}
