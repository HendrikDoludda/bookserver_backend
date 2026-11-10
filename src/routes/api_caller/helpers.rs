use axum::{
    body::Body,
    http::{header, Response, StatusCode},
    response::IntoResponse,
};

/// Simple liveness check. Also used as the placeholder handler for routes
/// whose backing functionality doesn't exist yet.
pub async fn ping_server() -> impl IntoResponse {
    (StatusCode::OK, "pong")
}

/// For answers that carry tokens: no client or proxy may keep a copy.
pub(super) fn no_store(body: impl IntoResponse) -> Response<Body> {
    ([(header::CACHE_CONTROL, "no-store")], body).into_response()
}
