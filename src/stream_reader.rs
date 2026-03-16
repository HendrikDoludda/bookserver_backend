use axum::{
    body::Body,
    http::{Request},
    response::{IntoResponse, Response},
};
use tower::ServiceExt; // for `oneshot`
use tower_http::services::ServeFile;
use crate::models::BookMetadata;

pub async fn streaming_file(
    metadata: &BookMetadata,
    request: Request<Body>
) -> Response<Body> {
    let service: ServeFile = ServeFile::new(&metadata.file_path);
    service
        .oneshot(request)
        .await
        .unwrap()
        .into_response()
}