use std::{io, path::PathBuf, str::FromStr};

use askama::Template;
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use chrono::{TimeZone, Utc};
use ed25519_dalek::{
    SigningKey,
    pkcs8::{DecodePrivateKey, EncodePrivateKey as EdEncode},
};
use getrandom::{SysRng, rand_core::UnwrapErr};
use idp_proto::{
    app::{GROUP_SUFFIX_ADMIN, GROUP_SUFFIX_USER, Membership},
    auth::AccessLevel,
    error::Denied,
    integration::{INTERNAL_INTEGRATION_ID, INTERNAL_INTEGRATION_NAME},
};
use jsonwebtoken::{Algorithm, DecodingKey, Validation, crypto};
use rsa::{
    RsaPrivateKey,
    pkcs8::{EncodePrivateKey as RsaEncode, LineEnding},
    rand_core::OsRng,
};
use serde::Deserialize;
use serde_yaml::Value;
use snafu::{Location, OptionExt, ResultExt, Snafu};
use sqlx::types::Json;
use tokio::{fs, io::AsyncWriteExt};
use tracing::{error, info};
use transit_core::InternalError;

use crate::{
    app,
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
            .map(|(n, line)| format!("{}{line}", "  ".repeat(indent * n.min(1))))
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

    #[snafu(display("At {location}: Failed to read authelia's yaml\n{source}"))]
    AutheliaRead {
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

    #[snafu(display("At {location}: Failed to parse authelia's yaml\n{source}"))]
    ConfigParse {
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

    #[snafu(display("At {location}: Malformed or invalid JWT"))]
    JwtMalformed {
        #[snafu(implicit)]
        location: Location,
    },

    #[snafu(display("At {location}: JWT signature or algorithm mismatch"))]
    JwtInvalid {
        #[snafu(implicit)]
        location: Location,
    },

    #[snafu(display("At {location}: Failed to verify JWT\n{source}"))]
    JwtVerify {
        source: jsonwebtoken::errors::Error,
        #[snafu(implicit)]
        location: Location,
    },

    #[snafu(display("At {location}: Base64 decode error\n{source}"))]
    Base64Decode {
        source: base64::DecodeError,
        #[snafu(implicit)]
        location: Location,
    },

    #[snafu(display("At {location}: JSON decode error\n{source}"))]
    JsonDecode {
        source: serde_json::Error,
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
            AutheliaError::JwtMalformed { .. }
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

        let dir = &*common::AUTHELIA_CONFIG_DIR;
        let path = dir.join("configuration.yml");

        let rsa = pem(dir.join("rsa2048.pem"), make_rsa).await?;
        let ed25519: String = pem(dir.join("ed25519.pem"), make_ed25519).await?;

        let integration_key = if path.exists() && path.is_file() {
            let contents = fs::read_to_string(&path).await.context(AutheliaReadSnafu)?;
            let parsed = serde_yaml::from_str::<Value>(&contents).context(ConfigParseSnafu)?;

            parsed
                .get("authentication_backend")
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
            .open(&path)
            .await
            .context(FileOpenSnafu { path: path.clone() })?
            .write_all(render.as_bytes())
            .await
            .context(FileWriteSnafu { path })?;

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

#[derive(Debug)]
pub(crate) struct AutheliaToken {
    pub(crate) uid: String,
    pub(crate) memberships: Vec<Membership>,
    pub(crate) issued: chrono::DateTime<Utc>,
}

impl AutheliaToken {
    pub(crate) async fn parse(token: impl AsRef<str>) -> Result<AutheliaToken, AuthError> {
        async fn _impl(token: impl AsRef<str>) -> Result<AutheliaToken, AutheliaError> {
            #[derive(Deserialize)]
            struct RawHeader {
                alg: String,
            }

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

            let token = token.as_ref();

            let (message, sig) = token.rsplit_once('.').context(JwtMalformedSnafu)?;
            let (header_b64, payload_b64) = message.split_once('.').context(JwtMalformedSnafu)?;
            snafu::ensure!(!payload_b64.contains('.'), JwtMalformedSnafu);

            let header = serde_json::from_slice::<RawHeader>(
                &URL_SAFE_NO_PAD
                    .decode(header_b64)
                    .context(Base64DecodeSnafu)?,
            )
            .context(JsonDecodeSnafu)?;

            snafu::ensure!(
                matches!(header.alg.as_str(), "EdDSA" | "Ed25519"),
                JwtInvalidSnafu
            );

            let key = verify_key().await?;
            let valid = crypto::verify(sig, message.as_bytes(), &key, Algorithm::EdDSA)
                .context(JwtVerifySnafu)?;

            snafu::ensure!(valid, JwtInvalidSnafu);

            let claims = serde_json::from_slice::<RawClaims>(
                &URL_SAFE_NO_PAD
                    .decode(payload_b64)
                    .context(Base64DecodeSnafu)?,
            )
            .context(JsonDecodeSnafu)?;

            snafu::ensure!(
                claims.client_id == common::xxh64_hex(app::DOMAIN_SLUG),
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

        let res = _impl(token).await;
        if let Err(e) = &res {
            error!(
                "Failed to parse authelia JWT:\n{}",
                snafu::Report::from_error(e).to_string()
            )
        }

        res.context(AutheliaSnafu)
    }

    pub(crate) fn is_domain_admin(&self) -> bool {
        self.memberships
            .iter()
            .any(|ship| ship.slug == app::DOMAIN_SLUG && ship.admin)
    }
}
