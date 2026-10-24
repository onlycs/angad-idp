use std::sync::Arc;

use argon2::PasswordHasher;
use idp_proto::{
    app::*,
    auth::{AccessLevel, Authentication},
};
use itertools::Itertools;
use snafu::prelude::*;
use sqlx::PgPool;
use transit_core::{InternalError, InternalErrorContext, error};

use crate::{
    auth::{self, AuthErrorCast},
    authelia,
    common::{self, SLUG_RE, URI_RE, URL_RE},
};

pub(crate) const DOMAIN_NAME: &str = "Angad IdP";
pub(crate) use idp_proto::app::DOMAIN_SLUG;

error! {
    VerifyError = NoApplication;
}

pub(crate) async fn verify(slugs: &[String], pg: &PgPool) -> Result<(), VerifyError> {
    sqlx::query_scalar!(
        r#"
        SELECT requested.slug
        FROM UNNEST($1::text[]) AS requested(slug)
        WHERE NOT EXISTS (
            SELECT 1
            FROM applications a
            WHERE a.slug = requested.slug
        )
        LIMIT 1
        "#,
        &slugs,
    )
    .fetch_optional(pg)
    .await
    .context(InternalErrorContext!(via(display), common::ERROR_DB))?
    .flatten()
    .map_or(Ok(()), |missing| Err(NoApplication { slug: missing }))?;

    Ok(())
}

#[allow(clippy::ptr_arg)]
pub(crate) fn split_roles(memberships: &Vec<Membership>) -> (Vec<String>, Vec<bool>) {
    memberships
        .iter()
        .sorted_by(|a, b| a.slug.cmp(&b.slug).then(b.admin.cmp(&a.admin)))
        .dedup_by(|a, b| a.slug == b.slug)
        .map(|Membership { slug, admin }| (slug.clone(), admin))
        .unzip::<_, _, Vec<_>, Vec<_>>()
}

pub(crate) async fn create_impl(
    Application {
        slug,
        name,
        url,
        oidc,
    }: Application,
    pg: &PgPool,
) -> Result<ApplicationCreateResponse, ApplicationCreateError> {
    let mut res = ApplicationCreateResponse {
        client_secret: None,
    };

    let name = name.trim();

    snafu::ensure!(SLUG_RE.is_match(&slug), InvalidSlug);
    snafu::ensure!(!name.is_empty(), InvalidAppName);
    snafu::ensure!(URL_RE.is_match(&url), InvalidUrl);

    if let Some(uri) = oidc
        .as_ref()
        .and_then(|oidc| oidc.redirect_uris.iter().find(|uri| !URI_RE.is_match(uri)))
    {
        return Err(InvalidRedirectUri {
            uri: uri.to_string(),
        }
        .into());
    }

    let mut tx = pg
        .begin()
        .await
        .context(InternalErrorContext!(via(display), common::ERROR_DB))?;

    let result = sqlx::query!(
        r#"
        INSERT INTO applications (slug, name, url)
        VALUES ($1, $2, $3)
        ON CONFLICT DO NOTHING
        "#,
        slug,
        name,
        url,
    )
    .execute(&mut *tx)
    .await
    .context(InternalErrorContext!(via(display), common::ERROR_DB))?;

    if result.rows_affected() == 0 {
        return Err(SlugInUse.into());
    }

    if let Some(oidc) = oidc {
        let client_secret = hex::encode(common::generate_secret()?);
        let hash = common::argon2()
            .hash_password(client_secret.as_bytes())
            .context(InternalErrorContext!(via(display), common::ERROR_GENERIC))?
            .to_string();

        sqlx::query!(
            r#"
            INSERT INTO applications_oidc (slug, client_secret_hash, redirect_uris)
            VALUES ($1, $2, $3)
            "#,
            slug,
            hash,
            oidc.redirect_uris.as_slice(),
        )
        .execute(&mut *tx)
        .await
        .context(InternalErrorContext!(via(display), common::ERROR_DB))?;

        res.client_secret = Some(client_secret);
    }

    tx.commit()
        .await
        .context(InternalErrorContext!(via(display), common::ERROR_DB))?;

    tokio::spawn(authelia::regenerate(pg.clone()));

    Ok(res)
}

