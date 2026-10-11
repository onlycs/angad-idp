use transit_core::{error, oneof, record, route};

use crate::{
    app::{Membership, NoApplication},
    auth::{Authentication, AuthenticationStrict},
    error::Denied,
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
    /// Username, Email, or ID
    pub unknown_identity: String,
    pub password: String,
}

#[record]
pub struct UserUpdateRequest {
    pub auth: Authentication,
    pub id: String,
    pub username: Option<String>,
    pub email: Option<String>,
    pub memberships: Option<Vec<Membership>>,
}

#[record]
pub struct UpdatePasswordRequest {
    pub auth: AuthenticationStrict,
    pub id: String,
    pub password: String,
}

#[oneof]
pub enum UserDeleteRequest {
    DeleteSelf { uid: String, password: String },
    DeleteOther { auth: Authentication, uid: String },
}

error! {
    NoUser("User not found");

    InvalidEmail("Invalid email");
    InvalidUsername("Invalid username");
    EmailInUse("Email in use");
    UsernameInUse("Username in use");
    IncorrectPassword("Incorrect password");

    CheckPasswordError = NoUser | IncorrectPassword;
    UserListError = Denied;
    UserSearchError = NoUser | Denied;
    UserUpdateError = NoUser | InvalidUsername | InvalidEmail | UsernameInUse | EmailInUse | NoApplication | Denied;
    UpdatePasswordError = NoUser | InsecurePassword | Denied;
    UserDeleteError = NoUser | Denied;
}

route! {
    CheckPassword(CheckPasswordRequest) -> Result<(), CheckPasswordError>;
    UserList(Authentication) -> Result<Vec<User>, UserListError>;
    UserUpdate(UserUpdateRequest) -> Result<User, UserUpdateError>;
    UpdatePassword(UpdatePasswordRequest) -> Result<(), UpdatePasswordError>;
    UserDelete(UserDeleteRequest) -> Result<(), UserDeleteError>;
}
