use axum::{
    body::Body,
    http::{header, Response, StatusCode},
    response::IntoResponse,
};

use crate::error_types::error_response;

/// Simple liveness check. Also used as the placeholder handler for routes
/// whose backing functionality doesn't exist yet.
pub async fn ping_server() -> impl IntoResponse {
    (StatusCode::OK, "pong")
}

pub(super) fn not_implemented_response() -> Response<Body> {
    error_response(StatusCode::NOT_IMPLEMENTED, "not_implemented")
}

/// For answers that carry tokens: no client or proxy may keep a copy.
pub(super) fn no_store(body: impl IntoResponse) -> Response<Body> {
    ([(header::CACHE_CONTROL, "no-store")], body).into_response()
}
