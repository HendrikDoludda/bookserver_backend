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
}

#[derive(Debug, Deserialize, Clone)]
pub struct LoginRequest {
    pub username_or_email: String,
    pub password: String,
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

#[derive(Serialize)]
pub struct AuthResponse {
    pub access_token: String,
    pub refresh_token: String,
}

//for the authentication use the axum extractors auth: AuthSession or something like that
