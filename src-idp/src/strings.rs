use regex::{__private::Lazy, Regex};

macro_rules! regex {
    ($re:literal) => {
        regex::__private::Lazy::new(|| regex::Regex::new($re).expect("invalid regex pattern"))
    };
}

pub(crate) const ERROR_DB: &str = "Error communicating with the database";

pub(crate) const ENV_SECRET: &str = "TOKEN_SECRET";
pub(crate) const ENV_DATABSE_URL: &str = "DATABASE_URL";

pub(crate) static SLUG_RE: Lazy<Regex> = regex!(r"^[a-z][a-z0-9-]*$");
pub(crate) static NAME_RE: Lazy<Regex> = regex!(r"^[a-zA-Z][a-zA-Z0-9-]*$");
pub(crate) static URL_RE: Lazy<Regex> = regex!(r"^https?://[^\s/$.?#].[^\s]*$");
pub(crate) static URI_RE: Lazy<Regex> = regex!(r"^[^\s/$.?#].[^\s]*$");
