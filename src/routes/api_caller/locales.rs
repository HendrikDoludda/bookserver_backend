//! The translation catalog's handlers. Neither takes a session: the login screen needs its texts
//! before anyone has signed in.

use std::sync::Arc;

use axum::{extract::State, Json};

use crate::{
    data_models::translations_model::{Locale, Translations, TranslationsQuery},
    db::Database,
    error_types::AppErrors,
    routes::translations,
};

use super::extractors::{PathParams, QueryParams};

/// `GET /api/v1/locales`: the languages the server offers, in their order.
pub async fn list_locales(State(db): State<Arc<Database>>) -> Result<Json<Vec<Locale>>, AppErrors> {
    Ok(Json(translations::locales(db).await?))
}

/// `GET /api/v1/locales/{code}/translations`: the language's texts for the apps.
pub async fn get_translations(
    State(db): State<Arc<Database>>,
    PathParams(code): PathParams<String>,
    QueryParams(query): QueryParams<TranslationsQuery>,
) -> Result<Json<Translations>, AppErrors> {
    Ok(Json(
        translations::translations(db, code, query.sections).await?,
    ))
}
