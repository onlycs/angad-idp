use transit_core::{error, record, route};

use crate::{
    app::{Membership, NoApplication},
    auth::Authentication,
    error::Denied,
    user::{EmailInUse, InvalidEmail, InvalidUsername, User, UsernameInUse},
};

#[record]
pub struct Invite {
    pub token: String,
}

#[record]
pub struct InviteCreateRequest {
    pub auth: Authentication,
    pub mailto: String,
    pub memberships: Vec<Membership>,
    /// Unix timestamp in milliseconds
    pub exp: u64,
}

#[record]
pub struct InviteUpdateRequest {
    pub auth: Authentication,
    pub token: String,
    pub memberships: Option<Vec<Membership>>,
    /// Unix timestamp in milliseconds
    pub exp: Option<u64>,
}

#[record]
pub struct InviteDeleteRequest {
    pub auth: Authentication,
    pub token: String,
}

#[record]
pub struct InviteUseRequest {
    pub token: String,
    pub username: String,
    pub password: String,
}

error! {
    InvalidExpiry("Expiry set to before now");
    NoInvite("Invite not found");
    InsecurePassword("Password is too weak") {
        warning: String,
        suggestions: Vec<String>
    };

    InviteCreateError = InvalidExpiry | InvalidEmail | EmailInUse | NoApplication | Denied;
    InviteUpdateError = InvalidExpiry | NoApplication | Denied;
    InviteListError = Denied;
    InviteDeleteError = NoInvite | Denied;
    InviteUseError = InvalidUsername | UsernameInUse | InsecurePassword | Denied;
}

route! {
    InviteCreate(InviteCreateRequest) -> Result<Invite, InviteCreateError>;
    InviteList(Authentication) -> Result<Vec<Invite>, InviteListError>;
    InviteUpdate(InviteUpdateRequest) -> Result<Invite, InviteUpdateError>;
    InviteDelete(InviteDeleteRequest) -> Result<(), InviteDeleteError>;
    InviteUse(InviteUseRequest) -> Result<User, InviteUseError>;
}
