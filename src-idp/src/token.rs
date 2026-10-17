use chrono::Utc;
use idp_proto::app::Membership;

use crate::auth::AuthError;

pub(crate) struct AutheliaToken {
    pub(crate) uid: String,
    pub(crate) username: String,
    pub(crate) email: String,
    pub(crate) memberships: Vec<Membership>,
    pub(crate) issued: chrono::DateTime<Utc>,
}

pub(crate) fn parse(token: impl AsRef<str>) -> Result<AutheliaToken, AuthError> {
    todo!("figure out how authelia works")
}
