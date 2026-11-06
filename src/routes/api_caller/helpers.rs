use axum::{
    body::Body,
    http::{Response, StatusCode},
    response::IntoResponse,
    Json,
};

use crate::{data_models::authentication_model::AuthResponse, routes::auth::CreatedTokensReadable};

/// Simple liveness check. Also used as the placeholder handler for routes
/// whose backing functionality doesn't exist yet.
pub async fn ping_server() -> impl IntoResponse {
    (StatusCode::OK, "pong")
}

pub(super) fn not_implemented_response() -> Response<Body> {
    (StatusCode::NOT_IMPLEMENTED, "Not implemented yet").into_response()
}

pub(super) fn create_internal_server_error_response(body: String) -> Response<Body> {
    Response::builder()
        .status(StatusCode::INTERNAL_SERVER_ERROR)
        .body(body.into())
        .unwrap()
}

fn create_unauthorized_error_response(body: String) -> Response<Body> {
    Response::builder()
        .status(StatusCode::UNAUTHORIZED)
        .body(body.into())
        .unwrap()
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
