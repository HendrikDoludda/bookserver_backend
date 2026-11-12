//! The request extractors. The session ones make Axum check the request's
//! `Authorization: Bearer` token before the handler body runs: every failure — no header,
//! another scheme, an unknown or expired token — gets the same 401; a non-admin gets 403.
//! `JsonBody`, `PathParams` and `QueryParams` read the body, the path and the query string,
//! and answer 400 `invalid_request` for what they can't read.

use std::{sync::Arc, time::SystemTime};

use axum::{
    extract::{FromRef, FromRequest, FromRequestParts, Path, Query, Request},
    http::{header::AUTHORIZATION, request::Parts, HeaderMap},
    Json,
};
use serde::de::DeserializeOwned;

use crate::{
    database_related_scripts::db_login_challenges,
    db::Database,
    error_types::{
        AppErrors,
        AuthenticationError::{ExpiredSession, Forbidden, UnauthorizedSession},
    },
    models::{
        LoginChallenge, QuerySeparator, Role, Sessions, SessionsDatabaseColumns, UserMetadata,
    },
    routes::auth::hash_token,
};

/// A JSON request body: axum's `Json`, but malformed JSON, a missing or mistyped field and a
/// missing `Content-Type` all answer 400 `invalid_request` as JSON.
pub struct JsonBody<T>(pub T);

impl<T, S> FromRequest<S> for JsonBody<T>
where
    T: DeserializeOwned,
    S: Send + Sync,
{
    type Rejection = AppErrors;

    async fn from_request(request: Request, state: &S) -> Result<Self, Self::Rejection> {
        // Not logged: the rejection's text can quote the sent value, such as a code.
        let Json(body) = Json::<T>::from_request(request, state)
            .await
            .map_err(|_| AppErrors::InvalidRequest)?;
        Ok(JsonBody(body))
    }
}

/// The path parameters: axum's `Path`, but a value that doesn't parse (`/books/abc`) answers
/// 400 `invalid_request` as JSON.
pub struct PathParams<T>(pub T);

impl<T, S> FromRequestParts<S> for PathParams<T>
where
    T: DeserializeOwned + Send,
    S: Send + Sync,
{
    type Rejection = AppErrors;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let Path(params) = Path::<T>::from_request_parts(parts, state)
            .await
            .map_err(|_| AppErrors::InvalidRequest)?;
        Ok(PathParams(params))
    }
}

/// The query string: axum's `Query`, but one it can't read — such as a field sent twice —
/// answers 400 `invalid_request` as JSON.
pub struct QueryParams<T>(pub T);

impl<T, S> FromRequestParts<S> for QueryParams<T>
where
    T: DeserializeOwned,
    S: Send + Sync,
{
    type Rejection = AppErrors;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let Query(params) = Query::<T>::from_request_parts(parts, state)
            .await
            .map_err(|_| AppErrors::InvalidRequest)?;
        Ok(QueryParams(params))
    }
}

/// The token of an `Authorization: Bearer <token>` header; the scheme is case-insensitive.
pub(crate) fn bearer_token(headers: &HeaderMap) -> Option<&str> {
    let value = headers.get(AUTHORIZATION)?.to_str().ok()?;
    let (scheme, token) = value.split_once(' ')?;
    let token = token.trim();
    (scheme.eq_ignore_ascii_case("bearer") && !token.is_empty()).then_some(token)
}

#[derive(Clone, Copy)]
enum TokenKind {
    Session,
    Refresh,
}

/// The one lookup behind `AuthSession` and `AuthRefreshSession`.
async fn find_session(
    parts: &Parts,
    db: Arc<Database>,
    kind: TokenKind,
) -> Result<Sessions, AppErrors> {
    let token = bearer_token(&parts.headers).ok_or(UnauthorizedSession)?;
    let hash = hash_token(token);
    let column = match kind {
        TokenKind::Session => SessionsDatabaseColumns::SessionToken,
        TokenKind::Refresh => SessionsDatabaseColumns::RefreshToken,
    };
    let session = db
        .read(move |conn| {
            Ok(Database::search_for_single_row_with_connection::<Sessions>(
                conn,
                &[column],
                &[&hash],
                QuerySeparator::And,
            )?)
        })
        .await?
        .ok_or(UnauthorizedSession)?;
    let expires_at = match kind {
        TokenKind::Session => session.session_token_expiration_date,
        TokenKind::Refresh => session.refresh_token_expiration_date,
    };
    if expires_at <= SystemTime::now() {
        return Err(ExpiredSession.into());
    }
    Ok(session)
}

/// A valid session, found by its session token.
pub struct AuthSession(pub Sessions);

impl<S> FromRequestParts<S> for AuthSession
where
    S: Send + Sync,
    Arc<Database>: FromRef<S>,
{
    type Rejection = AppErrors;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let db = Arc::<Database>::from_ref(state);
        Ok(AuthSession(
            find_session(parts, db, TokenKind::Session).await?,
        ))
    }
}

/// A valid session whose user is an admin.
pub struct AdminSession(pub Sessions);

impl<S> FromRequestParts<S> for AdminSession
where
    S: Send + Sync,
    Arc<Database>: FromRef<S>,
{
    type Rejection = AppErrors;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let AuthSession(session) = AuthSession::from_request_parts(parts, state).await?;
        let user_id = session.user_id;
        let user = Arc::<Database>::from_ref(state)
            .read(move |conn| {
                Ok(Database::get_entry_with_connection::<UserMetadata>(
                    conn, user_id,
                )?)
            })
            .await?;
        if user.role != Role::Admin {
            return Err(Forbidden.into());
        }
        Ok(AdminSession(session))
    }
}

/// A session found by its refresh token, for rotating the token pair.
pub struct AuthRefreshSession(pub Sessions);

impl<S> FromRequestParts<S> for AuthRefreshSession
where
    S: Send + Sync,
    Arc<Database>: FromRef<S>,
{
    type Rejection = AppErrors;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let db = Arc::<Database>::from_ref(state);
        Ok(AuthRefreshSession(
            find_session(parts, db, TokenKind::Refresh).await?,
        ))
    }
}

/// A login waiting for its TOTP code, found by its challenge token.
pub struct AuthTOTPLoginSession(pub LoginChallenge);

impl<S> FromRequestParts<S> for AuthTOTPLoginSession
where
    S: Send + Sync,
    Arc<Database>: FromRef<S>,
{
    type Rejection = AppErrors;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let token = bearer_token(&parts.headers).ok_or(UnauthorizedSession)?;
        let hash = hash_token(token);
        let now = SystemTime::now();
        let challenge = Arc::<Database>::from_ref(state)
            .read(move |conn| Ok(db_login_challenges::find_live(conn, &hash, now)?))
            .await?
            .ok_or(UnauthorizedSession)?;
        Ok(AuthTOTPLoginSession(challenge))
    }
}
