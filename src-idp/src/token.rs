use chrono::Utc;
use idp_proto::{app::Membership, error::Denied};
use transit_core::InternalError;

pub(crate) struct KeycloakToken {
    pub(crate) uid: String,
    pub(crate) username: String,
    pub(crate) email: String,
    pub(crate) memberships: Vec<Membership>,
    pub(crate) issued: chrono::DateTime<Utc>,
}

pub(crate) fn parse(
    token: impl AsRef<str>,
) -> Result<Result<KeycloakToken, Denied>, InternalError> {
    todo!("figure out how keycloak works")
}
