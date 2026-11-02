use std::{io, path::PathBuf, str::FromStr};

use askama::Template;
use chrono::{TimeZone, Utc};
use ed25519_dalek::{
    SigningKey,
    pkcs8::{DecodePrivateKey, EncodePrivateKey as EdEncode},
};
use getrandom::{SysRng, rand_core::UnwrapErr};
use idp_proto::{
    app::{DOMAIN_SLUG, Membership},
    auth::AccessLevel,
    error::Denied,
    integration::{INTERNAL_INTEGRATION_ID, INTERNAL_INTEGRATION_NAME},
};
use jsonwebtoken::{Algorithm, DecodingKey, Validation};
use rsa::{
    RsaPrivateKey,
    pkcs8::{EncodePrivateKey as RsaEncode, LineEnding},
    rand_core::OsRng,
};
use serde::Deserialize;
use snafu::{Location, OptionExt, ResultExt, Snafu};
use sqlx::types::Json;
use tokio::{fs, io::AsyncWriteExt};
use tracing::{error, info};
use transit_core::InternalError;

use crate::{
    auth::AuthError,
    common::{self, AUTHELIA_URL, FRONTEND_URL, PgRef},
    integration,
};

mod filters {
    use std::fmt;

    use itertools::Itertools;

    use crate::common;

    #[askama::filter_fn]
    pub(crate) fn xxh64(
        value: &dyn fmt::Display,
        _: &dyn askama::Values,
    ) -> askama::Result<String> {
        Ok(common::xxh64_hex(value))
    }

    #[askama::filter_fn]
    pub(crate) fn multilined(
        value: &dyn fmt::Display,
        _: &dyn askama::Values,
        indent: usize,
    ) -> askama::Result<String> {
        Ok(value
            .to_string()
            .lines()
            .enumerate()
            .map(|(n, line)| {
                if n == 0 {
                    line.to_string()
                } else {
                    format!("{}{line}", "  ".repeat(indent))
                }
            })
            .join("\n"))
    }
}

#[derive(Snafu, Debug)]
pub(crate) enum AutheliaError {
    #[snafu(display("At {location}: Database error\n{source}"))]
    Db {
        source: sqlx::Error,
        #[snafu(implicit)]
        location: Location,
    },

    #[snafu(display("At {location}: Askama error\n{source}"))]
    Askama {
        source: askama::Error,
        #[snafu(implicit)]
        location: Location,
    },

    #[snafu(display("At {location}: Failed to open `{path:?}`\n{source}",))]
    FileOpen {
        path: PathBuf,
        source: io::Error,
        #[snafu(implicit)]
        location: Location,
    },

    #[snafu(display("At {location}: Failed to write to `{path:?}`\n{source}"))]
    FileWrite {
        path: PathBuf,
        source: io::Error,
        #[snafu(implicit)]
        location: Location,
    },

    #[snafu(display("At {location}: Failed to read `{path:?}`\n{source}"))]
    FileRead {
        path: PathBuf,
        source: io::Error,
        #[snafu(implicit)]
        location: Location,
    },

    #[snafu(display("At {location}: Failed to parse yaml from `{path:?}`\n{source}"))]
    ConfigParse {
        path: PathBuf,
        source: serde_yaml::Error,
        #[snafu(implicit)]
        location: Location,
    },

    #[snafu(display("At {location}: Missing string `authentication_backend.ldap.password`"))]
    MissingConfigKey {
        #[snafu(implicit)]
        location: Location,
    },

    #[snafu(display("At {location}: Failed RSA generation\n{source}"))]
    RsaGen {
        source: rsa::Error,
        #[snafu(implicit)]
        location: Location,
    },

    #[snafu(display("At {location}: Failed RSA to PEM export\n{source}"))]
    RsaExport {
        source: rsa::pkcs8::Error,
        #[snafu(implicit)]
        location: Location,
    },

    #[snafu(display("At {location}: Failed Ed25519 to PEM export\n{source}"))]
    EdExport {
        source: ed25519_dalek::pkcs8::Error,
        #[snafu(implicit)]
        location: Location,
    },

    #[snafu(display("At {location}: Failed PEM to Ed25519 import\n{source}"))]
    EdImport {
        source: ed25519_dalek::pkcs8::Error,
        #[snafu(implicit)]
        location: Location,
    },

    #[snafu(display("At {location}: Failed to decode JWT\n{source}"))]
    JwtDecode {
        source: jsonwebtoken::errors::Error,
        #[snafu(implicit)]
        location: Location,
    },

