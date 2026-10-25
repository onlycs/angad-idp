use std::{path::PathBuf, sync::Arc};

use argon2::Argon2;
use lettre::{
    Address, AsyncSmtpTransport, Tokio1Executor, message::Mailbox,
    transport::smtp::authentication::Credentials,
};
use regex::{__private::Lazy, Regex};
use sha2::Digest;
use snafu::ResultExt;
use sqlx::PgPool;
use transit_core::{InternalError, InternalErrorContext};

macro_rules! regex {
    ($re:literal) => {
        regex::__private::Lazy::new(|| regex::Regex::new($re).expect("invalid regex pattern"))
    };
}

pub(crate) static SLUG_RE: Lazy<Regex> = regex!(r"^[a-z][a-z0-9-]*$");
pub(crate) static URL_RE: Lazy<Regex> = regex!(r"^https?://[^\s/$.?#].[^\s]*$");
pub(crate) static URI_RE: Lazy<Regex> = regex!(r"^[^\s/$.?#].[^\s]*$");
pub(crate) static DOMAIN_RE: Lazy<Regex> = regex!(r"^([a-z0-9-]+\.)+[a-z0-9-]+$");
pub(crate) static USERNAME_RE: Lazy<Regex> = regex!(r"^[a-z_][a-z0-9_-]{0,31}$");
pub(crate) static EMAIL_RE: Lazy<Regex> = regex!(r"^[^\s@]+@[^\s@]+\.[^\s@]+$");

fn env_parse_bool(s: String) -> Result<bool, InternalError> {
    match s.trim().to_ascii_lowercase().as_str() {
        "true" | "1" | "yes" => Ok(true),
        "false" | "0" | "no" => Ok(false),
        _ => Err(InternalError!(
            ctx(none),
            "Could not parse `{s}` as a boolean."
        )),
    }
}

fn env_parse_u16(s: String) -> Result<u16, snafu::Whatever> {
    s.parse()
        .with_whatever_context(|e| format!("{s} not a u16: {e}"))
}

pub(crate) fn parse_email(s: impl AsRef<str>) -> Result<Address, snafu::Whatever> {
    let s = s.as_ref();
    s.parse()
        .with_whatever_context(|e| format!("{s} not a u16: {e}"))
}

fn parse_email_str(s: impl AsRef<str>) -> Result<String, snafu::Whatever> {
    parse_email(s).map(|it| it.to_string())
}

fn env_check_domain(s: impl AsRef<str>) -> Result<String, snafu::Whatever> {
    let s = s.as_ref();

    snafu::ensure_whatever!(DOMAIN_RE.is_match(s), "`{s}` is not a domain");

    Ok(s.to_string())
}

fn env_parse_dirpath(s: impl AsRef<str>) -> Result<PathBuf, snafu::Whatever> {
    let buf = PathBuf::from(s.as_ref().to_string());

    snafu::ensure_whatever!(buf.exists(), "{buf:?} doesn't exist. Mount something here.");

    Ok(buf)
}

fn url_normalize(s: impl AsRef<str>) -> Result<String, snafu::Whatever> {
    let s = s.as_ref();

    snafu::ensure_whatever!(URL_RE.is_match(s), "`{s}` is not a URL");

    Ok(s.trim_end_matches("/").to_string())
}

