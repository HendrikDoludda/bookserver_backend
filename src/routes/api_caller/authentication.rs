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
        ChangeEmailRequest, ChangePasswordRequest, ChangeUserNameRequest,
        ConfirmPasswordResetRequest, DeviceInformation, LoginRequest, PasswordResetRequest,
        UserCreationRequest, VerifyEmailRequest,
    },
    db::Database,
    error_types::{AppErrors, AuthenticationError, EmailErrors},
    models::{UserDatabaseColumns, UserMetadata},
    routes::auth::{
        create_new_session, create_new_user, create_verification_email,
        search_for_user_using_email_or_username, send_verification_email,
        update_password_to_new_password, validate_new_user, validate_submitted_password,
        verify_verification_code,
    },
    routes::{
        email_helper::Mailer,
        login::{end_session, log_in, refresh},
        password_reset::{confirm_reset, send_reset_code},
    },
};

use super::{
    extractors::{bearer_token, AuthRefreshSession, AuthSession},
    helpers::{create_auth_response, no_store},
};

pub async fn sign_up(
    State(db): State<Arc<Database>>,
    State(mailer): State<Mailer>,
    Json(request): Json<UserCreationRequest>,
) -> Result<Response<Body>, AppErrors> {
    let email_info = db.setup_new_transaction(|conn| {
        let validated_user = validate_new_user(conn, request.clone())?;
        let user_id = create_new_user(conn, validated_user, request.clone())?;
        let result = send_verification_email(conn, user_id)?;
        Ok(result)
    })?;

    let result = create_verification_email(&mailer, &email_info)
        .await
        .map_err(|err| match err {
            EmailErrors::EmailSetUpNotFound => EmailErrors::SentFailedDueToNoConfig {
                code: email_info.code,
            },
            err => err,
        });
    match result {
        Ok(()) => Ok(Response::builder()
            .status(StatusCode::ACCEPTED)
            .body("Created a user and sent a verification email.".into())
            .unwrap()),
        Err(EmailErrors::SentFailedDueToNoConfig { code }) => {
            auto_verify_account(request.email, code, &db, email_info.user.user_id).await
        }
        Err(err) => Err(err.into()),
    }
}

async fn auto_verify_account(
    email: String,
    code: String,
    db: &Arc<Database>,
    user_id: i64,
) -> Result<Response<Body>, AppErrors> {
    let verify_email_request = VerifyEmailRequest {
        email_verification_token: code,
        email,
        device_name: "Unknown".to_string(),
        device_id: "Unknown".to_string(),
        platform: "Unknown".to_string(),
    };
    verify_email_logic(verify_email_request, db, user_id)
}

/// `POST /api/v1/auth/login`
pub async fn sign_in(
    State(db): State<Arc<Database>>,
    Json(request): Json<LoginRequest>,
) -> Result<Response<Body>, AppErrors> {
    let response = log_in(db, request, SystemTime::now()).await?;
    Ok(no_store(Json(response)))
}

pub async fn verify_email(
    request: VerifyEmailRequest,
    State(db): State<Arc<Database>>,
) -> Result<Response<Body>, AppErrors> {
    let user = db.setup_new_transaction(|conn| {
        let found_user = search_for_user_using_email_or_username(conn, &request.email)?;
        Ok(found_user)
    })?;
    verify_email_logic(request, &db, user.user_id)
}

fn verify_email_logic(
    request: VerifyEmailRequest,
    db: &Arc<Database>,
    user_id: i64,
) -> Result<Response<Body>, AppErrors> {
    let session_information = db.setup_new_transaction(|conn| {
        verify_verification_code(conn, &request, user_id)?;
        let device_information = DeviceInformation {
            device_id: &request.device_id,
            device_name: &request.device_name,
            platform: &request.platform,
        };
        let session = create_new_session(conn, user_id, device_information, SystemTime::now())?;
        Ok(session)
    })?;
    Ok(create_auth_response("Your user account has been verified. The servers email address needs to be set up so that actual verification can take place", session_information))
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

pub async fn change_password(
    session: AuthSession,
    State(db): State<Arc<Database>>,
    request: ChangePasswordRequest,
) -> Result<Response<Body>, AppErrors> {
    if request.new_password != request.new_password_repeat {
        return Err(AuthenticationError::MismatchingPasswords.into());
    }
    db.setup_new_transaction(|conn| {
        let user = Database::get_entry_with_connection::<UserMetadata>(conn, session.0.user_id)?;
        validate_submitted_password(&user, &request.current_password)?;
        update_password_to_new_password(conn, request, user.user_id)?;
        Ok(())
    })?;
    Ok((StatusCode::ACCEPTED, "Password has been changed").into_response())
}

pub async fn change_username(
    session: AuthSession,
    State(db): State<Arc<Database>>,
    request: ChangeUserNameRequest,
) -> Result<Response<Body>, AppErrors> {
    db.setup_new_transaction(|conn| {
        Database::update_value_with_connection::<UserMetadata>(
            conn,
            UserDatabaseColumns::Username,
            &request.new_username,
            session.0.user_id,
        )?;
        Ok(())
    })?;
    Ok((StatusCode::ACCEPTED, "Username has been changed").into_response())
}

pub async fn change_email(
    session: AuthSession,
    State(db): State<Arc<Database>>,
    request: ChangeEmailRequest,
) -> Result<Response<Body>, AppErrors> {
    db.setup_new_transaction(|conn| {
        Database::update_value_with_connection::<UserMetadata>(
            conn,
            UserDatabaseColumns::Email,
            &request.new_email,
            session.0.user_id,
        )?;
        Ok(())
    })?;
    Ok((StatusCode::ACCEPTED, "Email has been changed").into_response())
}

/// `POST /api/v1/auth/password_reset`: answers 202 before the address is even looked up,
/// so known and unknown addresses can't be told apart; the code goes out in the background.
pub async fn request_password_reset(
    State(db): State<Arc<Database>>,
    State(mailer): State<Mailer>,
    Json(request): Json<PasswordResetRequest>,
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
    Json(request): Json<ConfirmPasswordResetRequest>,
) -> Result<StatusCode, AppErrors> {
    confirm_reset(db, request, SystemTime::now()).await?;
    Ok(StatusCode::OK)
}
