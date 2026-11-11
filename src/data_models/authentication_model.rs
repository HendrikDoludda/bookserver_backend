use serde::{Deserialize, Serialize};

// The request bodies that carry a password or a code don't derive `Debug`, so a
// `{:?}` in a log line can't print them.

/// `POST /api/v1/auth/register`. The device fields are used when the account is verified on
/// the spot and logged in.
#[derive(Deserialize, Clone)]
pub struct UserCreationRequest {
    pub email: String,
    pub username: String,
    pub password: String,
    pub password_repeat: String,
    pub device_name: String,
    /// The id this server gave the device at an earlier login; absent on a first login.
    pub device_id: Option<String>,
    pub platform: String,
    /// The code the server logs at start while no account exists; only the first sign-up
    /// needs it.
    pub setup_code: Option<String>,
}

/// `POST /api/v1/auth/verify_email`: a login plus the emailed code.
#[derive(Deserialize, Clone)]
pub struct VerifyEmailRequest {
    pub username_or_email: String,
    pub password: String,
    pub code: String,
    pub device_name: String,
    /// The id this server gave the device at an earlier login; absent on a first login.
    pub device_id: Option<String>,
    pub platform: String,
}

/// `POST /api/v1/auth/verify_email/resend`
#[derive(Debug, Deserialize)]
pub struct ResendVerificationRequest {
    pub username_or_email: String,
}

/// The 202 answer to a sign-up whose account waits for the emailed code.
#[derive(Serialize)]
pub struct VerificationRequired {
    pub verification_required: bool,
}

/// `POST /api/v1/auth/login`
#[derive(Deserialize, Clone)]
pub struct LoginRequest {
    pub username_or_email: String,
    pub password: String,
    pub device_name: String,
    /// The id this server gave the device at an earlier login; absent on a first login.
    pub device_id: Option<String>,
    pub platform: String,
}

#[derive(Debug, Clone)]
pub struct DeviceInformation<'a> {
    pub device_name: &'a str,
    pub device_id: &'a str,
    pub platform: &'a str,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ChangeEmailRequest {
    pub new_email: String,
}

/// `PUT /api/v1/account/username`
#[derive(Debug, Deserialize, Clone)]
pub struct ChangeUserNameRequest {
    pub new_username: String,
}

/// `PUT /api/v1/account/password`
#[derive(Deserialize, Clone)]
pub struct ChangePasswordRequest {
    pub current_password: String,
    pub new_password: String,
    pub new_password_repeat: String,
}

/// `POST /api/v1/auth/password_reset`
#[derive(Debug, Deserialize)]
pub struct PasswordResetRequest {
    pub email: String,
}

/// `POST /api/v1/auth/password_reset/confirm`
#[derive(Deserialize)]
pub struct ConfirmPasswordResetRequest {
    pub email: String,
    pub code: String,
    pub new_password: String,
    pub new_password_repeat: String,
}

/// A session's tokens, as login and refresh return them. Times are unix seconds.
#[derive(Serialize)]
pub struct SessionTokens {
    pub session_token: String,
    pub session_expires_at: i64,
    pub refresh_token: String,
    pub refresh_expires_at: i64,
    pub device_id: String,
}

/// The answer to a correct password: `{"status": "ok", …tokens}`, or
/// `{"status": "totp_required", challenge_token, challenge_expires_at}`.
#[derive(Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum LoginResponse {
    Ok(SessionTokens),
    TotpRequired {
        challenge_token: String,
        challenge_expires_at: i64,
    },
}
