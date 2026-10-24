use std::io;

use askama::Template;
use idp_proto::{
    auth::AccessLevel,
    integration::{INTERNAL_INTEGRATION_ID, INTERNAL_INTEGRATION_NAME},
};
use snafu::{Location, OptionExt, ResultExt, Snafu};
use sqlx::types::Json;
use tokio::{fs, io::AsyncWriteExt};

use crate::{
    common::{self, AUTHELIA_DOMAIN, FRONTEND_URL, PgRef},
    integration,
};

mod filters {
    use std::fmt;

    #[askama::filter_fn]
    pub(crate) fn xxh64(
        value: &dyn fmt::Display,
        _: &dyn askama::Values,
    ) -> askama::Result<String> {
        Ok(hex::encode(
            xxhash_rust::xxh3::xxh3_64(value.to_string().as_bytes()).to_le_bytes(),
        ))
    }
}

#[derive(Snafu, Debug)]
pub(crate) enum RegenerateError {
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

    #[snafu(display("At {location}: Failed to open `{:?}`\n{source}", &*common::AUTHELIA_CONFIG_PATH))]
    FileOpen {
        source: io::Error,
        #[snafu(implicit)]
        location: Location,
    },

    #[snafu(display("At {location}: Failed to write to `{:?}`\n{source}", &*common::AUTHELIA_CONFIG_PATH))]
    FileWrite {
        source: io::Error,
        #[snafu(implicit)]
        location: Location,
    },

    #[snafu(display("At {location}: Failed to read from `{:?}`\n{source}", &*common::AUTHELIA_CONFIG_PATH))]
    FileRead {
        source: io::Error,
        #[snafu(implicit)]
        location: Location,
    },

    #[snafu(display("At {location}: Failed to parse yaml from `{:?}`\n{source}", &*common::AUTHELIA_CONFIG_PATH))]
    FileParse {
        source: serde_yaml::Error,
        #[snafu(implicit)]
        location: Location,
    },

    #[snafu(display("At {location}: Missing string `authentication_backend.ldap.password` in `{:?}`", &*common::AUTHELIA_CONFIG_PATH))]
    SubpathMissing {
        #[snafu(implicit)]
        location: Location,
    },

    #[snafu(display("At {location}: Failed to restart authelia:\n{source}"))]
    Restart {
        source: snafu::Whatever,
        #[snafu(implicit)]
        location: Location,
    },
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
struct AutheliaConfigTemplate {
    applications: Vec<Application>,
    integration_key: String,
}

pub(crate) async fn regenerate(pg: impl PgRef) -> Result<(), RegenerateError> {
    let applications = sqlx::query_as!(
        Application,
        r#"
        SELECT
            a.slug,
            a.name,
            COALESCE(
                JSONB_AGG(
                    JSONB_BUILD_OBJECT(
                        'redirect_uris', o.redirect_uris,
                        'client_secret_hash', o.client_secret_hash
                    )
                ),
                '{}'::jsonb
            ) AS "oidc: Json<ApplicationOidc>"
        FROM applications a
        LEFT JOIN applications_oidc o ON o.slug = a.slug
        GROUP BY a.slug, a.name;
        "#
    )
    .fetch_all(pg.as_ref())
    .await
    .context(DbSnafu)?;

    let path = &*common::AUTHELIA_CONFIG_PATH;
    let integration_key = if path.exists() && path.is_file() {
        let contents = fs::read_to_string(path).await.context(FileReadSnafu)?;
        let yaml = serde_yaml::to_value(&contents).context(FileParseSnafu)?;

        yaml.get("authentication_backend")
            .context(SubpathMissingSnafu)?
            .get("ldap")
            .context(SubpathMissingSnafu)?
            .get("password")
            .context(SubpathMissingSnafu)?
            .as_str()
            .context(SubpathMissingSnafu)?
            .to_string()
    } else {
        let res = integration::create_impl(
            INTERNAL_INTEGRATION_ID.to_string(),
            INTERNAL_INTEGRATION_NAME.to_string(),
            AccessLevel::Read,
            pg.as_ref(),
        )
        .await
        .expect("Failed to create internal integration (this shouldn't happen unless db issues)");

        res.key.to_string()
    };

    let config = AutheliaConfigTemplate {
        applications,
        integration_key,
    };
    let render = config.render().context(AskamaSnafu)?;

    fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&*common::AUTHELIA_CONFIG_PATH)
        .await
        .context(FileOpenSnafu)?
        .write_all(render.as_bytes())
        .await
        .context(FileWriteSnafu)?;

    restart().await.context(RestartSnafu)?;

    Ok(())
}

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
