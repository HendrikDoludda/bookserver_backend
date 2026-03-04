use axum::{
    body::Body,
    http::{Request, StatusCode},
    response::IntoResponse,
};
use tower::ServiceExt; // for `oneshot`
use tower_http::services::ServeFile;
use crate::models::BookMetadata;

pub async fn streaming_file(
    metadata: &BookMetadata,
    request: Request<Body>
) -> impl IntoResponse {
    let service = ServeFile::new(&metadata.file_path);
    service
        .oneshot(request)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}