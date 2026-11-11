//! The account handlers: changes users make to their own account.

use std::{sync::Arc, time::SystemTime};

use axum::{extract::State, http::StatusCode};

use crate::{
    data_models::authentication_model::ChangePasswordRequest, db::Database, error_types::AppErrors,
    routes::account_management,
};

use super::extractors::{AuthSession, JsonBody};

/// `PUT /api/v1/account/password`: ends the user's other sessions; this one stays.
pub async fn change_password(
    session: AuthSession,
    State(db): State<Arc<Database>>,
    JsonBody(request): JsonBody<ChangePasswordRequest>,
) -> Result<StatusCode, AppErrors> {
    let AuthSession(session) = session;
    account_management::change_password(
        db,
        session.user_id,
        session.session_id,
        request,
        SystemTime::now(),
    )
    .await?;
    Ok(StatusCode::NO_CONTENT)
}
