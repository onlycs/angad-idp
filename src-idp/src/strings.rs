use regex::{__private::Lazy, Regex};

macro_rules! regex {
    ($re:literal) => {
        regex::__private::Lazy::new(|| regex::Regex::new($re).expect("invalid regex pattern"))
    };
}

pub const ERROR_DB: &str = "Error communicating with the database";

pub const ENV_SECRET: &str = "TOKEN_SECRET";
pub const ENV_DATABSE_URL: &str = "DATABASE_URL";

pub static SLUG_RE: Lazy<Regex> = regex!(r"^[a-z][a-z0-9-]*$");
pub static NAME_RE: Lazy<Regex> = regex!(r"^[a-zA-Z][a-zA-Z0-9-]*$");
pub static URL_RE: Lazy<Regex> = regex!(r"^https?://[^\s/$.?#].[^\s]*$");
pub static URI_RE: Lazy<Regex> = regex!(r"^[^\s/$.?#].[^\s]*$");
