//! The TOTP handlers: the 2FA settings, and the code step at login.

use std::{sync::Arc, time::SystemTime};

use axum::{
    body::Body,
    extract::State,
    http::{Response, StatusCode},
    Json,
};

use crate::{
    data_models::{
        authentication_model::LoginResponse,
        totp_model::{
            ConfirmTOTPRequest, DisableTOTPRequest, RecoveryCodeSignInRequest, TOTPSignInRequest,
            TOTPStatus,
        },
    },
    db::Database,
    error_types::AppErrors,
    routes::{
        totp_cipher::TotpCipher,
        totp_management::{begin_setup, confirm_setup, disable, finish_login, status, TypedCode},
    },
};

use super::{
    extractors::{AuthSession, AuthTOTPLoginSession},
    helpers::no_store,
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

/// `POST /api/v1/account/totp/disable`: ends every session of the user, this one too.
pub async fn disable_totp(
    session: AuthSession,
    State(db): State<Arc<Database>>,
    State(cipher): State<TotpCipher>,
    Json(request): Json<DisableTOTPRequest>,
) -> Result<StatusCode, AppErrors> {
    disable(db, &cipher, session.0.user_id, request, SystemTime::now()).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// `POST /api/v1/auth/login/recovery_code` with the challenge token as Bearer: answers like
/// login.
pub async fn sign_in_totp_with_recovery_code(
    challenge: AuthTOTPLoginSession,
    State(db): State<Arc<Database>>,
    State(cipher): State<TotpCipher>,
    Json(request): Json<RecoveryCodeSignInRequest>,
) -> Result<Response<Body>, AppErrors> {
    let code = TypedCode::Recovery(request.code);
    let tokens = finish_login(db, &cipher, challenge.0, code, SystemTime::now()).await?;
    Ok(no_store(Json(LoginResponse::Ok(tokens))))
}

/// `POST /api/v1/auth/login/totp` with the challenge token as Bearer: answers like login.
pub async fn sign_in_totp(
    challenge: AuthTOTPLoginSession,
    State(db): State<Arc<Database>>,
    State(cipher): State<TotpCipher>,
    Json(request): Json<TOTPSignInRequest>,
) -> Result<Response<Body>, AppErrors> {
    let code = TypedCode::Authenticator(request.code);
    let tokens = finish_login(db, &cipher, challenge.0, code, SystemTime::now()).await?;
    Ok(no_store(Json(LoginResponse::Ok(tokens))))
}
