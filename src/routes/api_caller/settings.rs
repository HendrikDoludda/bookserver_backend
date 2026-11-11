//! The settings handlers: a user's own settings.

use std::sync::Arc;

use axum::{extract::State, Json};

use crate::{
    data_models::settings_model::Settings, db::Database, error_types::AppErrors,
    routes::user_settings,
};

use super::extractors::{AuthSession, JsonBody};

/// `GET /api/v1/account/settings`: the defaults while the user has changed none.
pub async fn get_settings(
    session: AuthSession,
    State(db): State<Arc<Database>>,
) -> Result<Json<Settings>, AppErrors> {
    Ok(Json(
        user_settings::read_settings(db, session.0.user_id).await?,
    ))
}

/// `PUT /api/v1/account/settings`: answers the settings as stored.
pub async fn replace_settings(
    session: AuthSession,
    State(db): State<Arc<Database>>,
    JsonBody(settings): JsonBody<Settings>,
) -> Result<Json<Settings>, AppErrors> {
    Ok(Json(
        user_settings::replace_settings(db, session.0.user_id, settings).await?,
    ))
}
