use transit_core::{error, record, route};

use crate::{
    auth::{AccessLevel, Authentication},
    error::Denied,
};

pub const INTERNAL_INTEGRATION_ID: &str = "a00000000000000000000000";
pub const INTERNAL_INTEGRATION_NAME: &str = "$internal";

#[record]
pub struct Integration {
    pub id: String,
    pub name: String,
    pub access: AccessLevel,
}

#[record]
pub struct IntegrationCreateRequest {
    pub auth: Authentication,
    pub name: String,
    pub access: AccessLevel,
}

#[record]
pub struct IntegrationCreateResponse {
    pub id: String,
    pub key: String,
}

#[record]
pub struct IntegrationAuthorizeRequest {
    pub key: String,
    pub level: AccessLevel,
}

#[record]
pub struct IntegrationUpdateRequest {
    pub auth: Authentication,
    pub id: String,
    pub name: Option<String>,
    pub access: Option<AccessLevel>,
    pub roll_key: bool,
}

#[record]
pub struct IntegrationUpdateResponse {
    pub integration: Integration,
    /// Some if and only if `roll_key` is `true`.
    pub key: Option<String>,
}

#[record]
pub struct IntegrationDeleteRequest {
    pub auth: Authentication,
    pub id: String,
}

error! {
    NoIntegration("Integration not found");

    InvalidIntegrationName("Invalid name");
    IntegrationNameInUse("Name in use");

    IntegrationCreateError = InvalidIntegrationName | IntegrationNameInUse | Denied;
    IntegrationListError = Denied;
    IntegrationAuthorizeError = Denied;
    IntegrationUpdateError = NoIntegration | InvalidIntegrationName | IntegrationNameInUse | Denied;
    IntegrationDeleteError = NoIntegration | Denied;
}

route! {
    IntegrationCreate(IntegrationCreateRequest) -> Result<IntegrationCreateResponse, IntegrationCreateError>;
    IntegrationList(Authentication) -> Result<Vec<Integration>, IntegrationListError>;
    IntegrationAuthorize(IntegrationAuthorizeRequest) -> Result<(), IntegrationAuthorizeError>;
    IntegrationUpdate(IntegrationUpdateRequest) -> Result<IntegrationUpdateResponse, IntegrationUpdateError>;
    IntegrationDelete(IntegrationDeleteRequest) -> Result<(), IntegrationDeleteError>;
}
