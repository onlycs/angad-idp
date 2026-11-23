use std::sync::Arc;

use idp_proto::{
    app::*,
    auth::{AccessLevel, Authentication},
};
use itertools::Itertools;
use snafu::prelude::*;
use sqlx::{PgExecutor, PgPool};
use transit_core::{InternalError, InternalErrorContext, error};

use crate::{
    auth::{self, AuthErrorCast},
    authelia,
    common::{self, SLUG_RE, URL_RE},
};

pub(crate) const DOMAIN_NAME: &str = "Angad IdP";
pub(crate) use idp_proto::app::DOMAIN_SLUG;

/// Meaningless for public clients (`client_secret_hash IS NULL`), as is `pkce`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, sqlx::Type, serde::Deserialize)]
#[sqlx(type_name = "tokenauthmethod", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub(crate) enum TokenAuthMethod {
    #[default]
    ClientSecretBasic,
    ClientSecretPost,
}

impl TokenAuthMethod {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::ClientSecretBasic => "client_secret_basic",
            Self::ClientSecretPost => "client_secret_post",
        }
    }
}

impl From<&ApplicationOidcAuthMethod> for TokenAuthMethod {
    fn from(value: &ApplicationOidcAuthMethod) -> Self {
        match value {
            ApplicationOidcAuthMethod::ClientSecretBasic => Self::ClientSecretBasic,
            ApplicationOidcAuthMethod::ClientSecretPost => Self::ClientSecretPost,
        }
    }
}

impl From<TokenAuthMethod> for ApplicationOidcAuthMethod {
    fn from(value: TokenAuthMethod) -> Self {
        match value {
            TokenAuthMethod::ClientSecretBasic => Self::ClientSecretBasic,
            TokenAuthMethod::ClientSecretPost => Self::ClientSecretPost,
        }
    }
}

fn private_from_row(
    public: bool,
    method: TokenAuthMethod,
    pkce: bool,
) -> Option<ApplicationOidcPrivate> {
    (!public).then(|| ApplicationOidcPrivate {
        method: method.into(),
        pkce,
    })
}

/// Returns the plaintext client secret iff `oidc.private` is set
async fn insert_oidc<'a>(
    slug: &str,
    oidc: &ApplicationOidcSupplied,
    pg: impl PgExecutor<'a>,
) -> Result<Option<String>, InternalError> {
    let client_secret = oidc
        .private
        .as_ref()
        .map(|_| common::generate_secret().map(hex::encode))
        .transpose()?;

    let hash = client_secret
        .as_ref()
        .map(|it| common::hash(it.as_bytes()))
        .transpose()?;

    let (method, pkce) = oidc
        .private
        .as_ref()
        .map_or((TokenAuthMethod::default(), true), |p| {
            ((&p.method).into(), p.pkce)
        });

    sqlx::query!(
        r#"
        INSERT INTO applications_oidc (slug, client_secret_hash, redirect_uris, token_endpoint_auth_method, pkce)
        VALUES ($1, $2, $3, $4, $5)
        "#,
        slug,
        hash,
        oidc.redirect_uris.as_slice(),
        method as TokenAuthMethod,
        pkce,
    )
    .execute(pg)
    .await
    .context(InternalErrorContext!(via(display), common::ERROR_DB))?;

    Ok(client_secret)
}

error! {
    VerifyError = NoApplication;
}

