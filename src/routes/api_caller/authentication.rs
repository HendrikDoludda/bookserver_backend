use std::{
    sync::Arc,
    time::{Duration, SystemTime},
};

use axum::{
    body::Body,
    extract::State,
    http::{Response, StatusCode},
    response::IntoResponse,
    Json,
};
use rusqlite::Connection;
use url::Url;

use crate::{
    data_models::authentication_model::{
        ChangeEmailRequest, ChangePasswordRequest, ChangeUserNameRequest, DeviceInformation,
        LoginRequest, RequestPasswordResetLinkRequest, ResettingUsersPassword, UserCreationRequest,
        VerifyEmailRequest,
    },
    db::Database,
    error_types::{DatabaseError, EmailErrors},
    models::{
        DatabaseTypes, QuerySeparator, ResetPasswordRequest, ResetPasswordRequestColumns,
        UserDatabaseColumns, UserMetadata,
    },
    routes::auth::{
        create_new_session, create_new_user, create_verification_email,
        generate_password_reset_codes, log_in, search_for_user_using_email_or_username,
        send_reset_password_email, send_verification_email, update_existing_session,
        update_password_to_new_password, validate_new_user, validate_submitted_password,
        verify_verification_code, CreatedTokensReadable, ResetAndCancelationTokens,
        UserTotpEnabledResult,
    },
};

use super::{
    extractors::{AuthRefreshSession, AuthSession},
    helpers::{create_auth_response, create_internal_server_error_response},
};

pub async fn sign_up(
    State(db): State<Arc<Database>>,
    Json(request): Json<UserCreationRequest>,
) -> Response<Body> {
    let email_info = match db.setup_new_transaction(|conn| {
        let validated_user = validate_new_user(conn, request.clone())?;
        let user_id = create_new_user(conn, validated_user, request.clone())?;
        let result = send_verification_email(conn, user_id)?;
        Ok(result)
    }) {
        Ok(information) => information,
        Err(err) => {
            log::error!("Failed the sign up procedure with error: {err}");
            return create_internal_server_error_response(format!(
                "Could not create a user due to: {err}"
            ));
        }
    };

    let result = create_verification_email(&email_info)
        .await
        .map_err(|err| match err {
            EmailErrors::EmailSetUpNotFound => EmailErrors::SentFailedDueToNoConfig {
                code: email_info.code,
            },
            err => err,
        });
    match result {
        Ok(()) => Response::builder()
            .status(StatusCode::ACCEPTED)
            .body("Created a user and sent a verification email.".into())
            .unwrap(),
        Err(EmailErrors::SentFailedDueToNoConfig { code }) => {
            auto_verify_account(request.email, code, &db, email_info.user.user_id).await
        }
        Err(err) => create_internal_server_error_response(format!(
            "Failed to send verification email due to: {err}"
        )),
    }
}

async fn auto_verify_account(
    email: String,
    code: String,
    db: &Arc<Database>,
    user_id: i64,
) -> Response<Body> {
    let verify_email_request = VerifyEmailRequest {
        email_verification_token: code,
        email,
        device_name: "Unknown".to_string(),
        device_id: "Unknown".to_string(),
        platform: "Unknown".to_string(),
    };
    verify_email_logic(verify_email_request, db, user_id)
}

pub async fn sign_in(State(db): State<Arc<Database>>, request: LoginRequest) -> Response<Body> {
    let tokens: CreatedTokensReadable = match db.setup_new_transaction(|conn| {
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
    }) {
        Ok(tokens) => tokens,
        Err(err) => {
            log::error!("Failed with log in transaction at: {err}");
            return create_internal_server_error_response(format!(
                "Failed to sign in due to: {err}"
            ));
        }
    };
    create_auth_response("Your user account has been logged in", tokens)
}

pub async fn verify_email(
    request: VerifyEmailRequest,
    State(db): State<Arc<Database>>,
) -> Response<Body> {
    let user = match db.setup_new_transaction(|conn| {
        let found_user = search_for_user_using_email_or_username(conn, &request.email)?;
        Ok(found_user)
    }) {
        Ok(user) => user,
        Err(err) => {
            log::error!("Failed the transaction for auto verifying an account: {err}");
            return create_internal_server_error_response(format!("failed retrieving user: {err}"));
        }
    };
    verify_email_logic(request, &db, user.user_id)
}

fn verify_email_logic(
    request: VerifyEmailRequest,
    db: &Arc<Database>,
    user_id: i64,
) -> Response<Body> {
    let session_information = match db.setup_new_transaction(|conn| {
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
    }) {
        Ok(session) => session,
        Err(err) => {
            log::error!("Failed the transaction for auto verifying an account: {err}");
            return create_internal_server_error_response(format!(
                "failed creating a new session: {err}"
            ));
        }
    };
    create_auth_response("Your user account has been verified. The servers email address needs to be set up so that actual verification can take place", session_information)
}

pub async fn log_out(session: AuthSession, State(db): State<Arc<Database>>) -> Response<Body> {
    match db.remove_entry(DatabaseTypes::Session, session.0.session_id) {
        Ok(()) => (StatusCode::ACCEPTED, "Successfully logged out!").into_response(),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to log out properly due to: {err}"),
        )
            .into_response(),
    }
}

