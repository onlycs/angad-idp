use std::env;

use hmac::{Hmac, KeyInit, Mac};
use idp_proto::{
    auth::{Token, TokenRaw},
    error::Denied,
};
use sha2::Sha256;
use snafu::{Location, prelude::*};
use transit_core::TransitErrorContext;

use crate::strings::ENV_SECRET;

#[derive(Snafu, Debug)]
pub(crate) enum TokenError {
    #[snafu(display("{ENV_SECRET} not set"))]
    Env {
        source: env::VarError,
        #[snafu(implicit)]
        location: Location,
    },

    #[snafu(display("{ENV_SECRET} invalid"))]
    InvalidHex {
        source: hex::FromHexError,
        #[snafu(implicit)]
        location: Location,
    },

    #[snafu(display("{ENV_SECRET} invalid"))]
    InvalidHmac {
        source: hmac::digest::InvalidLength,
        #[snafu(implicit)]
        location: Location,
    },
}

fn token_secret() -> Result<Vec<u8>, TokenError> {
    hex::decode(env::var(ENV_SECRET).context(EnvSnafu)?).context(InvalidHexSnafu)
}

pub(crate) fn sign(raw: TokenRaw) -> Result<Token, TokenError> {
    let mut mac = Hmac::<Sha256>::new_from_slice(&token_secret()?).context(InvalidHmacSnafu)?;
    mac.update(&bitcode::encode(&raw));

    let sig = mac.finalize().into_bytes();

    Ok(Token {
        raw,
        sig: sig.to_vec(),
    })
}

pub(crate) fn verify(token: &Token) -> Result<Result<&TokenRaw, Denied>, TokenError> {
    let mut mac = Hmac::<Sha256>::new_from_slice(&token_secret()?).context(InvalidHmacSnafu)?;
    mac.update(&bitcode::encode(&token.raw));

    let verify = mac
        .verify_slice(&token.sig)
        .context(TransitErrorContext!(display Denied));

    if let Err(e) = verify {
        return Ok(Err(e));
    }

    Ok(Ok(&token.raw))
}
