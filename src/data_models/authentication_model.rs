use std::time::SystemTime;

use serde::{Deserialize, Serialize};

// The request bodies that carry a password or a code don't derive `Debug`, so a
// `{:?}` in a log line can't print them.
#[derive(Deserialize, Clone)]
pub struct UserCreationRequest {
    pub email: String,
    pub username: String,
    pub password: String,
    pub password_repeat: String,
}

#[derive(Deserialize, Clone)]
pub struct VerifyEmailRequest {
    pub email_verification_token: String,
    pub email: String,
    pub device_name: String,
    pub device_id: String,
    pub platform: String,
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

#[derive(Debug, Deserialize, Clone)]
pub struct ChangeUserNameRequest {
    pub new_username: String,
}

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

#[derive(Serialize)]
pub struct AuthResponse {
    pub message: String,
    pub session_token: String,
    pub refresh_token: String,
    pub session_token_expiration: SystemTime,
    pub refresh_token_expiration_time: SystemTime,
}
