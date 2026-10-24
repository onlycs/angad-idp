use std::sync::Arc;

use idp_proto::{
    auth::{AccessLevel, Authentication},
    integration::*,
};
use snafu::prelude::*;
use sqlx::PgPool;
use transit_core::{InternalError, InternalErrorContext};

use crate::{
    auth::{self, AuthErrorCast},
    common::{self},
};

pub(crate) async fn create_impl(
    id: String,
    name: String,
    access: AccessLevel,
    pg: &PgPool,
) -> Result<IntegrationCreateResponse, IntegrationCreateError> {
    let name = name.trim();
    snafu::ensure!(!name.is_empty(), InvalidIntegrationName);

    let key = hex::encode(common::generate_secret()?);
    let hash = common::sha256(key.as_bytes());

    let result = sqlx::query!(
        r#"
        INSERT INTO integrations (id, name, key_hash, read_only)
        VALUES ($1, $2, $3, $4)
        ON CONFLICT DO NOTHING
        "#,
        id,
        name,
        hash,
        access == AccessLevel::Read,
    )
    .execute(pg)
    .await
    .context(InternalErrorContext!(via(display), common::ERROR_DB))?;

    snafu::ensure!(result.rows_affected() > 0, IntegrationNameInUse);

    Ok(IntegrationCreateResponse { id, key })
}

#[tracing::instrument(skip(pg))]
pub(crate) async fn create(
    IntegrationCreateRequest { auth, name, access }: IntegrationCreateRequest,
    pg: Arc<PgPool>,
) -> Result<IntegrationCreateResponse, IntegrationCreateError> {
    auth::authenticate(auth, AccessLevel::ReadWrite, &pg)
        .await
        .context(AuthErrorCast::<IntegrationCreateError>)?;

    create_impl(cuid2::cuid(), name, access, &pg).await
}

#[tracing::instrument(skip(pg))]
pub(crate) async fn list(
    auth: Authentication,
    pg: Arc<PgPool>,
) -> Result<Vec<Integration>, IntegrationListError> {
    auth::authenticate(auth, AccessLevel::Read, &pg)
        .await
        .context(AuthErrorCast::<IntegrationListError>)?;

    let integrations = sqlx::query!(
        r#"
        SELECT id, name, read_only
        FROM integrations
        WHERE id != $1
        "#,
        INTERNAL_INTEGRATION_ID
    )
    .fetch_all(&*pg)
    .await
    .context(InternalErrorContext!(via(display), common::ERROR_DB))?;

    Ok(integrations
        .into_iter()
        .map(|a| Integration {
            id: a.id,
            name: a.name,
            access: if a.read_only {
                AccessLevel::Read
            } else {
                AccessLevel::ReadWrite
            },
        })
        .collect())
}

#[tracing::instrument(skip(pg))]
pub(crate) async fn authorize(
    IntegrationAuthorizeRequest { key, level }: IntegrationAuthorizeRequest,
    pg: Arc<PgPool>,
) -> Result<(), IntegrationAuthorizeError> {
    auth::authenticate(Authentication::Integration(key), level, &pg)
        .await
        .context(AuthErrorCast::<IntegrationAuthorizeError>)?;

    Ok(())
}

#[tracing::instrument(skip(pg, auth))]
pub(crate) async fn update(
    IntegrationUpdateRequest {
        auth,
        id,
        name,
        access,
        roll_key,
    }: IntegrationUpdateRequest,
    pg: Arc<PgPool>,
) -> Result<IntegrationUpdateResponse, IntegrationUpdateError> {
    auth::authenticate(auth, AccessLevel::ReadWrite, &pg)
        .await
        .context(AuthErrorCast::<IntegrationUpdateError>)?;

    let name = name.as_ref().map(|n| n.trim());

    snafu::ensure!(id != INTERNAL_INTEGRATION_ID, NoIntegration);
    snafu::ensure!(name.is_none_or(|n| !n.is_empty()), InvalidIntegrationName);

    let key = if roll_key {
        Some(hex::encode(common::generate_secret()?))
    } else {
        None
    };

    let key_hash = key.as_ref().map(|k| common::sha256(k.as_bytes()));

    let integration_update = match sqlx::query!(
        r#"
        UPDATE integrations
        SET
            name = COALESCE($2, name),
            key_hash = COALESCE($3, key_hash),
            read_only = COALESCE($4, read_only)
        WHERE id = $1
        RETURNING *
        "#,
        id,
        name,
        key_hash,
        access.map(|a| a == AccessLevel::Read),
    )
    .fetch_one(&*pg)
    .await
    {
        Ok(res) => res,
        Err(sqlx::Error::RowNotFound) => {
            return Err(NoIntegration.into());
        }
        Err(other) => {
            return Err(InternalError!(ctx(display other), common::ERROR_DB).into());
        }
    };

    Ok(IntegrationUpdateResponse {
        integration: Integration {
            id,
            name: integration_update.name,
            access: if integration_update.read_only {
                AccessLevel::Read
            } else {
                AccessLevel::ReadWrite
            },
        },
        key,
    })
}

#[tracing::instrument(skip(pg, auth))]
pub(crate) async fn delete(
    IntegrationDeleteRequest { auth, id }: IntegrationDeleteRequest,
    pg: Arc<PgPool>,
) -> Result<(), IntegrationDeleteError> {
    auth::authenticate(auth, AccessLevel::ReadWrite, &pg)
        .await
        .context(AuthErrorCast::<IntegrationDeleteError>)?;

    snafu::ensure!(id != INTERNAL_INTEGRATION_ID, NoIntegration);

    let res = sqlx::query!(
        r#"
        DELETE FROM integrations
        WHERE id = $1
        "#,
        id,
    )
    .execute(&*pg)
    .await
    .context(InternalErrorContext!(via(display), common::ERROR_DB))?;

    if res.rows_affected() == 0 {
        return Err(NoIntegration.into());
    }

    Ok(())
}
