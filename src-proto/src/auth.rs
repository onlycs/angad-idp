use std::ops::Deref;

use transit_core::{error, oneof, record, route};

use crate::user::{NoUser, UserQuery};

#[oneof]
#[derive(Copy, PartialEq, Eq)]
pub enum AccessLevel {
    Read,
    ReadWrite,
}

#[record]
pub struct TokenRaw {
    pub uid: String,
    pub username: String,
    pub email: String,
    /// Unix timestamp in milliseconds
    pub exp: u64,
}

#[record]
pub struct Token {
    pub raw: TokenRaw,
    pub sig: Vec<u8>,
}

impl Deref for Token {
    type Target = TokenRaw;

    fn deref(&self) -> &Self::Target {
        &self.raw
    }
}

#[oneof]
pub enum Authentication {
    Token(Token),
    Integration(String),
}

#[oneof]
pub enum AuthenticationStrict {
    Password { uid: String, password: String },
    Integration(String),
}

#[record]
pub struct AuthenticateRequest {
    pub query: UserQuery,
    pub password: String,
}

error! {
    BadPassword("Incorrect password");
    AuthenticateError = NoUser | BadPassword;
}

route! {
    Authenticate(AuthenticateRequest) -> Result<Token, AuthenticateError>;
}
