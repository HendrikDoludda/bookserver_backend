use axum::{
    body::Body,
    http::{Response, StatusCode},
    response::IntoResponse,
    Json,
};

use crate::{
    data_models::authentication_model::AuthResponse, error_types::error_response,
    routes::auth::CreatedTokensReadable,
};

/// Simple liveness check. Also used as the placeholder handler for routes
/// whose backing functionality doesn't exist yet.
pub async fn ping_server() -> impl IntoResponse {
    (StatusCode::OK, "pong")
}

pub(super) fn not_implemented_response() -> Response<Body> {
    error_response(StatusCode::NOT_IMPLEMENTED, "not_implemented")
}

pub(super) fn create_auth_response(
    message: impl Into<String>,
    tokens: CreatedTokensReadable,
) -> Response<Body> {
    (
        StatusCode::ACCEPTED,
        Json(AuthResponse {
            message: message.into(),
            session_token: tokens.session_token,
            refresh_token: tokens.refresh_token,
            session_token_expiration: tokens.session_token_valid_until,
            refresh_token_expiration_time: tokens.refresh_token_valid_until,
        }),
    )
        .into_response()
}
