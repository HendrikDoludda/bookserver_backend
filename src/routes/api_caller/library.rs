//! TODO: every mutating handler should eventually require an `AuthSession`.
//! TODO: needs a database table of scannable directories (replacing `BOOK_DIRS`),
//!       with a link to the metadata-agent result.
//! TODO: check that served file paths lie inside a scannable directory or the
//!       cover-images folder.

use std::sync::Arc;

use axum::{
    body::Body,
    extract::{Path, State},
    http::{Request, Response, StatusCode},
    response::IntoResponse,
    Json,
};

use crate::{
    db::Database,
    error_types::DatabaseError,
    folder_scanner::scan_all_folders,
    models::{BookMetadata, BookSeriesMetadata, DatabaseTypes, LibraryMetadata, WithId},
    stream_reader::streaming_file,
};

use super::extractors::AuthSession;

// === Library, series & book handlers =========================================

/// Return every library, each paired with its row id.
// TODO: require an `AuthSession`.
pub async fn get_libraries(
    State(db): State<Arc<Database>>,
) -> Result<Json<Vec<WithId<LibraryMetadata>>>, DatabaseError> {
    let ids = db.get_all_libraries()?;
    let libraries = db.get_entries_batch::<LibraryMetadata>(&ids)?;

    Ok(Json(libraries))
}

/// Return every series that belongs to the given library, each with its row id.
pub async fn get_series_in_library(
    State(db): State<Arc<Database>>,
    Path(library_id): Path<i64>,
) -> Result<Json<Vec<WithId<BookSeriesMetadata>>>, DatabaseError> {
    let series_ids = db.get_series_entries_in_library(library_id)?;
    let series = db.get_entries_batch::<BookSeriesMetadata>(&series_ids)?;

    Ok(Json(series))
}

/// Return every book that belongs to the given series, each with its row id.
pub async fn get_series_children(
    State(db): State<Arc<Database>>,
    Path(series_id): Path<i64>,
) -> Result<Json<Vec<WithId<BookMetadata>>>, DatabaseError> {
    let book_ids = db.get_books_in_series(series_id)?;
    let books = db.get_entries_batch::<BookMetadata>(&book_ids)?;

    Ok(Json(books))
}

/// Stream a book file (with HTTP range support, handled inside `streaming_file`).
pub async fn request_file(
    State(db): State<Arc<Database>>,
    Path(id): Path<i64>,
    request: Request<Body>,
) -> Result<Response<Body>, DatabaseError> {
    // TODO: also serve cover images, partial books, and folders of images (e.g. an
    // unzipped CBZ). Image comics will likely need page heights pre-calculated so the
    // reader can lay them out correctly.
    let metadata = db.get_entry::<BookMetadata>(id)?;
    Ok(streaming_file(&metadata, request).await)
}

/// Return a single book's metadata without streaming the file itself.
pub async fn retrieve_metadata(
    State(db): State<Arc<Database>>,
    Path(id): Path<i64>,
) -> Result<Json<BookMetadata>, DatabaseError> {
    // TODO: return metadata for every metadata type, not just books.
    let metadata = db.get_entry::<BookMetadata>(id)?;
    Ok(Json(metadata))
}

/// (Re)run migrations / ensure the database is set up.
pub async fn initiate_database() -> Result<&'static str, DatabaseError> {
    Database::new()?;
    Ok("Database initialized")
}

/// Kick off a scan of every configured book directory.
pub async fn scan_all_directories(State(db): State<Arc<Database>>) -> impl IntoResponse {
    match scan_all_folders(db).await {
        Ok(()) => (StatusCode::OK, "Directory scan complete".to_string()),
        Err(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Directory scan failed: {error}"),
        ),
    }
}

/// Delete a library row by id.
pub async fn delete_library(
    State(db): State<Arc<Database>>,
    Path(id): Path<i64>,
) -> Result<&'static str, DatabaseError> {
    db.remove_entry(DatabaseTypes::Library, id)?;
    Ok("Library deleted")
}

// === Not yet implemented =====================================================

// Blocked: the request shape is undecided.
pub async fn update_database_entry(_session: AuthSession) -> impl IntoResponse {
    failed_getting_response()
}

// Blocked: the request shape is undecided.
pub async fn insert_database_entry(_session: AuthSession) -> impl IntoResponse {
    failed_getting_response()
}

// Blocked: online metadata fetching doesn't exist yet (see `scanner/metadata_fetcher.rs`).
pub async fn scan_for_metadata(_session: AuthSession) -> impl IntoResponse {
    failed_getting_response()
}

fn failed_getting_response() -> Response<Body> {
    Response::new("Failed to get response".into())
}
