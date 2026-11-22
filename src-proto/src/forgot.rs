use transit_core::{InternalError, error, record, route};

use crate::{
    invite::{EmailSend, InsecurePassword},
    user::InvalidEmail,
};

#[record]
pub struct ForgotCaptchaResponse {
    pub site_key: String,
}

#[record]
pub struct SendEmailRequest {
    pub email: String,
    pub token: String,
}

#[record]
pub struct ResetRequest {
    pub key: String,
    pub password: String,
}

error! {
    InvalidCaptcha("Invalid captcha token");
    VerifyFail("Failed to verify captcha with cloudflare");
    InvalidReset("Invalid or expired reset token");

    SendEmailError = InvalidEmail | InvalidCaptcha | VerifyFail | EmailSend;
    ResetError = InvalidReset | InsecurePassword;
}

route! {
    ForgotCaptcha() -> Result<ForgotCaptchaResponse, InternalError>;
    ForgotSendEmail(SendEmailRequest) -> Result<(), SendEmailError>;
    ForgotReset(ResetRequest) -> Result<(), ResetError>;
}
