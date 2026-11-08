//! Adding one of these as a handler argument makes Axum check the request's
//! `Authorization: Bearer` token before the handler body runs. Every failure — no header,
//! another scheme, an unknown or expired token — gets the same 401; a non-admin gets 403.

use std::{sync::Arc, time::SystemTime};

use axum::{
    extract::{FromRef, FromRequestParts},
    http::{header::AUTHORIZATION, request::Parts, HeaderMap},
};

use crate::{
    database_related_scripts::db_login_challenges,
    db::Database,
    error_types::{
        AppErrors,
        AuthenticationError::{ExpiredSession, Forbidden, UnauthorizedSession},
    },
    models::{LoginChallenge, QuerySeparator, Sessions, SessionsDatabaseColumns, UserMetadata},
    routes::auth::hash_token,
};

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
        if !user.is_admin {
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
