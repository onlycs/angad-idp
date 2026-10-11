use chrono::{TimeDelta, Utc};
use idp_proto::{
    app::{
        ApplicationCreate, ApplicationDelete, ApplicationList, ApplicationUpdate, DOMAIN_SLUG,
        Membership,
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
    sqlx::query!(
        r#"
        INSERT INTO applications
        VALUES ($1, 'Angad IdP', $2)
        ON CONFLICT DO NOTHING
        "#,
        DOMAIN_SLUG,
        format!("{}/", &*common::FRONTEND_URL)
    )
    .execute(pg)
    .await
    .context(InternalSnafu)?;

    let domain_adm = sqlx::query!(
        r#"
        SELECT u.id, u.username FROM roles r
        JOIN users u ON r.uid = u.id
        WHERE r.application = $1 AND r.admin = true
        LIMIT 1
        "#,
        DOMAIN_SLUG
    )
    .fetch_optional(pg)
    .await
    .context(InternalSnafu)?;

    if let Some(adm) = domain_adm {
        info!(id = adm.id, username = adm.username, "Found domain admin");
        return Ok(());
    }

    info!("No domain admin, creating...");

    invite::create_impl(
        common::INITIAL_ADMIN_EMAIL.clone(),
        vec![Membership {
            slug: DOMAIN_SLUG.to_string(),
            admin: true,
        }],
        Utc::now() + TimeDelta::weeks(1),
        pg,
    )
    .await
    .context(InternalSnafu)?;

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
