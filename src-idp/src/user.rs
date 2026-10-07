use idp_proto::user::UserQuery;
use snafu::ResultExt;
use sqlx::{
    PgPool,
    types::chrono::{self, Utc},
};
use transit_core::{InternalError, InternalErrorContext};

use crate::common;

#[derive(Clone, Debug, sqlx::FromRow)]
pub(crate) struct UserDb {
    pub(crate) id: String,
    pub(crate) username: String,
    pub(crate) email: String,
    pub(crate) password_hash: String,
    pub(crate) created_at: chrono::DateTime<Utc>,
    pub(crate) updated_at: chrono::DateTime<Utc>,
}

pub(crate) async fn query(uq: &UserQuery, pg: &PgPool) -> Result<Option<UserDb>, InternalError> {
    let mut q = sqlx::QueryBuilder::new("SELECT * FROM users WHERE ");

    match uq {
        UserQuery::Uid(uid) => q.push("uid = $1").push_bind(uid),
        UserQuery::Username(username) => q.push("username = $1").push_bind(username),
        UserQuery::Email(email) => q.push("email = $1").push_bind(email),
    };

    let result = q
        .build_query_as::<UserDb>()
        .fetch_optional(pg)
        .await
        .context(InternalErrorContext!(via(display), common::ERROR_DB))?;

    Ok(result)
}
