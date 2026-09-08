use std::time::SystemTime;

use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Clone)]
pub struct UserCreationRequest {
    pub email: String,
    pub username: String,
    pub password: String,
    pub password_repeat: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct VerifyEmailRequest {
    pub email_verification_token: String,
    pub email: String,
    pub device_name: String,
    pub device_id: String,
    pub platform: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct LoginRequest {
    pub username_or_email: String,
    pub password: String,
    pub device_name: String,
    pub device_id: String,
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

#[derive(Debug, Deserialize, Clone)]
pub struct ChangePasswordRequest {
    pub current_password: String,
    pub new_password: String,
    pub new_password_repeat: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct RequestPasswordResetLinkRequest {
    pub email: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ResettingUsersPassword {
    pub verification_token: String,
    pub new_password: String,
    pub new_password_repeat: String,
}

#[derive(Serialize)]
pub struct AuthResponse {
    pub message: String,
    pub session_token: String,
    pub refresh_token: String,
    pub session_token_expiration: SystemTime,
    pub refresh_token_expiration_time: SystemTime,
}

//for the authentication use the axum extractors auth: AuthSession or something like that