#[tracing::instrument(skip(pg))]
pub(crate) async fn create(
    ApplicationCreateRequest { auth, app }: ApplicationCreateRequest,
    pg: Arc<PgPool>,
) -> Result<ApplicationCreateResponse, ApplicationCreateError> {
    auth::authenticate(auth, AccessLevel::ReadWrite, &pg)
        .await
        .context(AuthErrorCast::<ApplicationCreateError>)?;

    create_impl(app, &pg).await
}

#[tracing::instrument(skip(pg))]
pub(crate) async fn list(
    auth: Authentication,
    pg: Arc<PgPool>,
) -> Result<Vec<Application>, ApplicationListError> {
    auth::authenticate(auth, AccessLevel::Read, &pg)
        .await
        .context(AuthErrorCast::<ApplicationListError>)?;

    let apps = sqlx::query!(
        r#"
        SELECT
            a.slug,
            a.name,
            a.url,
            o.redirect_uris,
            o.client_secret_hash IS NULL AS public
        FROM applications a
        LEFT JOIN applications_oidc o
            ON a.slug = o.slug
        "#,
    )
    .fetch_all(&*pg)
    .await
    .context(InternalErrorContext!(via(display), common::ERROR_DB))?;

    Ok(apps
        .into_iter()
        .map(|a| Application {
            slug: a.slug,
            name: a.name,
            url: a.url,
            oidc: a
                .redirect_uris
                .zip_with(a.public, |uris, public| ApplicationOidc {
                    redirect_uris: uris,
                    public,
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
        oidc,
    }: ApplicationUpdateRequest,
    pg: Arc<PgPool>,
) -> Result<ApplicationUpdateResponse, ApplicationUpdateError> {
    auth::authenticate(auth, AccessLevel::ReadWrite, &pg)
        .await
        .context(AuthErrorCast::<ApplicationUpdateError>)?;

    let name = name.as_ref().map(|name| name.trim());
    snafu::ensure!(name.is_none_or(|n| !n.is_empty()), InvalidAppName);
    snafu::ensure!(url.as_ref().is_none_or(|u| URL_RE.is_match(u)), InvalidUrl);

    let mut tx = pg
        .begin()
        .await
        .context(InternalErrorContext!(via(display), common::ERROR_DB))?;

    let mut res_oidc = sqlx::query_as!(
        ApplicationOidc,
        r#"
        SELECT redirect_uris, client_secret_hash IS NULL as "public!: bool"
        FROM applications_oidc
        WHERE slug = $1
        "#,
        slug,
    )
    .fetch_optional(&mut *tx)
    .await
    .context(InternalErrorContext!(via(display), common::ERROR_DB))?;

    let mut res_client_secret = None;

    match oidc {
        ApplicationOidcUpdate::Create(_) if res_oidc.is_some() => {
            return Err(ApplicationOidcExists { slug }.into());
        }

        ApplicationOidcUpdate::Create(oidc) => {
            if !oidc.public {
                res_client_secret = Some(hex::encode(common::generate_secret()?));
            }

            let hash = res_client_secret
                .as_ref()
                .map(|res| {
                    common::argon2()
                        .hash_password(res.as_bytes())
                        .context(InternalErrorContext!(via(display), common::ERROR_GENERIC))
                        .map(|hash| hash.to_string())
                })
                .transpose()?;

            sqlx::query!(
                r#"
                INSERT INTO applications_oidc (slug, client_secret_hash, redirect_uris)
                VALUES ($1, $2, $3)
                "#,
                slug,
                hash,
                oidc.redirect_uris.as_slice()
            )
            .execute(&*pg)
            .await
            .context(InternalErrorContext!(via(display), common::ERROR_DB))?;

            res_oidc = Some(oidc)
        }

        ApplicationOidcUpdate::Modify {
            redirect_uris,
            mut public,
        } => {
            let Some(res_oidc) = res_oidc.as_mut() else {
                return Err(NoApplicationOidc { slug }.into());
            };

            if public == ApplicationPublicUpdate::RollClientSecret && res_oidc.public {
                return Err(NotPrivate { slug }.into());
            }

            if let ApplicationPublicUpdate::Set(to) = public
                && to == res_oidc.public
            {
                public = ApplicationPublicUpdate::Keep;
            }

            if public == ApplicationPublicUpdate::RollClientSecret
                || public == ApplicationPublicUpdate::Set(false)
            {
                res_client_secret = Some(hex::encode(common::generate_secret()?));
            }

            let hash = res_client_secret
                .as_ref()
                .map(|res| {
                    common::argon2()
                        .hash_password(res.as_bytes())
                        .context(InternalErrorContext!(via(display), common::ERROR_GENERIC))
                        .map(|hash| hash.to_string())
                })
                .transpose()?;

            sqlx::query!(
                r#"
                UPDATE applications_oidc
                SET
                    redirect_uris = COALESCE($2, redirect_uris),
                    client_secret_hash = $3
                WHERE
                    slug = $1
                "#,
                slug,
                redirect_uris.as_ref().map(|x| x.as_slice()),
                hash
            )
            .execute(&*pg)
            .await
            .context(InternalErrorContext!(via(display), common::ERROR_DB))?;

            if let ApplicationPublicUpdate::Set(to) = public {
                res_oidc.public = to;
            }

            if let Some(it) = redirect_uris {
                res_oidc.redirect_uris = it;
            }
        }

        ApplicationOidcUpdate::Remove if res_oidc.is_none() => {
            return Err(NoApplicationOidc { slug }.into());
        }

        ApplicationOidcUpdate::Remove => {
            sqlx::query!(
                r#"
                DELETE FROM applications_oidc
                WHERE slug = $1
                "#,
                slug
            )
            .execute(&*pg)
            .await
            .context(InternalErrorContext!(via(display), common::ERROR_DB))?;

            res_oidc = None;
        }

        ApplicationOidcUpdate::Keep => {}
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
    .fetch_one(&mut *tx)
    .await
    {
        Ok(res) => res,
        Err(sqlx::Error::RowNotFound) => {
            return Err(NoApplication { slug }.into());
        }
        Err(other) => {
            return Err(InternalError!(ctx(display other), common::ERROR_DB).into());
        }
    };

    tx.commit()
        .await
        .context(InternalErrorContext!(via(display), common::ERROR_DB))?;

    tokio::spawn(authelia::regenerate(pg));

    Ok(ApplicationUpdateResponse {
        app: Application {
            slug: slug.clone(),
            name: app_update.name,
            url: app_update.url,
            oidc: res_oidc,
        },
        client_secret: res_client_secret,
    })
}

#[tracing::instrument(skip(pg, auth))]
pub(crate) async fn delete(
    ApplicationDeleteRequest { auth, slug }: ApplicationDeleteRequest,
    pg: Arc<PgPool>,
) -> Result<(), ApplicationDeleteError> {
    auth::authenticate(auth, AccessLevel::ReadWrite, &pg)
        .await
        .context(AuthErrorCast::<ApplicationDeleteError>)?;

    let res = sqlx::query!(
        r#"
        DELETE FROM applications
        WHERE slug = $1
        "#,
        slug,
    )
    .execute(&*pg)
    .await
    .context(InternalErrorContext!(via(display), common::ERROR_DB))?;

    if res.rows_affected() == 0 {
        return Err(NoApplication { slug }.into());
    }

    tokio::spawn(authelia::regenerate(pg));

    Ok(())
}