    #[snafu(display("At {location}: Invalid client_id `{found}` in JWT"))]
    InvalidClientId {
        found: String,
        #[snafu(implicit)]
        location: Location,
    },

    #[snafu(display("At {location}: Invalid iat `{found}` in JWT"))]
    InvalidIat {
        found: i64,
        #[snafu(implicit)]
        location: Location,
    },

    #[snafu(display("At {location}: Failed to restart authelia\n{source}"))]
    Restart {
        source: snafu::Whatever,
        #[snafu(implicit)]
        location: Location,
    },
}

pub(crate) struct AutheliaSnafu;

impl snafu::IntoError<AuthError> for AutheliaSnafu {
    type Source = AutheliaError;

    fn into_error(self, source: Self::Source) -> AuthError {
        match source {
            AutheliaError::JwtDecode { .. }
            | AutheliaError::InvalidClientId { .. }
            | AutheliaError::InvalidIat { .. } => Denied.into(),
            _ => InternalError!(ctx(source), common::ERROR_GENERIC).into(),
        }
    }
}

#[derive(serde::Deserialize)]
struct ApplicationOidc {
    pub redirect_uris: Vec<String>,
    pub client_secret_hash: Option<String>,
}

struct Application {
    pub slug: String,
    pub name: String,
    pub oidc: Option<Json<ApplicationOidc>>,
}

#[derive(Template)]
#[template(path = "authelia-config-template.yml")]
#[allow(non_snake_case)]
struct AutheliaConfigTemplate<'a> {
    applications: Vec<Application>,
    integration_key: String,
    rsa2048_private: &'a str,
    ed25519_private: &'a str,
}

async fn pem(
    path: PathBuf,
    generator: fn() -> Result<String, AutheliaError>,
) -> Result<String, AutheliaError> {
    if path.exists() && path.is_file() {
        fs::read_to_string(&path)
            .await
            .context(FileReadSnafu { path })
    } else {
        let contents = generator()?;

        fs::write(&path, contents.as_bytes())
            .await
            .context(FileWriteSnafu { path })?;

        Ok(contents.to_string())
    }
}

fn make_rsa() -> Result<String, AutheliaError> {
    let mut rng = OsRng;
    let private = RsaPrivateKey::new(&mut rng, 2048).context(RsaGenSnafu)?;

    private
        .to_pkcs8_pem(LineEnding::LF)
        .context(RsaExportSnafu)
        .map(|it| it.to_string())
}

fn make_ed25519() -> Result<String, AutheliaError> {
    let mut rng = UnwrapErr(SysRng);
    let private = SigningKey::generate(&mut rng);

    private
        .to_pkcs8_pem(LineEnding::LF)
        .context(EdExportSnafu)
        .map(|it| it.to_string())
}

pub(crate) async fn verify_key() -> Result<DecodingKey, AutheliaError> {
    let path = &*common::AUTHELIA_CONFIG_DIR;
    let ed25519 = path.join("ed25519.pem");
    let pem = pem(ed25519, make_ed25519).await?;
    let sk = SigningKey::from_pkcs8_pem(&pem).context(EdImportSnafu)?;
    let pk = sk.verifying_key().to_bytes();
    Ok(DecodingKey::from_ed_der(&pk))
}

