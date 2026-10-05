use transit_core::{error, record, route};

use crate::{auth::Authentication, error::Denied};

#[record]
pub struct Membership {
    pub slug: String,
    pub admin: bool,
}

#[record]
pub struct ApplicationOidc {
    pub redirect_uris: Vec<String>,
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
    pub client_secret: Option<String>,
}

#[record]
pub struct ApplicationUpdateRequest {
    pub auth: Authentication,
    pub slug: String,
    pub name: Option<String>,
    pub url: Option<String>,
    pub redirect_uris: Option<Vec<String>>,
    pub roll_client_secret: bool,
}

#[record]
pub struct ApplicationUpdateResponse {
    pub app: Application,
    /// `redirect_uris` empty before update or `roll_client_secret` was true
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

    InvalidSlug("Invalid slug");
    InvalidAppName("Invalid name");
    InvalidUrl("Invalid url");
    InvalidRedirectUri("Could not parse redirect_uri `{uri}`") { uri: String };
    SlugInUse("Slug in use");
    AppNameInUse("Name in use");

    ApplicationCreateError = InvalidSlug | InvalidAppName | InvalidUrl | InvalidRedirectUri | SlugInUse | AppNameInUse | Denied;
    ApplicationListError = Denied;
    ApplicationUpdateError = NoApplication | NoApplicationOidc | InvalidAppName | InvalidUrl | InvalidRedirectUri | AppNameInUse | Denied;
    ApplicationDeleteError = NoApplication | Denied;
}

route! {
    ApplicationCreate(ApplicationCreateRequest) -> Result<ApplicationCreateResponse, ApplicationCreateError>;
    ApplicationList(Authentication) -> Result<Vec<Application>, ApplicationListError>;
    ApplicationUpdate(ApplicationUpdateRequest) -> Result<ApplicationUpdateResponse, ApplicationUpdateError>;
    ApplicationDelete(ApplicationDeleteRequest) -> Result<(), ApplicationDeleteError>;
}
