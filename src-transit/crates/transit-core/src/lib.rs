#![allow(clippy::expect_fun_call)]
#![feature(error_generic_member_access)]

#[cfg(all(target_family = "wasm", feature = "uniffi"))]
compile_error!("`wasm32` is not compatible with `uniffi`");

#[cfg(all(target_family = "wasm", feature = "server"))]
compile_error!("`wasm32` is not compatible with `server`");

#[cfg(feature = "uniffi")]
uniffi::setup_scaffolding!("transit_core");

#[cfg(feature = "client")]
#[cfg_attr(target_family = "wasm", path = "wasm32.rs")]
#[cfg_attr(not(target_family = "wasm"), path = "tcp.rs")]
mod arch;

#[cfg(feature = "client")]
pub mod client;

#[cfg(feature = "server")]
pub mod server;

pub mod frame;

pub use transit_macros::{error, error_shard, oneof, record, route};

pub trait Route {
    const ID: frame::RouteId;

    type Request: bitcode::Encode + bitcode::DecodeOwned + Send + Sync + 'static;
    type Response: bitcode::Encode + bitcode::DecodeOwned + Send + Sync + 'static;
}

#[error_shard("Internal server error")]
pub struct InternalError;
