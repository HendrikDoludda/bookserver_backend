//! Stubs until the flow in `totp_management.rs` is implemented.

use axum::{body::Body, http::Response};

use super::helpers::not_implemented_response;

/// Requires a fully authenticated session (`AuthSession`).
pub async fn totp_start_set_up() -> Response<Body> {
    not_implemented_response()
}

/// Requires a fully authenticated session (`AuthSession`).
pub async fn finalize_totp_set_up() -> Response<Body> {
    not_implemented_response()
}

/// Requires a fully authenticated session (`AuthSession`).
pub async fn disable_totp() -> Response<Body> {
    not_implemented_response()
}

/// Requires a half-authenticated TOTP login session (`AuthTOTPLoginSession`).
pub async fn sign_in_totp_with_recovery_code() -> Response<Body> {
    not_implemented_response()
}

/// Requires a half-authenticated TOTP login session (`AuthTOTPLoginSession`).
pub async fn sign_in_totp() -> Response<Body> {
    not_implemented_response()
}
