use transit_core::{error, oneof, record, route};

use crate::{auth::Authentication, error::Denied};

pub const DOMAIN_SLUG: &str = "angad-idp";
pub const GROUP_SUFFIX_ADMIN: &str = "admin";
pub const GROUP_SUFFIX_USER: &str = "member";

#[cfg(feature = "uniffi")]
#[uniffi::export]
pub fn domain_slug() -> String {
    DOMAIN_SLUG.to_string()
}

#[cfg(feature = "uniffi")]
#[uniffi::export]
pub fn group_suffix_admin() -> String {
    GROUP_SUFFIX_ADMIN.to_string()
}

#[cfg(feature = "uniffi")]
#[uniffi::export]
pub fn group_suffix_user() -> String {
    GROUP_SUFFIX_USER.to_string()
}

#[record]
#[cfg_attr(not(target_family = "wasm"), derive(serde::Deserialize))]
#[derive(PartialEq, Eq)]
pub struct Membership {
    pub slug: String,
    pub admin: bool,
}

impl Membership {
    pub fn is_domain_admin(&self) -> bool {
        self.slug == DOMAIN_SLUG && self.admin
    }
}

#[record]
pub struct ApplicationOidc {
    pub redirect_uris: Vec<String>,
    pub match_url: String,
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
pub struct ApplicationCreateRequest {
    pub auth: Authentication,
    pub app: Application,
}

#[record]
pub struct ApplicationCreateResponse {
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
    Create(ApplicationOidc),
    Modify {
        redirect_uris: Option<Vec<String>>,
        match_url: Option<String>,
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
