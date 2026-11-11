use std::{sync::Arc, time::SystemTime};

use axum::{
    body::Body,
    extract::State,
    http::{HeaderMap, Response, StatusCode},
    response::IntoResponse,
    Json,
};

use crate::{
    data_models::authentication_model::{
        ConfirmPasswordResetRequest, LoginRequest, PasswordResetRequest, ResendVerificationRequest,
        UserCreationRequest, VerificationRequired, VerifyEmailRequest,
    },
    db::Database,
    error_types::{AppErrors, EmailErrors},
    routes::{
        email_helper::Mailer,
        login::{end_session, log_in, refresh},
        password_reset::{confirm_reset, send_reset_code},
        registration::{
            confirm_email, create_account, send_verification_code, SetupCode, SignUpOutcome,
        },
    },
};

use super::{
    extractors::{bearer_token, AuthRefreshSession, JsonBody},
    helpers::no_store,
};

/// `POST /api/v1/auth/register`: 200 with login's answer for an account verified on the spot,
/// 202 `{"verification_required": true}` for one that waits for its emailed code.
pub async fn sign_up(
    State(db): State<Arc<Database>>,
    State(mailer): State<Mailer>,
    State(setup_code): State<SetupCode>,
    JsonBody(request): JsonBody<UserCreationRequest>,
) -> Result<Response<Body>, AppErrors> {
    let outcome = create_account(db, &mailer, setup_code, request, SystemTime::now()).await?;
    Ok(match outcome {
        SignUpOutcome::LoggedIn(response) => no_store(Json(response)),
        SignUpOutcome::VerificationRequired => (
            StatusCode::ACCEPTED,
            Json(VerificationRequired {
                verification_required: true,
            }),
        )
            .into_response(),
    })
}

/// `POST /api/v1/auth/login`
pub async fn sign_in(
    State(db): State<Arc<Database>>,
    JsonBody(request): JsonBody<LoginRequest>,
) -> Result<Response<Body>, AppErrors> {
    let response = log_in(db, request, SystemTime::now()).await?;
    Ok(no_store(Json(response)))
}

/// `POST /api/v1/auth/verify_email`: answers like login.
pub async fn verify_email(
    State(db): State<Arc<Database>>,
    JsonBody(request): JsonBody<VerifyEmailRequest>,
) -> Result<Response<Body>, AppErrors> {
    let response = confirm_email(db, request, SystemTime::now()).await?;
    Ok(no_store(Json(response)))
}

/// `POST /api/v1/auth/verify_email/resend`: answers 202 before the name is even looked up, so
/// the answer can't tell whether an unverified account exists; the code goes out in the
/// background.
pub async fn resend_verification_code(
    State(db): State<Arc<Database>>,
    State(mailer): State<Mailer>,
    JsonBody(request): JsonBody<ResendVerificationRequest>,
) -> Result<StatusCode, AppErrors> {
    if !mailer.is_enabled() {
        return Err(EmailErrors::EmailSetUpNotFound.into());
    }
    tokio::spawn(send_verification_code(
        db,
        mailer,
        request.username_or_email,
        SystemTime::now(),
    ));
    Ok(StatusCode::ACCEPTED)
}

/// `POST /api/v1/auth/logout` with either token as Bearer. No session extractor: an expired
/// token must still log out, and the answer is 204 whether or not a session was found.
pub async fn log_out(
    State(db): State<Arc<Database>>,
    headers: HeaderMap,
) -> Result<StatusCode, AppErrors> {
    end_session(db, bearer_token(&headers).map(str::to_string)).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// `POST /api/v1/auth/refresh` with the refresh token as Bearer.
pub async fn refresh_session(
    session: AuthRefreshSession,
    State(db): State<Arc<Database>>,
) -> Result<Response<Body>, AppErrors> {
    let tokens = refresh(db, session.0, SystemTime::now()).await?;
    Ok(no_store(Json(tokens)))
}

/// `POST /api/v1/auth/password_reset`: answers 202 before the address is even looked up,
/// so known and unknown addresses can't be told apart; the code goes out in the background.
pub async fn request_password_reset(
    State(db): State<Arc<Database>>,
    State(mailer): State<Mailer>,
    JsonBody(request): JsonBody<PasswordResetRequest>,
) -> Result<StatusCode, AppErrors> {
    if !mailer.is_enabled() {
        return Err(EmailErrors::EmailSetUpNotFound.into());
    }
    tokio::spawn(send_reset_code(
        db,
        mailer,
        request.email,
        SystemTime::now(),
    ));
    Ok(StatusCode::ACCEPTED)
}

/// `POST /api/v1/auth/password_reset/confirm`
pub async fn confirm_password_reset(
    State(db): State<Arc<Database>>,
    JsonBody(request): JsonBody<ConfirmPasswordResetRequest>,
) -> Result<StatusCode, AppErrors> {
    confirm_reset(db, request, SystemTime::now()).await?;
    Ok(StatusCode::OK)
}
