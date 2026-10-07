#![feature(error_generic_member_access, duration_constructors)]

use std::env;

use idp_proto::{
    app::{ApplicationCreate, ApplicationDelete, ApplicationList, ApplicationUpdate},
    auth::Authenticate,
};
use snafu::ResultExt;
use tracing_subscriber::{filter::Targets, layer::SubscriberExt, util::SubscriberInitExt};
use transit_core::{
    InternalError, InternalSnafu,
    server::{self, Router, TcpListenOptions},
};

mod app;
mod auth;
mod strings;
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

#[tokio::main]
async fn main() -> Result<(), InternalError> {
    init_logger();
    dotenvy::dotenv().ok();

    let pool = sqlx::PgPool::connect(&env::var(strings::ENV_DATABSE_URL).context(InternalSnafu)?)
        .await
        .context(InternalSnafu)?;

    sqlx::migrate!().run(&pool).await.context(InternalSnafu)?;

    let router = Router::new(pool)
        .route::<ApplicationCreate, _>(app::create)
        .route::<ApplicationList, _>(app::list)
        .route::<ApplicationUpdate, _>(app::update)
        .route::<ApplicationDelete, _>(app::delete)
        .route::<Authenticate, _>(auth::route)
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
