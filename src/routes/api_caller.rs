//all functions need authentication header eventually. so that only logged in users can make changes to the backend
//TODO: Requires a database table for directories that should be scannable.
//TODO: The table still needs a link for the metadata agent result
//TODO: Check if the file paths that get served are inside the database table for scannable files or the cover images folder

use axum::{
    body::Body,
    extract::{FromRequestParts, Path, State},
    http::{Request, Response, StatusCode},
    response::IntoResponse,
    Json,
};
use rusqlite::Connection;
use serde_json::error::Category::Data;
use std::{
    alloc::System,
    sync::Arc,
    time::{Duration, SystemTime},
};
use url::Url;

use crate::{
    data_models::authentication_model::{
        AuthResponse, ChangeEmailRequest, ChangePasswordRequest, RequestPasswordResetLinkRequest,
        ResettingUsersPassword,
    },
    folder_scanner::scan_all_folders,
    models::{
        QuerySeparator, ResetPasswordRequest, ResetPasswordRequestColumns, SelectionMethod,
        UserDatabaseColumns,
    },
    routes::auth::{
        create_new_user, create_verification_email, generate_password_reset_codes,
        search_for_user_using_email_or_username, send_reset_password_email, validate_new_user,
        ResetAndCancelationTokens,
    },
};
use crate::{
    data_models::authentication_model::{ChangeUserNameRequest, VerifyEmailRequest},
    error_types::{DatabaseError, RequestErrors},
    routes::auth::{update_password_to_new_password, validate_submitted_password},
};
use crate::{
    data_models::authentication_model::{DeviceInformation, LoginRequest, UserCreationRequest},
    routes::auth::{update_existing_session, UserTotpEnabledResult},
};
use crate::{
    data_models::models::{
        BookMetadata, BookSeriesMetadata, DatabaseTypes, LibraryMetadata, WithId,
    },
    routes::auth::{
        create_new_session, log_in, send_verification_email, verify_verification_code,
        CreatedTokensReadable,
    },
};
use crate::{
    db::Database,
    error_types::EmailErrors::{self},
};
use crate::{
    error_types::{
        AppErrors,
        AuthenticationError::{ExpiredSession, UnautherizedSession},
    },
    models::{Sessions, SessionsDatabaseColumns, UserMetadata},
    stream_reader::streaming_file,
};

use tower_cookies::{cookie, Cookies};

pub struct AdminSession(pub Sessions);

impl FromRequestParts<Arc<Database>> for AdminSession {
    type Rejection = RequestErrors;

    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        db: &Arc<Database>,
    ) -> Result<Self, Self::Rejection> {
        let AuthSession(session) = AuthSession::from_request_parts(parts, db).await?;
        let user = db
            .get_entry::<UserMetadata>(session.user_id)
            .map_err(|_| RequestErrors::RequestFailed)?;
        if !user.is_admin {
            return Err(RequestErrors::Forbidden);
        }
        Ok(AdminSession(session))
    }
}

pub struct AuthSession(pub Sessions);

impl FromRequestParts<Arc<Database>> for AuthSession {
    type Rejection = RequestErrors;

    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        db: &Arc<Database>,
    ) -> Result<Self, Self::Rejection> {
        let cookies = Cookies::from_request_parts(parts, db)
            .await
            .map_err(|_| RequestErrors::RequestFailed)?;
        let session = authorized(cookies, db)?;
        Ok(AuthSession(session))
    }
}

fn authorized(cookies: Cookies, db: &Arc<Database>) -> Result<Sessions, RequestErrors> {
    let session_token_cookie = cookies
        .get("session_token")
        .ok_or(RequestErrors::AuthorizationFailed)?;
    let hashed_session_token = blake3::hash(session_token_cookie.value().as_bytes())
        .to_hex()
        .to_string();
    let session: Sessions = db
        .setup_new_transaction(|conn| {
            let session = Database::search_for_single_row_with_connection::<Sessions>(
                conn,
                &[SessionsDatabaseColumns::SessionToken],
                &[&hashed_session_token],
                crate::models::QuerySeparator::And,
                crate::models::SelectionMethod::Everything,
            )?
            .ok_or(AppErrors::Authentication(UnautherizedSession))?;
            if session.session_token_expiration_date < SystemTime::now() {
                return Err(AppErrors::Authentication(ExpiredSession));
            }
            if !session.authentication_completed {
                return Err(AppErrors::Authentication(UnautherizedSession));
            }
            Ok(session)
        })
        .map_err(|err| match err {
            AppErrors::Authentication(_) => RequestErrors::AuthorizationFailed, // 401
            other => {
                log::error!("authorization transaction failed: {other}");
                RequestErrors::RequestFailed // 500
            }
        })?;
    Ok(session)
}

pub struct AuthTOTPLoginSession(pub Sessions);

