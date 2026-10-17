#![feature(option_zip)]

use chrono::{TimeDelta, Utc};
use idp_proto::{
    app::{
        Application, ApplicationCreate, ApplicationCreateError, ApplicationDelete, ApplicationList,
        ApplicationOidc, ApplicationUpdate, Membership,
    },
    integration::{IntegrationCreate, IntegrationDelete, IntegrationList, IntegrationUpdate},
    invite::{InviteCreate, InviteDelete, InviteList, InviteUpdate, InviteUse},
    user::{CheckPassword, UpdatePassword, UserDelete, UserList, UserUpdate},
};
use snafu::ResultExt;
use sqlx::PgPool;
use tracing::info;
use tracing_subscriber::{filter::Targets, layer::SubscriberExt, util::SubscriberInitExt};
use transit_core::{
    InternalError, InternalSnafu,
    server::{self, Router, TcpListenOptions},
};

mod app;
mod auth;
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

async fn setup_domain(pg: &PgPool) -> Result<(), InternalError> {
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
        Err(failure) => {
            return Err(InternalError!(
                ctx(failure),
                "Creating internal application failed"
            ));
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
    .context(InternalSnafu)?;

    info!(
        email = invite.email,
        id = hex::encode(invite.id),
        "Initial admin invite sent"
    );

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), InternalError> {
    init_logger();
    dotenvy::dotenv().ok();
    common::env_check_all();

    let pool = sqlx::PgPool::connect(common::DATABASE_URL.as_str())
        .await
        .context(InternalSnafu)?;

    sqlx::migrate!().run(&pool).await.context(InternalSnafu)?;

    setup_domain(&pool).await?;

    let router = Router::new(pool)
        // Application CRUD
        .route::<ApplicationCreate, _>(app::create)
        .route::<ApplicationList, _>(app::list)
        .route::<ApplicationUpdate, _>(app::update)
        .route::<ApplicationDelete, _>(app::delete)
        // Integration CRUD
        .route::<IntegrationCreate, _>(integration::create)
        .route::<IntegrationList, _>(integration::list)
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

    server::listen_tcp_tls(
        TcpListenOptions {
            addr: "0.0.0.0",
            port: 23849,
            tls: None,
        },
        router,
    )
    .await
    .context(InternalSnafu)?;

    Ok(())
}
