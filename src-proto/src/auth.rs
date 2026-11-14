use transit_core::{oneof, record};

#[oneof]
#[derive(Copy, PartialEq, Eq)]
pub enum AccessLevel {
    Read,
    ReadWrite,
}

#[oneof]
pub enum Authentication {
    Token(String),
    Integration(String),
}

#[record]
pub struct AuthenticateSelfStrict {
    pub uid: String,
    pub password: String,
}
