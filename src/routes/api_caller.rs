//all functions need authentication header eventually. so that only logged in users can make changes to the backend
//TODO: Requires a database table for directories that should be scannable.
//TODO: The table still needs a link for the metadata agent result
//TODO: Check if the file paths that get served are inside the database table for scannable files or the cover images folder

use axum::{
    body::Body,
    extract::{Path, State},
    http::{Request, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use std::sync::Arc;

use crate::error_types::{DatabaseError, RequestErrors};
use crate::folder_scanner::scan_all_folders;
use crate::routes::auth;
use crate::stream_reader::streaming_file;
use crate::{data_models::authentication_model::UserCreationRequest, routes::auth::create_user};
use crate::{
    data_models::models::{
        BookMetadata, BookSeriesMetadata, DatabaseTypes, LibraryMetadata, WithId,
    },
    routes::auth::send_verification_email,
};
use crate::{
    db::Database,
    error_types::EmailErrors::{self, EmailSetUpNotFound},
};

fn authorized() -> Result<(), RequestErrors> {
    //check the session token
    //check the expiration date
    //check authentication complete (to see if it completed the final log in step)
    Ok(())
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

pub async fn update_database_entry() -> impl IntoResponse {
    // Blocked: models don't derive Deserialize, and the request shape is undecided.
    if authorized().is_ok() {}
    return failed_getting_response();
}

pub async fn insert_database_entry() -> impl IntoResponse {
    // Blocked: models don't derive Deserialize, and the request shape is undecided.
    if authorized().is_ok() {}
    return failed_getting_response();
}

pub async fn scan_for_metadata() -> impl IntoResponse {
    if authorized().is_ok() {}
    return failed_getting_response();
}

fn failed_getting_response() -> Response<Body> {
    return Response::new("Failed to get response".into());
}

/*pub async fn sign_up(
    State(db): State<Arc<Database>>,
    Json(request): Json<UserCreationRequest>,
) -> Response<Body> {
    // Create the user (validation has already happened)
    let user = match create_user(&db, request).await {
        Ok(user) => user,
        Err(error) => return error.into_response(),
    };

    // Create a short-lived JWT
    let access_token = match create_access_token(user.id) {
        Ok(token) => token,
        Err(error) => return error.into_response(),
    };

    // Create a long-lived refresh token
    let refresh_token = generate_refresh_token();

    // Store the refresh token hash in the database
    if let Err(error) = create_session(
        &db,
        user.id,
        &refresh_token,
        /* device info */
    )
    .await
    {
        return error.into_response();
    }

    Json(AuthResponse {
        access_token,
        refresh_token,
    })
    .into_response()
} */
pub async fn sign_up(
    State(db): State<Arc<Database>>,
    Json(request): Json<UserCreationRequest>,
) -> Response<Body> {
    let user = match create_user(&db, request).await {
        Ok(user) => user,
        Err(err) => {
            return Response::builder()
                .status(StatusCode::BAD_REQUEST)
                .body(format!("Failed Creating User: {}", err.to_string()).into())
                .unwrap()
        }
    };
    let id = match db.insert(&user) {
        Ok(id) => id,
        Err(err) => {
            return Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(format!("Failed inserting user: {}", err.to_string()).into())
                .unwrap()
        }
    };

    let result = send_verification_email(user, &db).await;
    match result {
        Ok(()) => {
            return Response::builder()
                .status(StatusCode::ACCEPTED)
                .body("Created a user and sent a verification email.".into())
                .unwrap()
        }
        Err(EmailErrors::SentFailedDueToNoConfig { code }) => { /*verify email automatically */ }
        Err(err) => {
            return Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(
                    format!(
                        "Failed to send verification email due to: {}",
                        err.to_string()
                    )
                    .into(),
                )
                .unwrap()
        }
    }
    if authorized().is_ok() {}
    return failed_getting_response();
}

pub async fn sign_in() -> Response<Body> {}

pub async fn verify_email() -> Response<Body> {}

pub async fn log_out() -> Response<Body> {}

pub async fn change_password() -> Response<Body> {}

pub async fn change_username() -> Response<Body> {}

pub async fn change_email() -> Response<Body> {}

pub async fn reset_password() -> Response<Body> {}

pub async fn totp_start_set_up() -> Response<Body> {}

pub async fn finalize_totp_set_up() -> Response<Body> {}

pub async fn disable_totp() -> Response<Body> {}

pub async fn sign_in_totp_with_recovery_code() -> Response<Body> {}

pub async fn sign_in_totp() -> Response<Body> {}
