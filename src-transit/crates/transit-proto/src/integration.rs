use transit_core::{error, record, route};

use crate::{auth::Authentication, error::Denied};

#[record]
pub struct Integration {
    pub id: String,
    pub name: String,
    pub readonly: bool,
}

#[record]
pub struct IntegrationCreateRequest {
    pub auth: Authentication,
    pub name: String,
    pub readonly: bool,
}

#[record]
pub struct IntegrationCreateResponse {
    pub id: String,
    pub key: String,
}

#[record]
pub struct IntegrationUpdateRequest {
    pub auth: Authentication,
    pub id: String,
    pub name: Option<String>,
    pub readonly: Option<bool>,
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
    IntegrationUpdateError = NoIntegration | InvalidIntegrationName | IntegrationNameInUse | Denied;
    IntegrationDeleteError = NoIntegration | Denied;
}

route! {
    IntegrationCreate(IntegrationCreateRequest) -> Result<IntegrationCreateResponse, IntegrationCreateError>;
    IntegrationList(Authentication) -> Result<Vec<Integration>, IntegrationListError>;
    IntegrationUpdate(IntegrationUpdateRequest) -> Result<Integration, IntegrationUpdateError>;
    IntegrationDelete(IntegrationDeleteRequest) -> Result<(), IntegrationDeleteError>;
}