pub async fn refresh_session(
    session: AuthRefreshSession,
    State(db): State<Arc<Database>>,
) -> Response<Body> {
    let tokens: CreatedTokensReadable = match db.setup_new_transaction(|conn| {
        let tokens = update_existing_session(conn, session.0)?;
        Ok(tokens)
    }) {
        Ok(new_tokens) => new_tokens,
        Err(err) => {
            return create_internal_server_error_response(format!(
                "Failed the transaction due to: {err}"
            ));
        }
    };
    create_auth_response("refreshed session", tokens)
}

pub async fn change_password(
    session: AuthSession,
    State(db): State<Arc<Database>>,
    request: ChangePasswordRequest,
) -> Response<Body> {
    if request.new_password != request.new_password_repeat {
        return create_internal_server_error_response("Password mismatch".to_string());
    }
    match db.setup_new_transaction(|conn| {
        let user = Database::get_entry_with_connection::<UserMetadata>(conn, session.0.user_id)?;
        validate_submitted_password(&user, &request.current_password)?;
        update_password_to_new_password(conn, request, user.user_id)?;
        Ok(())
    }) {
        Ok(()) => (),
        Err(err) => {
            return create_internal_server_error_response(format!(
                "Failed to change the password for the following reason: {err}"
            ))
        }
    };
    (StatusCode::ACCEPTED, "Password has been changed").into_response()
}

pub async fn change_username(
    session: AuthSession,
    State(db): State<Arc<Database>>,
    request: ChangeUserNameRequest,
) -> Response<Body> {
    match db.setup_new_transaction(|conn| {
        Database::update_value_with_connection::<UserMetadata>(
            conn,
            UserDatabaseColumns::Username,
            &request.new_username,
            session.0.user_id,
        )?;
        Ok(())
    }) {
        Ok(()) => (),
        Err(err) => {
            return create_internal_server_error_response(format!(
                "Failed to update username due to: {err}"
            ))
        }
    };

    (StatusCode::ACCEPTED, "Username has been changed").into_response()
}

pub async fn change_email(
    session: AuthSession,
    State(db): State<Arc<Database>>,
    request: ChangeEmailRequest,
) -> Response<Body> {
    match db.setup_new_transaction(|conn| {
        Database::update_value_with_connection::<UserMetadata>(
            conn,
            UserDatabaseColumns::Email,
            &request.new_email,
            session.0.user_id,
        )?;
        Ok(())
    }) {
        Ok(()) => (),
        Err(err) => {
            return create_internal_server_error_response(format!(
                "Failed to update email due to: {err}"
            ))
        }
    };

    (StatusCode::ACCEPTED, "Email has been changed").into_response()
}

pub async fn reset_password(
    _request: ResettingUsersPassword,
    State(_db): State<Arc<Database>>,
) -> Response<Body> {
    // Planned flow:
    // - get the user using the email address
    // - check for an existing reset-password entry using the user id
    // - check that the attempt count is below five
    // - check that the reset code matches the entry's hashed code
    //   - if yes, change the password
    //   - if no, increment the attempt count and return an unauthorized response
    (
        StatusCode::NOT_IMPLEMENTED,
        "Needs the front end structure in place first so this can match the structure",
    )
        .into_response()
}

pub async fn request_reset_password_link(
    request: RequestPasswordResetLinkRequest,
    State(db): State<Arc<Database>>,
) -> Response<Body> {
    let tokens = match db.setup_new_transaction(|conn| {
        let user = search_for_user_using_email_or_username(conn, &request.email)?;
        let reset_and_cancelation_token = generate_password_reset_codes()?;
        store_user_password_reset_token(conn, &reset_and_cancelation_token, user)?;
        Ok(reset_and_cancelation_token)
    }) {
        Ok(tokens) => tokens,
        Err(err) => {
            return create_internal_server_error_response(format!(
                "Could not create reset password link due to: {err}"
            ))
        }
    };

    send_reset_password_email(
        &tokens.reset_token,
        &tokens.cancelation_token,
        &Url::parse("https://invalid.link").unwrap(),
        &request.email,
    )
    .await
    .map_err(|err| return create_internal_server_error_response(err.to_string()));
    (StatusCode::ACCEPTED, "Email has been sent").into_response()
}

fn store_user_password_reset_token(
    conn: &Connection,
    tokens: &ResetAndCancelationTokens,
    user: UserMetadata,
) -> Result<(), DatabaseError> {
    Database::remove_entry_with_connection_based_on_columns::<ResetPasswordRequest>(
        conn,
        &[ResetPasswordRequestColumns::UserID],
        &[&user.user_id],
        QuerySeparator::And,
    )?;
    let expiration_time = SystemTime::now()
        .checked_add(Duration::from_mins(15))
        .ok_or(return Err(DatabaseError::OperationFailure))?;
    Database::insert_with_connection(
        conn,
        &ResetPasswordRequest {
            user_id: user.user_id,
            reset_token: tokens.reset_token_hashed,
            cancelation_token: tokens.cancelation_token_hashed,
            attempt_count: 0,
            email_sent_at: SystemTime::UNIX_EPOCH,
            expiration_date: expiration_time,
            invalidated: false,
        },
    )?;
    Ok(())
}
