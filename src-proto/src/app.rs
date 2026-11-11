use std::str::FromStr;

use transit_core::{constvar, error, oneof, record, route};
use xxhash_rust::const_xxh3;

use crate::{auth::Authentication, error::Denied};

#[constvar]
pub const DOMAIN_SLUG: &str = "angad-idp";

#[constvar]
pub const GROUP_SUFFIX_ADMIN: &str = "admin";

#[constvar]
pub const GROUP_SUFFIX_USER: &str = "member";

#[constvar]
pub const DOMAIN_CLIENT_ID: &str = const {
    const _DOMAIN_CLIENT_ID_BYTES: [u8; 16] = const {
        const HEX: &[u8; 16] = b"0123456789abcdef";

        let xxh = const_xxh3::xxh3_64(DOMAIN_SLUG.as_bytes()).to_le_bytes();
        let mut hex = [0u8; 16];
        let mut i = 0;

        while i < xxh.len() {
            hex[2 * i] = HEX[(xxh[i] >> 4) as usize];
            hex[2 * i + 1] = HEX[(xxh[i] & 0x0f) as usize];
            i += 1;
        }

        hex
    };

    unsafe { std::str::from_utf8_unchecked(&_DOMAIN_CLIENT_ID_BYTES) }
};

#[record]
#[cfg_attr(not(target_family = "wasm"), derive(serde::Deserialize))]
#[derive(PartialEq, Eq)]
pub struct Membership {
    pub slug: String,
    pub admin: bool,
}

impl FromStr for Membership {
    type Err = snafu::Whatever;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let split = s.trim().rsplitn(2, ":").collect::<Vec<_>>();

        snafu::ensure_whatever!(
            split.len() == 2 && matches!(split[0], GROUP_SUFFIX_ADMIN | GROUP_SUFFIX_USER),
            "Must be of form `group:[{GROUP_SUFFIX_ADMIN}|{GROUP_SUFFIX_USER}]"
        );

        Ok(Self {
            slug: split[1].trim().to_string(),
            admin: split[0] == GROUP_SUFFIX_ADMIN,
        })
    }
}

#[record]
pub struct ApplicationOidcSupplied {
    pub redirect_uris: Vec<String>,
    pub public: bool,
}

#[record]
pub struct ApplicationOidc {
    pub redirect_uris: Vec<String>,
    pub client_id: String,
    pub public: bool,
}

#[record]
pub struct Application {
    pub slug: String,
    pub name: String,
    pub url: String,
    pub oidc: Option<ApplicationOidc>,
}

#[record]
pub struct ApplicationSupplied {
    pub slug: String,
    pub name: String,
    pub url: String,
    pub oidc: Option<ApplicationOidcSupplied>,
}

#[record]
pub struct ApplicationCreateRequest {
    pub auth: Authentication,
    pub app: ApplicationSupplied,
}

#[record]
pub struct ApplicationCreateResponse {
    /// iff app.oidc was provided
    pub client_id: Option<String>,
    /// iff app.oidc.public = false
    pub client_secret: Option<String>,
}

#[oneof]
#[derive(PartialEq, Eq)]
pub enum ApplicationPublicUpdate {
    Keep,
    Set(bool),
    /// can be used only if public was false
    RollClientSecret,
}

#[oneof]
pub enum ApplicationOidcUpdate {
    Keep,
    Create(ApplicationOidcSupplied),
    Modify {
        redirect_uris: Option<Vec<String>>,
        public: ApplicationPublicUpdate,
    },
    Remove,
}

#[record]
pub struct ApplicationUpdateRequest {
    pub auth: Authentication,
    pub slug: String,
    pub name: Option<String>,
    pub url: Option<String>,
    pub oidc: ApplicationOidcUpdate,
}

#[record]
pub struct ApplicationUpdateResponse {
    pub app: Application,
    /// iff req.oidc was Create or req.update.public was RollClientSecret
    pub client_secret: Option<String>,
}

#[record]
pub struct ApplicationDeleteRequest {
    pub auth: Authentication,
    pub slug: String,
}

error! {
    NoApplication("Application {slug:?} not found") { slug: String };
    NoApplicationOidc("Application {slug:?} does not have OIDC") { slug: String };
    NotPrivate("Application {slug:?} is public, and does not have a client secret") { slug: String };
    ApplicationOidcExists("Application {slug:?} already has an OIDC component") { slug: String };

    InvalidSlug("Invalid slug");
    InvalidAppName("Invalid name");
    InvalidUrl("Invalid url");
    InvalidRedirectUri("Could not parse redirect_uri `{uri}`") { uri: String };
    SlugInUse("Slug in use");
    AppNameInUse("Name in use");

    ApplicationCreateError = InvalidSlug | InvalidAppName | InvalidUrl | InvalidRedirectUri | SlugInUse | AppNameInUse | Denied;
    ApplicationListError = Denied;
    ApplicationUpdateError = NoApplication | NoApplicationOidc | ApplicationOidcExists | NotPrivate | InvalidAppName | InvalidUrl | InvalidRedirectUri | AppNameInUse | Denied;
    ApplicationDeleteError = NoApplication | Denied;
}

route! {
    ApplicationCreate(ApplicationCreateRequest) -> Result<ApplicationCreateResponse, ApplicationCreateError>;
    ApplicationList(Authentication) -> Result<Vec<Application>, ApplicationListError>;
    ApplicationUpdate(ApplicationUpdateRequest) -> Result<ApplicationUpdateResponse, ApplicationUpdateError>;
    ApplicationDelete(ApplicationDeleteRequest) -> Result<(), ApplicationDeleteError>;
}
