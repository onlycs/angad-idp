use transit_core::{error, record, route};

use crate::{
    app::{Membership, NoApplication},
    auth::Authentication,
    error::Denied,
    user::{EmailInUse, InvalidEmail, InvalidUsername, User, UsernameInUse},
};

#[record]
pub struct Invite {
    pub id: String,
    pub email: String,
    pub memberships: Vec<Membership>,
    pub exp: u64,
}

#[record]
pub struct InviteCreateRequest {
    pub auth: Authentication,
    pub email: String,
    pub memberships: Vec<Membership>,
    /// Unix timestamp in milliseconds
    pub exp: u64,
}

#[record]
pub struct InviteUpdateRequest {
    pub auth: Authentication,
    pub id: String,
    pub memberships: Option<Vec<Membership>>,
    /// Unix timestamp in milliseconds
    pub exp: Option<u64>,
}

#[record]
pub struct InviteDeleteRequest {
    pub auth: Authentication,
    pub id: String,
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
    InviteExpired("Invite expired");
    EmailSend("Failed to send email");
    InsecurePassword("Password is too weak") {
        warning: String,
        suggestions: Vec<String>
    };

    InviteCreateError = InvalidExpiry | InvalidEmail | EmailInUse | EmailSend | NoApplication | Denied;
    InviteUpdateError = InvalidExpiry | NoInvite | NoApplication | Denied;
    InviteListError = Denied;
    InviteDeleteError = NoInvite | Denied;
    InviteUseError = NoInvite | InviteExpired | InvalidUsername | UsernameInUse | InsecurePassword;
}

route! {
    InviteCreate(InviteCreateRequest) -> Result<Invite, InviteCreateError>;
    InviteList(Authentication) -> Result<Vec<Invite>, InviteListError>;
    InviteUpdate(InviteUpdateRequest) -> Result<Invite, InviteUpdateError>;
    InviteDelete(InviteDeleteRequest) -> Result<(), InviteDeleteError>;
    InviteUse(InviteUseRequest) -> Result<User, InviteUseError>;
}