pub(crate) async fn verify<'a>(
    slugs: &[String],
    pg: impl PgExecutor<'a>,
) -> Result<(), VerifyError> {
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
pub(crate) fn split_and_fix(memberships: &mut Vec<Membership>) -> (Vec<String>, Vec<bool>) {
    if memberships
        .iter()
        .find(|it| it.slug == DOMAIN_SLUG)
        .is_none()
    {
        memberships.push(Membership {
            slug: DOMAIN_SLUG.to_string(),
            admin: false,
        });
    }

    memberships
        .iter()
        .sorted_by(|a, b| a.slug.cmp(&b.slug).then(b.admin.cmp(&a.admin)))
        .dedup_by(|a, b| a.slug == b.slug)
        .map(|Membership { slug, admin }| (slug.clone(), *admin))
        .unzip::<_, _, Vec<_>, Vec<_>>()
}

pub(crate) async fn create_impl(
    ApplicationSupplied {
        slug,
        name,
        url,
        oidc,
    }: ApplicationSupplied,
    pg: &PgPool,
) -> Result<ApplicationCreateResponse, ApplicationCreateError> {
    let mut res = ApplicationCreateResponse {
        client_secret: None,
        client_id: None,
    };

    let name = name.trim();

    snafu::ensure!(SLUG_RE.is_match(&slug), InvalidSlug);
    snafu::ensure!(!name.is_empty(), InvalidAppName);
    snafu::ensure!(URL_RE.is_match(&url), InvalidUrl);

    if let Some(uri) = oidc
        .as_ref()
        .and_then(|oidc| oidc.redirect_uris.iter().find(|uri| !common::is_uri(uri)))
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

    match sqlx::query!(
        r#"
        INSERT INTO applications (slug, name, url)
        VALUES ($1, $2, $3)
        "#,
        slug,
        name,
        url,
    )
    .execute(&mut *tx)
    .await
    {
        Ok(_) => {}
        Err(err) => match err {
            sqlx::Error::Database(db) if db.is_unique_violation() => match db.constraint() {
                Some("applications_name_key") => return Err(AppNameInUse.into()),
                _ => return Err(SlugInUse.into()),
            },
            err => return Err(InternalError!(ctx(display err), common::ERROR_DB).into()),
        },
    }

    if let Some(oidc) = oidc {
        res.client_id = Some(common::xxh64_hex(&slug));

        let client_secret = insert_oidc(&slug, &oidc, &mut *tx).await?;

        res.client_secret = client_secret;
    }

    tx.commit()
        .await
        .context(InternalErrorContext!(via(display), common::ERROR_DB))?;

    tokio::spawn(authelia::regenerate(pg.clone()));

    Ok(res)
}

#[tracing::instrument(err, skip(auth, pg))]
pub(crate) async fn create(
    ApplicationCreateRequest { auth, app }: ApplicationCreateRequest,
    pg: Arc<PgPool>,
) -> Result<ApplicationCreateResponse, ApplicationCreateError> {
    auth::authenticate(auth, AccessLevel::ReadWrite, &pg)
        .await
        .context(AuthErrorCast::<ApplicationCreateError>)?;

    create_impl(app, &pg).await
}

#[tracing::instrument(ret, err, skip(auth, pg))]
pub(crate) async fn list(
    auth: Authentication,
    pg: Arc<PgPool>,
) -> Result<Vec<Application>, ApplicationListError> {
    let (allow_all, specific) = match auth {
        Authentication::Integration(key) => {
            auth::authenticate_ikey(key, AccessLevel::Read, &pg)
                .await
                .context(AuthErrorCast::<ApplicationListError>)?;

            (true, vec![])
        }
        Authentication::Token(user) => {
            let token = auth::authenticate_user_any(user, &pg)
                .await
                .context(AuthErrorCast::<ApplicationListError>)?;

            (
                token.is_domain_admin(),
                token
                    .memberships
                    .into_iter()
                    .map(|it| it.slug)
                    .collect_vec(),
            )
        }
    };

    let mut qb = sqlx::QueryBuilder::new(
        r#"
        SELECT
            a.slug,
            a.name,
            a.url,
            o.redirect_uris,
            o.client_secret_hash IS NULL AS public,
            o.token_endpoint_auth_method,
            o.pkce
        FROM applications a
        LEFT JOIN applications_oidc o
            ON a.slug = o.slug
        "#,
    );

    if !allow_all {
        qb.push(r#"WHERE a.slug = ANY("#)
            .push_bind(&specific)
            .push(")");
    }

    #[derive(sqlx::FromRow)]
    struct QueryRes {
        slug: String,
        name: String,
        url: String,
        redirect_uris: Option<Vec<String>>,
        public: Option<bool>,
        token_endpoint_auth_method: Option<TokenAuthMethod>,
        pkce: Option<bool>,
    }

    let apps = qb
        .build_query_as::<QueryRes>()
        .fetch_all(&*pg)
        .await
        .context(InternalErrorContext!(via(display), common::ERROR_DB))?;

    Ok(apps
        .into_iter()
        .map(|a| Application {
            name: a.name,
            url: a.url,
            oidc: match (
                a.redirect_uris,
                a.public,
                a.token_endpoint_auth_method,
                a.pkce,
            ) {
                (Some(uris), Some(public), Some(method), Some(pkce)) => Some(ApplicationOidc {
                    redirect_uris: uris,
                    client_id: common::xxh64_hex(&a.slug),
                    private: private_from_row(public, method, pkce),
                }),
                _ => None,
            },
            slug: a.slug,
        })
        .collect())
}

#[tracing::instrument(err, skip(auth, pg))]
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
    snafu::ensure!(slug != DOMAIN_SLUG, ModifyDomain);

    let mut tx = pg
        .begin()
        .await
        .context(InternalErrorContext!(via(display), common::ERROR_DB))?;

    let mut res_oidc = sqlx::query!(
        r#"
        SELECT
            redirect_uris,
            client_secret_hash IS NULL as "public!: bool",
            token_endpoint_auth_method as "token_endpoint_auth_method: TokenAuthMethod",
            pkce
        FROM applications_oidc
        WHERE slug = $1
        "#,
        slug,
    )
    .fetch_optional(&mut *tx)
    .await
    .context(InternalErrorContext!(via(display), common::ERROR_DB))?
    .map(|it| ApplicationOidcSupplied {
        redirect_uris: it.redirect_uris,
        private: private_from_row(it.public, it.token_endpoint_auth_method, it.pkce),
    });

    let mut res_client_secret = None;

    match oidc {
        ApplicationOidcUpdate::Create { .. } if res_oidc.is_some() => {
            return Err(ApplicationOidcExists { slug }.into());
        }

        ApplicationOidcUpdate::Create(oidc) => {
            res_client_secret = insert_oidc(&slug, &oidc, &mut *tx).await?;
            res_oidc = Some(oidc)
        }

        ApplicationOidcUpdate::Modify {
            redirect_uris,
            private,
        } => {
            let Some(res_oidc) = res_oidc.as_mut() else {
                return Err(NoApplicationOidc { slug }.into());
            };

            if let Some(uri) = redirect_uris
                .as_ref()
                .and_then(|it| it.iter().find(|it| !common::is_uri(it)))
            {
                return Err(InvalidRedirectUri { uri: uri.clone() }.into());
            }

            let (write_secret, private) = match (private, res_oidc.private.take()) {
                (ApplicationOidcPrivateUpdate::Keep, current) => (false, current),
                (ApplicationOidcPrivateUpdate::Create(_), Some(_)) => {
                    return Err(AlreadyPrivate { slug }.into());
                }
                (ApplicationOidcPrivateUpdate::Create(new), None) => (true, Some(new)),
                (
                    ApplicationOidcPrivateUpdate::Modify { .. }
                    | ApplicationOidcPrivateUpdate::Remove,
                    None,
                ) => {
                    return Err(NotPrivate { slug }.into());
                }
                (ApplicationOidcPrivateUpdate::Modify { method, pkce, roll }, Some(current)) => (
                    roll,
                    Some(ApplicationOidcPrivate {
                        method: method.unwrap_or(current.method),
                        pkce: pkce.unwrap_or(current.pkce),
                    }),
                ),
                (ApplicationOidcPrivateUpdate::Remove, Some(_)) => (true, None),
            };

            if write_secret && private.is_some() {
                res_client_secret = Some(hex::encode(common::generate_secret()?));
            }

            let hash = res_client_secret
                .as_ref()
                .map(|res| common::hash(res.as_bytes()))
                .transpose()?;

            sqlx::query!(
                r#"
                UPDATE applications_oidc
                SET
                    redirect_uris = COALESCE($2, redirect_uris),
                    client_secret_hash = CASE WHEN $4 THEN $3 ELSE client_secret_hash END,
                    token_endpoint_auth_method = COALESCE($5, token_endpoint_auth_method),
                    pkce = COALESCE($6, pkce)
                WHERE
                    slug = $1
                "#,
                slug,
                redirect_uris.as_ref().map(|x| x.as_slice()),
                hash,
                write_secret,
                private.as_ref().map(|p| TokenAuthMethod::from(&p.method))
                    as Option<TokenAuthMethod>,
                private.as_ref().map(|p| p.pkce),
            )
            .execute(&mut *tx)
            .await
            .context(InternalErrorContext!(via(display), common::ERROR_DB))?;

            res_oidc.private = private;

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
            .execute(&mut *tx)
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
            name: app_update.name,
            url: app_update.url,
            oidc: res_oidc.map(|it| ApplicationOidc {
                redirect_uris: it.redirect_uris,
                client_id: common::xxh64_hex(&slug),
                private: it.private,
            }),
            slug,
        },
        client_secret: res_client_secret,
    })
}

#[tracing::instrument(ret, err, skip(auth, pg))]
pub(crate) async fn delete(
    ApplicationDeleteRequest { auth, slug }: ApplicationDeleteRequest,
    pg: Arc<PgPool>,
) -> Result<(), ApplicationDeleteError> {
    auth::authenticate(auth, AccessLevel::ReadWrite, &pg)
        .await
        .context(AuthErrorCast::<ApplicationDeleteError>)?;

    snafu::ensure!(slug != DOMAIN_SLUG, ModifyDomain);

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
