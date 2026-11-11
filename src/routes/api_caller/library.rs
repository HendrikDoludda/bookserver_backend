//! TODO: needs a database table of scannable directories (replacing `BOOK_DIRS`),
//!       with a link to the metadata-agent result.
//! TODO: check that served file paths lie inside a scannable directory or the
//!       cover-images folder.

use std::sync::Arc;

use axum::{
    body::Body,
    extract::State,
    http::{Request, Response, StatusCode},
    response::IntoResponse,
    Json,
};

use crate::{
    db::Database,
    error_types::{internal_error_response, DatabaseError},
    folder_scanner::scan_all_folders,
    models::{BookMetadata, BookSeriesMetadata, DatabaseTypes, LibraryMetadata, WithId},
    stream_reader::streaming_file,
};

use super::extractors::{AdminSession, AuthSession, PathParams};

// === Library, series & book handlers =========================================

/// Return every library, each paired with its row id.
pub async fn get_libraries(
    _session: AuthSession,
    State(db): State<Arc<Database>>,
) -> Result<Json<Vec<WithId<LibraryMetadata>>>, DatabaseError> {
    let ids = db.get_all_libraries()?;
    let libraries = db.get_entries_batch::<LibraryMetadata>(&ids)?;

    Ok(Json(libraries))
}

/// Return every series that belongs to the given library, each with its row id.
pub async fn get_series_in_library(
    _session: AuthSession,
    State(db): State<Arc<Database>>,
    PathParams(library_id): PathParams<i64>,
) -> Result<Json<Vec<WithId<BookSeriesMetadata>>>, DatabaseError> {
    let series_ids = db.get_series_entries_in_library(library_id)?;
    let series = db.get_entries_batch::<BookSeriesMetadata>(&series_ids)?;

    Ok(Json(series))
}

/// Return every book that belongs to the given series, each with its row id.
pub async fn get_series_children(
    _session: AuthSession,
    State(db): State<Arc<Database>>,
    PathParams(series_id): PathParams<i64>,
) -> Result<Json<Vec<WithId<BookMetadata>>>, DatabaseError> {
    let book_ids = db.get_books_in_series(series_id)?;
    let books = db.get_entries_batch::<BookMetadata>(&book_ids)?;

    Ok(Json(books))
}

/// Stream a book file (with HTTP range support, handled inside `streaming_file`).
pub async fn request_file(
    _session: AuthSession,
    State(db): State<Arc<Database>>,
    PathParams(id): PathParams<i64>,
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
    _session: AuthSession,
    State(db): State<Arc<Database>>,
    PathParams(id): PathParams<i64>,
) -> Result<Json<BookMetadata>, DatabaseError> {
    // TODO: return metadata for every metadata type, not just books.
    let metadata = db.get_entry::<BookMetadata>(id)?;
    Ok(Json(metadata))
}

/// Kick off a scan of every configured book directory.
pub async fn scan_all_directories(
    _admin: AdminSession,
    State(db): State<Arc<Database>>,
) -> Response<Body> {
    match scan_all_folders(db).await {
        Ok(()) => (StatusCode::OK, "Directory scan complete").into_response(),
        Err(error) => {
            tracing::error!("directory scan failed: {error:#}");
            internal_error_response()
        }
    }
}

/// Delete a library row by id.
pub async fn delete_library(
    _admin: AdminSession,
    State(db): State<Arc<Database>>,
    PathParams(id): PathParams<i64>,
) -> Result<&'static str, DatabaseError> {
    db.remove_entry(DatabaseTypes::Library, id)?;
    Ok("Library deleted")
}
