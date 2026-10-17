use std::io;

use askama::Template;
use snafu::{Location, ResultExt, Snafu};
use sqlx::types::Json;
use tokio::{fs, io::AsyncWriteExt};

use crate::common::{self, AUTHELIA_DOMAIN, FRONTEND_URL, PgRef};

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
}

pub(crate) async fn regenerate(pg: impl PgRef) -> Result<(), RegenerateError> {
    let apps = sqlx::query_as!(
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

    let config = AutheliaConfigTemplate { applications: apps };
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

    // TODO: trigger restart?

    Ok(())
}