pub(crate) async fn regenerate(pg: impl PgRef) {
    #[cfg(feature = "k8s")]
    async fn restart_k8s() -> Result<(), snafu::Whatever> {
        use k8s_openapi::api::apps::v1::Deployment;
        use kube::{Api, Client};

        let client = Client::try_default()
            .await
            .whatever_context("Failed to create kubernetes client instance")?;

        let deployments: Api<Deployment> = Api::namespaced(client, &common::AUTHELIA_NAMESPACE);

        deployments
            .restart(&common::AUTHELIA_DEPLOYMENT)
            .await
            .whatever_context("Failed to restart authelia's deployment")?;

        Ok(())
    }

    #[cfg(feature = "docker")]
    async fn restart_docker() -> Result<(), snafu::Whatever> {
        use bollard::Docker;

        let docker = Docker::connect_with_socket_defaults()
            .whatever_context("Failed to connect to docker socket. Did you mount it?")?;

        docker
            .restart_container(&common::AUTHELIA_CONTAINER, None)
            .await
            .whatever_context("Failed to restart authelia's container")?;

        Ok(())
    }

    async fn restart() -> Result<(), snafu::Whatever> {
        cfg_select! {
            not(any(feature = "docker", feature = "k8s")) => {
                compile_error!("Require either `docker` or `k8s`")
            }
            all(feature = "docker", feature = "k8s") => {
                if restart_k8s().await.is_err() {
                    restart_docker().await?;
                }

                Ok(())
            }
            feature = "docker" => restart_docker().await,
            feature = "k8s" => restart_k8s().await,
        }
    }

    async fn _impl(pg: impl PgRef) -> Result<(), AutheliaError> {
        info!("Regenerating authelia's configuration");

        let applications = sqlx::query_as!(
            Application,
            r#"
            SELECT
                a.slug,
                a.name,
                JSONB_BUILD_OBJECT(
                    'redirect_uris', o.redirect_uris,
                    'client_secret_hash', o.client_secret_hash
                ) AS "oidc!: Json<ApplicationOidc>"
            FROM applications a
            JOIN applications_oidc o ON o.slug = a.slug
            "#
        )
        .fetch_all(pg.as_ref())
        .await
        .context(DbSnafu)?;

        let path = &*common::AUTHELIA_CONFIG_DIR;
        let config = path.join("configuration.yml");

        let rsa = pem(path.join("rsa2048.pem"), make_rsa).await?;
        let ed25519 = pem(path.join("ed25519.pem"), make_ed25519).await?;

        let integration_key = if config.exists() && config.is_file() {
            let contents = fs::read_to_string(&config).await.context(FileReadSnafu {
                path: config.clone(),
            })?;

            let yaml = serde_yaml::to_value(&contents).context(ConfigParseSnafu {
                path: config.clone(),
            })?;

            yaml.get("authentication_backend")
                .context(MissingConfigKeySnafu)?
                .get("ldap")
                .context(MissingConfigKeySnafu)?
                .get("password")
                .context(MissingConfigKeySnafu)?
                .as_str()
                .context(MissingConfigKeySnafu)?
                .to_string()
        } else {
            let res = integration::create_impl(
                INTERNAL_INTEGRATION_ID.to_string(),
                INTERNAL_INTEGRATION_NAME.to_string(),
                AccessLevel::Read,
                pg.as_ref(),
            )
            .await
            .expect(
                "Failed to create internal integration (this shouldn't happen unless db issues)",
            );

            res.key.to_string()
        };

        let render = AutheliaConfigTemplate {
            applications,
            integration_key,
            rsa2048_private: rsa.as_str(),
            ed25519_private: ed25519.as_str(),
        }
        .render()
        .context(AskamaSnafu)?;

        fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&config)
            .await
            .context(FileOpenSnafu {
                path: config.clone(),
            })?
            .write_all(render.as_bytes())
            .await
            .context(FileWriteSnafu { path: config })?;

        restart().await.context(RestartSnafu)?;

        Ok(())
    }

    if let Err(res) = _impl(pg).await {
        error!(
            "Failed to regenerate authelia's config:\n{}",
            snafu::Report::from_error(&res).to_string()
        )
    }
}

pub(crate) struct AutheliaToken {
    pub(crate) uid: String,
    pub(crate) memberships: Vec<Membership>,
    pub(crate) issued: chrono::DateTime<Utc>,
}

impl AutheliaToken {
    pub(crate) async fn parse(token: impl AsRef<str>) -> Result<AutheliaToken, AuthError> {
        async fn _impl(token: impl AsRef<str>) -> Result<AutheliaToken, AutheliaError> {
            #[derive(Deserialize)]
            struct RawClaims {
                uid: String,
                #[serde(default)]
                groups: Vec<String>,
                iat: i64,
                client_id: String,
            }

            let mut v = Validation::new(Algorithm::EdDSA);
            v.set_issuer(&[AUTHELIA_URL.as_str()]);
            v.validate_aud = false;

            let key = verify_key().await?;

            let claims = jsonwebtoken::decode::<RawClaims>(token.as_ref(), &key, &v)
                .context(JwtDecodeSnafu)?
                .claims;

            snafu::ensure!(
                claims.client_id == common::xxh64_hex(DOMAIN_SLUG),
                InvalidClientIdSnafu {
                    found: claims.client_id
                }
            );

            Ok(AutheliaToken {
                uid: claims.uid,
                memberships: claims
                    .groups
                    .into_iter()
                    .filter_map(|g| Membership::from_str(g.as_str()).ok())
                    .collect(),
                issued: Utc
                    .timestamp_opt(claims.iat, 0)
                    .single()
                    .context(InvalidIatSnafu { found: claims.iat })?,
            })
        }

        _impl(token).await.context(AutheliaSnafu)
    }
}
