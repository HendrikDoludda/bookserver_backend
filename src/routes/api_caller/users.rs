//! The admin's user management.

use std::sync::Arc;

use axum::{extract::State, Json};

use crate::{
    data_models::users_model::{UserChanges, UserSummary},
    db::Database,
    error_types::AppErrors,
    models::WithId,
    routes::user_management::{all_users, change_user},
};

use super::extractors::{AdminSession, JsonBody, PathParams};

/// `GET /api/v1/users`
pub async fn list_users(
    _admin: AdminSession,
    State(db): State<Arc<Database>>,
) -> Result<Json<Vec<WithId<UserSummary>>>, AppErrors> {
    Ok(Json(all_users(db).await?))
}

/// `PATCH /api/v1/users/{id}` with `{"role"?, "email_verified"?}`: answers the changed user.
pub async fn update_user(
    _admin: AdminSession,
    State(db): State<Arc<Database>>,
    PathParams(user_id): PathParams<i64>,
    JsonBody(changes): JsonBody<UserChanges>,
) -> Result<Json<WithId<UserSummary>>, AppErrors> {
    Ok(Json(change_user(db, user_id, changes).await?))
}
