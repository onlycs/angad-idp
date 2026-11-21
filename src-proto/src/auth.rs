use transit_core::{error, oneof, record};

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

#[oneof]
pub enum DeniedReason {
    NoAccess,
    Invalid,
    Expired,
}

error! {
    Denied("Permission denied") { reason: DeniedReason };
}
