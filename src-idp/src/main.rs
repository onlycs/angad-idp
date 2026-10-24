#![feature(option_zip, error_generic_member_access, path_trailing_sep)]
#![allow(unexpected_cfgs)]

use std::sync::Arc;

use chrono::{TimeDelta, Utc};
use idp_proto::{
    app::{
        Application, ApplicationCreate, ApplicationCreateError, ApplicationDelete, ApplicationList,
        ApplicationOidc, ApplicationUpdate, Membership,
    },
    integration::{
        IntegrationAuthorize, IntegrationCreate, IntegrationDelete, IntegrationList,
        IntegrationUpdate,
    },
    invite::{InviteCreate, InviteCreateError, InviteDelete, InviteList, InviteUpdate, InviteUse},
    user::{CheckPassword, UpdatePassword, UserDelete, UserList, UserUpdate},
};
use snafu::{Location, ResultExt, Snafu};
use sqlx::PgPool;
use tracing::info;
use tracing_subscriber::{filter::Targets, layer::SubscriberExt, util::SubscriberInitExt};
use transit_core::server::{self, Router, TcpListenOptions, XwtListenOptions};
use wtransport::Identity;

mod app;
mod auth;
mod authelia;
mod common;
mod integration;
mod invite;
mod token;
mod user;

fn init_logger() {
    #[cfg(debug_assertions)]
    const LOG_LEVEL: tracing::Level = tracing::Level::DEBUG;
    #[cfg(not(debug_assertions))]
    const LOG_LEVEL: tracing::Level = tracing::Level::INFO;

    let fmt = tracing_subscriber::fmt::layer().pretty();

    let filter = Targets::new()
        .with_default(LOG_LEVEL)
        .with_target("sqlx", tracing::Level::INFO);

    tracing_subscriber::registry().with(filter).with(fmt).init();
}

#[derive(Snafu, Debug)]
pub enum DomainSetupError {
    #[snafu(display("At {location}: Failed to create internal application\n{source}"))]
    ApplicationCreate {
        source: ApplicationCreateError,
        #[snafu(implicit)]
        location: Location,
    },

    #[snafu(display("At {location}: Failed to send internal invite\n{source}"))]
    InviteSend {
        source: InviteCreateError,
        #[snafu(implicit)]
        location: Location,
    },
}

#[derive(Snafu, Debug)]
pub enum StartupError {
    #[snafu(display("At {location}: Failed to set up domain\n{source}"))]
    DomainSetup {
        source: DomainSetupError,
        #[snafu(implicit)]
        location: Location,
    },

    #[snafu(display("At {location}: Database error\n{source}"))]
    Db {
        source: sqlx::Error,
        #[snafu(implicit)]
        location: Location,
    },

    #[snafu(display("At {location}: Migration error\n{source}"))]
    Migrate {
        source: sqlx::migrate::MigrateError,
        #[snafu(implicit)]
        location: Location,
    },

    #[snafu(display("At {location}: Listen error\n{source}"))]
    Listen {
        source: transit_core::server::ListenError,
        #[snafu(implicit)]
        location: Location,
    },
}

async fn setup_domain(pg: &PgPool) -> Result<(), DomainSetupError> {
    match app::create_impl(
        Application {
            slug: app::DOMAIN_SLUG.to_string(),
            name: app::DOMAIN_NAME.to_string(),
            url: format!("{}/", *common::FRONTEND_URL),
            oidc: Some(ApplicationOidc {
                redirect_uris: vec![format!("{}/", *common::FRONTEND_URL)],
                public: true,
            }),
        },
        pg,
    )
    .await
    {
        Ok(_) => {}
        Err(ApplicationCreateError::AppNameInUse { .. }) => return Ok(()),
        Err(ApplicationCreateError::SlugInUse { .. }) => return Ok(()),
        other => {
            other.context(ApplicationCreateSnafu)?;
        }
    }

    let invite = invite::create_impl(
        common::INITIAL_ADMIN_EMAIL.clone(),
        vec![Membership {
            slug: app::DOMAIN_SLUG.to_string(),
            admin: true,
        }],
        Utc::now() + TimeDelta::weeks(1),
        pg,
    )
    .await
    .context(InviteSendSnafu)?;

    info!(
        email = invite.email,
        id = invite.id,
        "Initial admin invite sent"
    );

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), StartupError> {
    init_logger();
    dotenvy::dotenv().ok();
    common::env_check_all();

    let pool = sqlx::PgPool::connect(common::DATABASE_URL.as_str())
        .await
        .context(DbSnafu)?;

    sqlx::migrate!().run(&pool).await.context(MigrateSnafu)?;

    setup_domain(&pool).await.context(DomainSetupSnafu)?;

    let router = Router::new(pool)
        // Application CRUD
        .route::<ApplicationCreate, _>(app::create)
        .route::<ApplicationList, _>(app::list)
        .route::<ApplicationUpdate, _>(app::update)
        .route::<ApplicationDelete, _>(app::delete)
        // Integration CRUD
        .route::<IntegrationCreate, _>(integration::create)
        .route::<IntegrationList, _>(integration::list)
        .route::<IntegrationAuthorize, _>(integration::authorize)
        .route::<IntegrationUpdate, _>(integration::update)
        .route::<IntegrationDelete, _>(integration::delete)
        // Invite CRUD
        .route::<InviteCreate, _>(invite::create)
        .route::<InviteList, _>(invite::list)
        .route::<InviteUpdate, _>(invite::update)
        .route::<InviteDelete, _>(invite::delete)
        .route::<InviteUse, _>(invite::redeem)
        // User CRUD
        .route::<CheckPassword, _>(user::check_password)
        .route::<UserList, _>(user::list)
        .route::<UserUpdate, _>(user::update)
        .route::<UpdatePassword, _>(user::update_password)
        .route::<UserDelete, _>(user::delete)
        .build();

    let tcp = tokio::spawn(server::listen_tcp_tls(
        TcpListenOptions {
            addr: &*common::LISTEN_ADDRESS,
            port: *common::TRANSIT_PORT,
            tls: None,
        },
        Arc::clone(&router),
    ));

    let xwt = tokio::spawn(server::listen_xwt(
        XwtListenOptions {
            addr: &*common::LISTEN_ADDRESS,
            port: *common::WT_PORT,
            path: common::WT_PATH.clone(),
            tls: Identity::self_signed(["localhost", "127.0.0.1"]).expect("Invalid SAN"),
        },
        router,
    ));

    let (left, right) = tokio::join!(tcp, xwt);
    left.unwrap().unwrap();
    right.unwrap().unwrap();

    Ok(())
}
