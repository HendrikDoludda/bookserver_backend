//! The translation catalog's handlers. Neither takes a session: the login screen needs its texts
//! before anyone has signed in.

use std::sync::Arc;

use axum::{
    body::Body,
    extract::State,
    http::{
        header::{CACHE_CONTROL, CONTENT_TYPE, ETAG, IF_NONE_MATCH},
        HeaderMap, Response, StatusCode,
    },
    response::IntoResponse,
    Json,
};

use crate::{
    data_models::translations_model::{Locale, TranslationsQuery},
    db::Database,
    error_types::{internal_error_response, AppErrors},
    routes::translations,
};

use super::extractors::{PathParams, QueryParams};

/// `GET /api/v1/locales`: the languages the server offers, in their order.
pub async fn list_locales(State(db): State<Arc<Database>>) -> Result<Json<Vec<Locale>>, AppErrors> {
    Ok(Json(translations::locales(db).await?))
}

/// `GET /api/v1/locales/{code}/translations`: the language's texts for the apps, with an ETag;
/// an `If-None-Match` naming it answers 304 without them.
pub async fn get_translations(
    State(db): State<Arc<Database>>,
    PathParams(code): PathParams<String>,
    QueryParams(query): QueryParams<TranslationsQuery>,
    headers: HeaderMap,
) -> Result<Response<Body>, AppErrors> {
    let translations = translations::translations(db, code, query.sections).await?;
    let body = match serde_json::to_vec(&translations) {
        Ok(body) => body,
        Err(err) => {
            tracing::error!("serializing a translation catalog failed: {err}");
            return Ok(internal_error_response());
        }
    };
    Ok(revalidated_json(&headers, body))
}

/// A JSON answer with its ETag, the BLAKE3 hash of `body`; or 304 without the body when the
/// request's `If-None-Match` names that ETag. `no-cache`: a client may keep the answer, but asks
/// again before it uses it.
fn revalidated_json(request: &HeaderMap, body: Vec<u8>) -> Response<Body> {
    let etag = format!("\"{}\"", blake3::hash(&body).to_hex());
    let cache_headers = [
        (ETAG, etag.clone()),
        (CACHE_CONTROL, "no-cache".to_string()),
    ];
    if none_match_names(request, &etag) {
        (StatusCode::NOT_MODIFIED, cache_headers).into_response()
    } else {
        (cache_headers, [(CONTENT_TYPE, "application/json")], body).into_response()
    }
}

/// Whether the request's `If-None-Match` names `etag`: `*`, or one of its tags, strong or weak
/// (`W/"…"`) — RFC 9110 compares this header's tags by their quoted value alone.
fn none_match_names(request: &HeaderMap, etag: &str) -> bool {
    request
        .get_all(IF_NONE_MATCH)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(','))
        .map(str::trim)
        .any(|tag| tag == "*" || tag.strip_prefix("W/").unwrap_or(tag) == etag)
}

#[cfg(test)]
mod tests {
    use axum::http::HeaderValue;

    use super::*;

    fn names(values: &[&str], etag: &str) -> bool {
        let mut headers = HeaderMap::new();
        for value in values {
            headers.append(IF_NONE_MATCH, HeaderValue::from_str(value).unwrap());
        }
        none_match_names(&headers, etag)
    }

    #[test]
    fn if_none_match_names_the_etag_as_itself_weak_in_a_list_or_as_a_star() {
        let etag = "\"abc\"";
        for values in [
            vec!["\"abc\""],
            vec!["W/\"abc\""],
            vec!["\"other\", \"abc\""],
            vec!["\"other\",W/\"abc\""],
            vec!["\"other\"", "\"abc\""],
            vec!["*"],
        ] {
            assert!(names(&values, etag), "{values:?}");
        }
        for values in [
            vec![],
            vec!["\"other\""],
            vec!["abc"],
            vec!["\"ABC\""],
            vec!["\"abc"],
            vec![""],
        ] {
            assert!(!names(&values, etag), "{values:?}");
        }
    }

    #[test]
    fn the_etag_is_the_quoted_hash_of_the_body_and_follows_it() {
        let etag = |body: &[u8]| {
            revalidated_json(&HeaderMap::new(), body.to_vec()).headers()[ETAG]
                .to_str()
                .unwrap()
                .to_string()
        };
        assert_eq!(etag(b"{}"), format!("\"{}\"", blake3::hash(b"{}").to_hex()));
        assert_eq!(etag(b"{}"), etag(b"{}"));
        assert_ne!(etag(b"{}"), etag(br#"{"a":{}}"#));
    }
}
