//! The TOTP handlers. Signing in with a code and turning 2FA off answer 501 until they are built.

use std::{sync::Arc, time::SystemTime};

use axum::{body::Body, extract::State, http::Response, Json};

use crate::{
    data_models::totp_model::{ConfirmTOTPRequest, TOTPStatus},
    db::Database,
    error_types::AppErrors,
    routes::{
        totp_cipher::TotpCipher,
        totp_management::{begin_setup, confirm_setup, status},
    },
};

use super::{
    extractors::AuthSession,
    helpers::{no_store, not_implemented_response},
};

/// `GET /api/v1/account/totp`
pub async fn totp_status(
    session: AuthSession,
    State(db): State<Arc<Database>>,
) -> Result<Json<TOTPStatus>, AppErrors> {
    Ok(Json(status(db, session.0.user_id).await?))
}

/// `POST /api/v1/account/totp`: the answer carries the new secret.
pub async fn totp_start_set_up(
    session: AuthSession,
    State(db): State<Arc<Database>>,
    State(cipher): State<TotpCipher>,
) -> Result<Response<Body>, AppErrors> {
    let setup = begin_setup(db, &cipher, session.0.user_id, SystemTime::now()).await?;
    Ok(no_store(Json(setup)))
}

/// `POST /api/v1/account/totp/confirm`: answers the recovery codes, shown only this once.
pub async fn finalize_totp_set_up(
    session: AuthSession,
    State(db): State<Arc<Database>>,
    State(cipher): State<TotpCipher>,
    Json(request): Json<ConfirmTOTPRequest>,
) -> Result<Response<Body>, AppErrors> {
    let AuthSession(session) = session;
    let codes = confirm_setup(
        db,
        &cipher,
        session.user_id,
        session.session_id,
        request,
        SystemTime::now(),
    )
    .await?;
    Ok(no_store(Json(codes)))
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
