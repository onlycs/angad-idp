use transit_core::{error, record, route};

use crate::{app::Application, error::Denied, user::User};

#[record]
pub struct LdapQueryRequest {
    pub integration: String,
    pub query: String,
}

#[record]
pub struct LdapQueryResponse {
    pub users: Vec<User>,
    pub applications: Vec<Application>,
}

error! {
    InvalidQuery("Could not parse query");
    LdapQueryError = InvalidQuery | Denied;
}

route! {
    LdapQuery(LdapQueryRequest) -> Result<LdapQueryResponse, LdapQueryError>;
}
