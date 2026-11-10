use axum::{
    body::Body,
    http::{header, Response, StatusCode},
    response::IntoResponse,
};

use crate::error_types::error_response;

/// Simple liveness check.
pub async fn ping_server() -> impl IntoResponse {
    (StatusCode::OK, "pong")
}

/// The answer of every route whose handler isn't built yet.
pub async fn not_implemented() -> Response<Body> {
    error_response(StatusCode::NOT_IMPLEMENTED, "not_implemented")
}

/// The router's answer for a path no route matches.
pub async fn not_found() -> Response<Body> {
    error_response(StatusCode::NOT_FOUND, "not_found")
}

/// The router's answer for a path that exists, but not with this method.
pub async fn method_not_allowed() -> Response<Body> {
    error_response(StatusCode::METHOD_NOT_ALLOWED, "method_not_allowed")
}

/// For answers that carry tokens: no client or proxy may keep a copy.
pub(super) fn no_store(body: impl IntoResponse) -> Response<Body> {
    ([(header::CACHE_CONTROL, "no-store")], body).into_response()
}