impl FromRequestParts<Arc<Database>> for AuthTOTPLoginSession {
    type Rejection = RequestErrors;
    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        db: &Arc<Database>,
    ) -> Result<Self, Self::Rejection> {
        let cookies = Cookies::from_request_parts(parts, db)
            .await
            .map_err(|_| RequestErrors::RequestFailed)?;
        let session = validate_totp_login_session(cookies, db)?;
        Ok(AuthTOTPLoginSession(session))
    }
}

fn validate_totp_login_session(
    cookies: Cookies,
    db: &Arc<Database>,
) -> Result<Sessions, RequestErrors> {
    let session_token_cookie = cookies
        .get("session_token")
        .ok_or(RequestErrors::AuthorizationFailed)?;
    let hashed_session_token = blake3::hash(session_token_cookie.value().as_bytes())
        .to_hex()
        .to_string();
    let session: Sessions = db
        .setup_new_transaction(|conn| {
            let session = Database::search_for_single_row_with_connection::<Sessions>(
                conn,
                &[SessionsDatabaseColumns::SessionToken],
                &[&hashed_session_token],
                crate::models::QuerySeparator::And,
                crate::models::SelectionMethod::Everything,
            )?
            .ok_or(AppErrors::Authentication(UnautherizedSession))?;
            if session.session_token_expiration_date < SystemTime::now() {
                return Err(AppErrors::Authentication(ExpiredSession));
            }
            if session.authentication_completed {
                return Err(AppErrors::Authentication(UnautherizedSession));
            }
            Ok(session)
        })
        .map_err(|err| match err {
            AppErrors::Authentication(_) => RequestErrors::AuthorizationFailed, // 401
            other => {
                log::error!("authorization transaction failed: {other}");
                RequestErrors::RequestFailed // 500
            }
        })?;
    Ok(session)
}

pub struct AuthRefreshSession(pub Sessions);

impl FromRequestParts<Arc<Database>> for AuthRefreshSession {
    type Rejection = RequestErrors;
    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        db: &Arc<Database>,
    ) -> Result<Self, Self::Rejection> {
        let cookies = Cookies::from_request_parts(parts, db)
            .await
            .map_err(|_| RequestErrors::RequestFailed)?;
        let session = refresh_authorized(cookies, db)?;
        Ok(AuthRefreshSession(session))
    }
}

fn refresh_authorized(cookies: Cookies, db: &Arc<Database>) -> Result<Sessions, RequestErrors> {
    let refresh_token_cookie = cookies
        .get("refresh_token")
        .ok_or(RequestErrors::AuthorizationFailed)?;
    let hashed_refresh_token = blake3::hash(refresh_token_cookie.value().as_bytes())
        .to_hex()
        .to_string();
    let session: Sessions = db
        .setup_new_transaction(|conn| {
            let session = Database::search_for_single_row_with_connection::<Sessions>(
                conn,
                &[SessionsDatabaseColumns::RefreshToken],
                &[&hashed_refresh_token],
                crate::models::QuerySeparator::And,
                crate::models::SelectionMethod::Everything,
            )?
            .ok_or(AppErrors::Authentication(UnautherizedSession))?;
            if session.refresh_token_expiration_date < SystemTime::now() {
                return Err(AppErrors::Authentication(ExpiredSession));
            }
            if !session.authentication_completed {
                return Err(AppErrors::Authentication(UnautherizedSession));
            }
            Ok(session)
        })
        .map_err(|err| match err {
            AppErrors::Authentication(_) => RequestErrors::AuthorizationFailed, // 401
            other => {
                log::error!("authorization transaction failed: {other}");
                RequestErrors::RequestFailed // 500
            }
        })?;
    Ok(session)
}

// Simple liveness check. Also used as the placeholder handler for routes
// whose backing functionality doesn't exist yet.
pub async fn ping_server() -> impl IntoResponse {
    (StatusCode::OK, "pong")
}

// === Implemented against existing functionality =============================

// Return every library, each paired with its row id.
// TODO: gate behind auth once sessions exist.
pub async fn get_libraries(
    State(db): State<Arc<Database>>,
) -> Result<Json<Vec<WithId<LibraryMetadata>>>, DatabaseError> {
    let ids = db.get_all_libraries()?;
    let libraries = db.get_entries_batch::<LibraryMetadata>(&ids)?;

    Ok(Json(libraries))
}

// Return every series that belongs to the given library, each with its row id.
pub async fn get_series_in_library(
    State(db): State<Arc<Database>>,
    Path(library_id): Path<i64>,
) -> Result<Json<Vec<WithId<BookSeriesMetadata>>>, DatabaseError> {
    let series_ids = db.get_series_entries_in_library(library_id)?;
    let series = db.get_entries_batch::<BookSeriesMetadata>(&series_ids)?;

    Ok(Json(series))
}

// Return every book that belongs to the given series, each with its row id.
pub async fn get_series_children(
    State(db): State<Arc<Database>>,
    Path(series_id): Path<i64>,
) -> Result<Json<Vec<WithId<BookMetadata>>>, DatabaseError> {
    let book_ids = db.get_books_in_series(series_id)?;
    let books = db.get_entries_batch::<BookMetadata>(&book_ids)?;

    Ok(Json(books))
}

