use transit_core::oneof;

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

#[oneof]
pub enum AuthenticationStrict {
    Password { uid: String, password: String },
    Integration(String),
}
