use transit_core::{error, oneof, record, route};

use crate::{
    app::{Membership, NoApplication},
    auth::{AuthenticateSelfStrict, Authentication, Denied},
    invite::InsecurePassword,
};

#[record]
pub struct User {
    pub id: String,
    pub username: String,
    pub email: String,
    pub memberships: Vec<Membership>,
}

#[record]
pub struct CheckPasswordRequest {
    /// Integration key
    pub token: String,
    /// Username, Email, or ID
    pub unknown_identity: String,
    pub password: String,
}

#[oneof]
pub enum UserUpdateRequest {
    UpdateSelf {
        auth: AuthenticateSelfStrict,
        username: Option<String>,
        email: Option<String>,
        password: Option<String>,
    },
    UpdateOther {
        auth: Authentication,
        uid: String,
        memberships: Vec<Membership>,
    },
}

#[oneof]
pub enum UserDeleteRequest {
    DeleteSelf { auth: AuthenticateSelfStrict },
    DeleteOther { auth: Authentication, uid: String },
}

error! {
    NoUser("User not found");

    InvalidEmail("Invalid email");
    InvalidUsername("Invalid username");
    EmailInUse("Email in use");
    UsernameInUse("Username in use");
    IncorrectPassword("Incorrect password");
    SelfDeprivilege("You cannot remove your own domain admin privileges");
    LastDomainAdmin("Cannot delete the last domain admin");

    CheckPasswordError = NoUser | IncorrectPassword | Denied;
    UserListError = Denied;
    UserUpdateError = NoUser | InvalidUsername | InvalidEmail | InsecurePassword | UsernameInUse | EmailInUse | NoApplication | SelfDeprivilege | Denied;
    UserDeleteError = NoUser | LastDomainAdmin | Denied;
}

route! {
    CheckPassword(CheckPasswordRequest) -> Result<(), CheckPasswordError>;
    UserList(Authentication) -> Result<Vec<User>, UserListError>;
    UserUpdate(UserUpdateRequest) -> Result<(), UserUpdateError>;
    UserDelete(UserDeleteRequest) -> Result<(), UserDeleteError>;
}
