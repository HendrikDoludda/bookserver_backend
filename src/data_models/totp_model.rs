use serde::{Deserialize, Serialize};

// The request bodies carry passwords and codes, so none of them derives `Debug`.

/// `POST /api/v1/auth/login/totp`
#[derive(Deserialize)]
pub struct TOTPSignInRequest {
    pub code: String,
}

/// `POST /api/v1/auth/login/recovery_code`
#[derive(Deserialize)]
pub struct RecoveryCodeSignInRequest {
    pub code: String,
}

/// `POST /api/v1/account/totp/confirm`: a code from the authenticator app and the password.
#[derive(Deserialize)]
pub struct ConfirmTOTPRequest {
    pub code: String,
    pub password: String,
}

/// `POST /api/v1/account/totp/disable`: the password and an authenticator or recovery code.
#[derive(Deserialize)]
pub struct DisableTOTPRequest {
    pub password: String,
    pub code: String,
}

/// `POST /api/v1/account/totp/recovery_codes`: the password and an authenticator or recovery
/// code.
#[derive(Deserialize)]
pub struct RenewRecoveryCodesRequest {
    pub password: String,
    pub code: String,
}

/// The answer to starting a setup; the app draws the QR code from `otpauth_url`.
#[derive(Serialize)]
pub struct TOTPSetup {
    pub otpauth_url: String,
    pub secret_base32: String,
}

/// The answer to confirming a setup or renewing the codes — the only time they are shown.
#[derive(Serialize)]
pub struct RecoveryCodesIssued {
    pub recovery_codes: Vec<String>,
}

/// `GET /api/v1/account/totp`
#[derive(Serialize)]
pub struct TOTPStatus {
    pub enabled: bool,
    pub recovery_codes_left: usize,
}