// Stream a book file (with HTTP range support, handled inside streaming_file).
pub async fn request_file(
    State(db): State<Arc<Database>>,
    Path(id): Path<i64>,
    request: Request<Body>,
) -> Result<Response<Body>, DatabaseError> {
    //should be able to handle images (cover images) as well as books and partial books (as well as folders with images, like an unzipped cbz file)
    //the image comics probably need to have their height pre-calculated in order for correct display conditions
    let metadata = db.get_entry::<BookMetadata>(id)?;
    Ok(streaming_file(&metadata, request).await)
}

// Return a single book's metadata without streaming the file itself.
pub async fn retrieve_metadata(
    State(db): State<Arc<Database>>,
    Path(id): Path<i64>,
) -> Result<Json<BookMetadata>, DatabaseError> {
    //preferably can return the metadata from all metadata types
    let metadata = db.get_entry::<BookMetadata>(id)?;
    Ok(Json(metadata))
}

// (Re)run migrations / ensure the database is set up.
pub async fn initiate_database() -> Result<&'static str, DatabaseError> {
    Database::new()?;
    Ok("Database initialized")
}

// Kick off a scan of every configured book directory.
pub async fn scan_all_directories(State(db): State<Arc<Database>>) -> impl IntoResponse {
    match scan_all_folders(db).await {
        Ok(()) => (StatusCode::OK, "Directory scan complete".to_string()),
        Err(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Directory scan failed: {error}"),
        ),
    }
}

// Delete a library row by id.
pub async fn delete_library(
    State(db): State<Arc<Database>>,
    Path(id): Path<i64>,
) -> Result<&'static str, DatabaseError> {
    db.remove_entry(DatabaseTypes::Library, id)?;
    Ok("Library deleted")
}

// === Not yet implementable — waiting on backing functionality ===============

pub async fn update_database_entry(session: AuthSession) -> impl IntoResponse {
    // Blocked: models don't derive Deserialize, and the request shape is undecided.
    return failed_getting_response();
}

pub async fn insert_database_entry(session: AuthSession) -> impl IntoResponse {
    // Blocked: models don't derive Deserialize, and the request shape is undecided.
    return failed_getting_response();
}

pub async fn scan_for_metadata(session: AuthSession) -> impl IntoResponse {
    return failed_getting_response();
}

fn failed_getting_response() -> Response<Body> {
    return Response::new("Failed to get response".into());
}

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
                "Could not create a user due to: {}",
                err.to_string()
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
        Ok(()) => {
            return Response::builder()
                .status(StatusCode::ACCEPTED)
                .body("Created a user and sent a verification email.".into())
                .unwrap()
        }
        Err(EmailErrors::SentFailedDueToNoConfig { code }) => {
            auto_verify_account(request.email, code, &db, email_info.user.user_id).await
        }
        Err(err) => {
            return create_internal_server_error_response(format!(
                "Failed to send verification email due to: {}",
                err.to_string()
            ));
        }
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
                "Failed to sign in due to: {}",
                err.to_string()
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
        Ok(()) => (StatusCode::ACCEPTED, format!("Successfully logged out!")).into_response(),
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
        return create_internal_server_error_response(format!("Password mismatch"));
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
    request: ResettingUsersPassword,
    State(db): State<Arc<Database>>,
) -> Response<Body> {
    (
        //get the user using the email address
        //check for existing reset password entry using user id
        //check if attempt amount is less than five
        //check if reset code matches the hashed entries code
        //---->if yes change password
        //---->if no up the attempt amount and return unautherized message
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

pub async fn totp_start_set_up() -> Response<Body> {} //needs the normal autherization

pub async fn finalize_totp_set_up() -> Response<Body> {} //normal autherization

pub async fn disable_totp() -> Response<Body> {} //normal autherization

pub async fn sign_in_totp_with_recovery_code() -> Response<Body> {} //totp sign in autherization

pub async fn sign_in_totp() -> Response<Body> {} //totp sign in autherization

//-------------------------Authentication Helpers----------------------------------//

fn create_internal_server_error_response(body: String) -> Response<Body> {
    return Response::builder()
        .status(StatusCode::INTERNAL_SERVER_ERROR)
        .body(body.into())
        .unwrap();
}

fn create_unautherized_error_response(body: String) -> Response<Body> {
    return Response::builder()
        .status(StatusCode::UNAUTHORIZED)
        .body(body.into())
        .unwrap();
}

fn create_auth_response(
    message: impl Into<String>,
    tokens: CreatedTokensReadable,
) -> Response<Body> {
    (
        StatusCode::ACCEPTED,
        Json(AuthResponse {
            message: message.into(),
            session_token: tokens.session_token,
            refresh_token: tokens.refresh_token,
            session_token_expiration: tokens.session_token_valid_until,
            refresh_token_expiration_time: tokens.refresh_token_valid_until,
        }),
    )
        .into_response()
}
