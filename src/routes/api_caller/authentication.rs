use std::{sync::Arc, time::SystemTime};

use axum::{
    body::Body,
    extract::State,
    http::{Response, StatusCode},
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
    models::{DatabaseTypes, UserDatabaseColumns, UserMetadata},
    routes::auth::{
        create_new_session, create_new_user, create_verification_email, log_in,
        search_for_user_using_email_or_username, send_verification_email, update_existing_session,
        update_password_to_new_password, validate_new_user, validate_submitted_password,
        verify_verification_code, CreatedTokensReadable, UserTotpEnabledResult,
    },
    routes::{
        email_helper::Mailer,
        password_reset::{confirm_reset, send_reset_code},
    },
};

use super::{
    extractors::{AuthRefreshSession, AuthSession},
    helpers::create_auth_response,
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

pub async fn sign_in(
    State(db): State<Arc<Database>>,
    request: LoginRequest,
) -> Result<Response<Body>, AppErrors> {
    let tokens: CreatedTokensReadable = db.setup_new_transaction(|conn| {
        let user = search_for_user_using_email_or_username(conn, &request.username_or_email)?;
        let user_totp_settings = log_in(&request, conn, &user)?;
        let device_information = DeviceInformation {
            device_id: &request.device_id,
            device_name: &request.device_name,
            platform: &request.platform,
        };
        let token_info =
            create_new_session(conn, user.user_id, device_information, user_totp_settings)?;
        Ok(token_info)
    })?;
    Ok(create_auth_response(
        "Your user account has been logged in",
        tokens,
    ))
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
        let default_totp_settings = UserTotpEnabledResult {
            user_has_totp_enabled: false,
        };
        let session = create_new_session(conn, user_id, device_information, default_totp_settings)?;
        Ok(session)
    })?;
    Ok(create_auth_response("Your user account has been verified. The servers email address needs to be set up so that actual verification can take place", session_information))
}

pub async fn log_out(
    session: AuthSession,
    State(db): State<Arc<Database>>,
) -> Result<Response<Body>, AppErrors> {
    db.remove_entry(DatabaseTypes::Session, session.0.session_id)?;
    Ok((StatusCode::ACCEPTED, "Successfully logged out!").into_response())
}

pub async fn refresh_session(
    session: AuthRefreshSession,
    State(db): State<Arc<Database>>,
) -> Result<Response<Body>, AppErrors> {
    let tokens: CreatedTokensReadable = db.setup_new_transaction(|conn| {
        let tokens = update_existing_session(conn, session.0)?;
        Ok(tokens)
    })?;
    Ok(create_auth_response("refreshed session", tokens))
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
