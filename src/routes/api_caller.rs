//all functions need authentication header eventually. so that only logged in users can make changes to the backend
//TODO: Requires a database table for directories that should be scannable.
//TODO: The table still needs a link for the metadata agent result
//TODO: Check if the file paths that get served are inside the database table for scannable files or the cover images folder

use axum::{body::Body, http::Response, response::IntoResponse};
use tower_http::auth;

use crate::error_types::RequestErrors;

fn authorized() -> Result<(), RequestErrors> {
    Ok(())
}

pub async fn ping_server() -> impl IntoResponse {
    if authorized().is_ok() {}
    return failed_getting_response();
}

pub async fn get_libraries() -> impl IntoResponse {
    if authorized().is_ok() {}
    return failed_getting_response();
}

pub async fn get_series_in_library() -> impl IntoResponse {
    if authorized().is_ok() {}
    return failed_getting_response();
}

pub async fn get_series_children() -> impl IntoResponse {
    if authorized().is_ok() {}
    return failed_getting_response();
}

pub async fn request_file() -> impl IntoResponse {
    //should be able to handle images (cover images) as well as books and partial books (as well as folders with images, like an unzipped cbz file)
    //the image comics probably need to have their height pre-calculated in order for correct display conditions
    if authorized().is_ok() {}
    return failed_getting_response();
}

pub async fn initiate_database() -> impl IntoResponse {
    if authorized().is_ok() {}
    return failed_getting_response();
}

pub async fn scan_all_directories() -> impl IntoResponse {
    if authorized().is_ok() {}
    return failed_getting_response();
}

pub async fn update_database_entry() -> impl IntoResponse {
    if authorized().is_ok() {}
    return failed_getting_response();
}

pub async fn insert_database_entry() -> impl IntoResponse {
    if authorized().is_ok() {}
    return failed_getting_response();
}

pub async fn retrieve_metadata() -> impl IntoResponse {
    //preferably can return the metadata from all metadata types
    if authorized().is_ok() {}
    return failed_getting_response();
}

pub async fn scan_for_metadata() -> impl IntoResponse {
    if authorized().is_ok() {}
    return failed_getting_response();
}

pub async fn delete_library() -> impl IntoResponse {
    if authorized().is_ok() {}
    return failed_getting_response();
}

fn failed_getting_response() -> Response<Body> {
    return Response::new("Failed to get response".into());
}