macro_rules! static_env {
    // ---- done: emit the checker ----
    (@parse [$( $(#[$a:meta])* $id:ident : $t:ty, )*]) => {
        /// Forces every env static so misconfiguration fails at startup.
        pub fn env_check_all() {
            $(
                $(#[$a])*
                { let _: &$t = ::std::hint::black_box(&*$id); }
            )*
        }
    };

    // ---- derived static ----
    (@parse [$($seen:tt)*] $(#[$a:meta])* $id:ident : $t:ty = $init:expr $(; $($rest:tt)*)?) => {
        static_env!(@emit $(#[$a])* $id: $t = $init);
        static_env!(@parse [$($seen)* $(#[$a])* $id: $t,] $($($rest)*)?);
    };

    // ---- parsed, fallible default ----
    (@parse [$($seen:tt)*] $(#[$a:meta])* $env:ident as $t:ident via $f:ident or try $d:expr $(; $($rest:tt)*)?) => {
        static_env!(@emit $(#[$a])* $env: $t =
            match ::std::env::var(stringify!($env)) {
                Ok(v) => static_env!(@parse_with $env, $f, v),
                Err(_) => static_env!(@ok_or_die $env, $d),
            });
        static_env!(@parse [$($seen)* $(#[$a])* $env: $t,] $($($rest)*)?);
    };

    // ---- parsed, plain default ----
    (@parse [$($seen:tt)*] $(#[$a:meta])* $env:ident as $t:ident via $f:ident or $d:expr $(; $($rest:tt)*)?) => {
        static_env!(@emit $(#[$a])* $env: $t =
            match ::std::env::var(stringify!($env)) {
                Ok(v) => static_env!(@parse_with $env, $f, v),
                Err(_) => $d,
            });
        static_env!(@parse [$($seen)* $(#[$a])* $env: $t,] $($($rest)*)?);
    };

    // ---- parsed, required ----
    (@parse [$($seen:tt)*] $(#[$a:meta])* $env:ident as $t:ident via $f:ident $(; $($rest:tt)*)?) => {
        static_env!(@emit $(#[$a])* $env: $t = {
            let v = static_env!(@require $env);
            static_env!(@parse_with $env, $f, v)
        });
        static_env!(@parse [$($seen)* $(#[$a])* $env: $t,] $($($rest)*)?);
    };

    // ---- String, fallible default ----
    (@parse [$($seen:tt)*] $(#[$a:meta])* $env:ident or try $d:expr $(; $($rest:tt)*)?) => {
        static_env!(@emit $(#[$a])* $env: String =
            ::std::env::var(stringify!($env))
                .unwrap_or_else(|_| static_env!(@ok_or_die $env, $d)));
        static_env!(@parse [$($seen)* $(#[$a])* $env: String,] $($($rest)*)?);
    };

    // ---- String, plain default ----
    (@parse [$($seen:tt)*] $(#[$a:meta])* $env:ident or $d:expr $(; $($rest:tt)*)?) => {
        static_env!(@emit $(#[$a])* $env: String =
            ::std::env::var(stringify!($env)).unwrap_or_else(|_| $d));
        static_env!(@parse [$($seen)* $(#[$a])* $env: String,] $($($rest)*)?);
    };

    // ---- String, required ----
    (@parse [$($seen:tt)*] $(#[$a:meta])* $env:ident $(; $($rest:tt)*)?) => {
        static_env!(@emit $(#[$a])* $env: String = static_env!(@require $env));
        static_env!(@parse [$($seen)* $(#[$a])* $env: String,] $($($rest)*)?);
    };

    // ---- helpers ----
    (@emit $(#[$a:meta])* $id:ident : $t:ty = $init:expr) => {
        $(#[$a])*
        pub(crate) static $id: ::std::sync::LazyLock<$t> = ::std::sync::LazyLock::new(|| $init);
    };
    (@require $env:ident) => {
        ::std::env::var(stringify!($env)).unwrap_or_else(|e| {
            panic!("environment variable `{}`: {e}", stringify!($env))
        })
    };
    (@parse_with $env:ident, $f:ident, $v:expr) => {
        $f($v).unwrap_or_else(|e| {
            panic!("invalid value for `{}`: {e:?}", stringify!($env))
        })
    };
    (@ok_or_die $env:ident, $d:expr) => {
        ($d).unwrap_or_else(|e| {
            panic!("`{}` is unset and has no usable default: {e:?}", stringify!($env))
        })
    };

    // ---- entry point ----
    ($($input:tt)*) => {
        static_env!(@parse [] $($input)*);
    };
}

static_env!(
    DATABASE_URL;

    LISTEN_ADDRESS or String::from("0.0.0.0");
    TRANSIT_PORT as u16 via env_parse_u16 or 23849;
    WT_PORT as u16 via env_parse_u16 or 80;
    WT_PATH or String::from("/api");

    SMTP_USERNAME;
    SMTP_PASSWORD;
    SMTP_FROM as Address via parse_email or try parse_email(&*SMTP_USERNAME);
    SMTP_SERVER;
    // SMTP_USE_TLS as bool via env_parse_bool or true;
    INITIAL_ADMIN_EMAIL as String via parse_email_str;

    FRONTEND_URL as String via url_normalize;

    AUTHELIA_DOMAIN as String via env_check_domain;
    AUTHELIA_CONFIG_DIR as PathBuf via env_parse_dirpath;

    #[cfg(feature = "k8s")]
    AUTHELIA_NAMESPACE or try {
        cfg_select! {
            feature = "docker" => Ok::<_, snafu::Whatever>(String::default()),
            _ => Err::<(), _>("Missing environment variable `AUTHELIA_NAMESPACE`")
        }
    };
    #[cfg(feature = "k8s")]
    AUTHELIA_DEPLOYMENT or try {
        cfg_select! {
            feature = "docker" => Ok::<_, snafu::Whatever>(String::default()),
            _ => Err::<(), _>("Missing environment variable `AUTHELIA_DEPLOYMENT`")
        }
    };
    #[cfg(feature = "docker")]
    AUTHELIA_CONTAINER  or try {
        cfg_select! {
            feature = "k8s" => Ok::<_, snafu::Whatever>(String::default()),
            _ => Err::<(), _>("Missing environment variable `AUTHELIA_CONTAINER`")
        }
    };

    SMTP_FROM_MAILBOX: Mailbox = Mailbox::new(None, SMTP_FROM.clone());
    SMTP_CREDENTIALS: Credentials = Credentials::new(SMTP_USERNAME.clone(), SMTP_PASSWORD.clone());
    SMTP_TRANSPORT: AsyncSmtpTransport<Tokio1Executor> = AsyncSmtpTransport::<Tokio1Executor>::relay(&SMTP_SERVER).unwrap().credentials(SMTP_CREDENTIALS.clone()).build();
);

pub(crate) const ERROR_DB: &str = "Error communicating with the database";
pub(crate) const ERROR_GENERIC: &str = "Unknown internal error";

pub fn generate_secret() -> Result<[u8; 16], InternalError> {
    let mut buf = [0u8; 16];
    getrandom::fill(&mut buf).context(InternalErrorContext!("Could not generate client secret"))?;

    Ok(buf)
}

pub fn argon2() -> Argon2<'static> {
    Argon2::new(
        argon2::Algorithm::Argon2id,
        argon2::Version::V0x13,
        argon2::Params::DEFAULT,
    )
}

pub fn sha256(bytes: &[u8]) -> Vec<u8> {
    sha2::Sha256::new().chain_update(bytes).finalize().to_vec()
}

pub(crate) trait PgRef {
    fn as_ref(&self) -> &PgPool;
}

impl PgRef for PgPool {
    fn as_ref(&self) -> &PgPool {
        self
    }
}

impl PgRef for &PgPool {
    fn as_ref(&self) -> &PgPool {
        self
    }
}

impl PgRef for Arc<PgPool> {
    fn as_ref(&self) -> &PgPool {
        self
    }
}
