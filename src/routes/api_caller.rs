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

use crate::db::Database;
use crate::error_types::{DatabaseError, RequestErrors};
use crate::folder_scanner::scan_all_folders;
use crate::models::{BookMetadata, BookSeriesMetadata, DatabaseTypes, LibraryMetadata, WithId};
use crate::stream_reader::streaming_file;

fn authorized() -> Result<(), RequestErrors> {
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

    let mut libraries = Vec::with_capacity(ids.len());
    for id in ids {
        libraries.push(WithId {
            id,
            data: db.get_entry::<LibraryMetadata>(id)?,
        });
    }

    Ok(Json(libraries))
}

// Return every series that belongs to the given library, each with its row id.
pub async fn get_series_in_library(
    State(db): State<Arc<Database>>,
    Path(library_id): Path<i64>,
) -> Result<Json<Vec<WithId<BookSeriesMetadata>>>, DatabaseError> {
    let series_ids = db.get_series_entries_in_library(library_id)?;

    let mut series = Vec::with_capacity(series_ids.len());
    for id in series_ids {
        series.push(WithId {
            id,
            data: db.get_entry::<BookSeriesMetadata>(id)?,
        });
    }

    Ok(Json(series))
}

// Return every book that belongs to the given series, each with its row id.
pub async fn get_series_children(
    State(db): State<Arc<Database>>,
    Path(series_id): Path<i64>,
) -> Result<Json<Vec<WithId<BookMetadata>>>, DatabaseError> {
    let book_ids = db.get_books_in_series(series_id)?;

    let mut books = Vec::with_capacity(book_ids.len());
    for id in book_ids {
        books.push(WithId {
            id,
            data: db.get_entry::<BookMetadata>(id)?,
        });
    }

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
