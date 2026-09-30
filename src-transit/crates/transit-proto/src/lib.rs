#![feature(error_generic_member_access)]

#[cfg(all(target_family = "wasm", feature = "uniffi"))]
compile_error!("`wasm32` is not compatible with `uniffi`");

#[cfg(feature = "uniffi")]
uniffi::setup_scaffolding!("proto");

pub mod app;
pub mod auth;
pub mod error;
pub mod integration;
pub mod invite;
pub mod ldap;
pub mod user;
