//! Adding one of these as a handler argument makes Axum validate the request's
//! cookies before the handler body runs; a failed check rejects with 401/403.

use std::{sync::Arc, time::SystemTime};

use axum::extract::FromRequestParts;
use tower_cookies::Cookies;

use crate::{
    db::Database,
    error_types::{
        AppErrors,
        AuthenticationError::{ExpiredSession, UnauthorizedSession},
        RequestErrors,
    },
    models::{QuerySeparator, SelectionMethod, Sessions, SessionsDatabaseColumns, UserMetadata},
};

/// A fully authenticated session whose user is an admin.
pub struct AdminSession(pub Sessions);

impl FromRequestParts<Arc<Database>> for AdminSession {
    type Rejection = RequestErrors;

    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        db: &Arc<Database>,
    ) -> Result<Self, Self::Rejection> {
        let AuthSession(session) = AuthSession::from_request_parts(parts, db).await?;
        let user = db
            .get_entry::<UserMetadata>(session.user_id)
            .map_err(|_| RequestErrors::RequestFailed)?;
        if !user.is_admin {
            return Err(RequestErrors::Forbidden);
        }
        Ok(AdminSession(session))
    }
}

/// A valid, fully authenticated session, looked up by the `session_token` cookie.
pub struct AuthSession(pub Sessions);

impl FromRequestParts<Arc<Database>> for AuthSession {
    type Rejection = RequestErrors;

    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        db: &Arc<Database>,
    ) -> Result<Self, Self::Rejection> {
        let cookies = Cookies::from_request_parts(parts, db)
            .await
            .map_err(|_| RequestErrors::RequestFailed)?;
        let session = authorized(cookies, db)?;
        Ok(AuthSession(session))
    }
}

fn authorized(cookies: Cookies, db: &Arc<Database>) -> Result<Sessions, RequestErrors> {
    let session_token_cookie = cookies
        .get("session_token")
        .ok_or(RequestErrors::AuthorizationFailed)?;
    let hashed_session_token = blake3::hash(session_token_cookie.value().as_bytes())
        .to_hex()
        .to_string();
    let session: Sessions = db
        .setup_new_transaction(|conn| {
            let session = Database::search_for_single_row_with_connection::<Sessions>(
                conn,
                &[SessionsDatabaseColumns::SessionToken],
                &[&hashed_session_token],
                QuerySeparator::And,
                SelectionMethod::Everything,
            )?
            .ok_or(AppErrors::Authentication(UnauthorizedSession))?;
            if session.session_token_expiration_date < SystemTime::now() {
                return Err(AppErrors::Authentication(ExpiredSession));
            }
            if !session.authentication_completed {
                return Err(AppErrors::Authentication(UnauthorizedSession));
            }
            Ok(session)
        })
        .map_err(|err| match err {
            AppErrors::Authentication(_) => RequestErrors::AuthorizationFailed, // 401
            other => {
                log::error!("authorization transaction failed: {other}");
                RequestErrors::RequestFailed // 500
            }
        })?;
    Ok(session)
}

/// A session that passed the password step but is still waiting for its TOTP code.
pub struct AuthTOTPLoginSession(pub Sessions);

impl FromRequestParts<Arc<Database>> for AuthTOTPLoginSession {
    type Rejection = RequestErrors;
    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        db: &Arc<Database>,
    ) -> Result<Self, Self::Rejection> {
        let cookies = Cookies::from_request_parts(parts, db)
            .await
            .map_err(|_| RequestErrors::RequestFailed)?;
        let session = validate_totp_login_session(cookies, db)?;
        Ok(AuthTOTPLoginSession(session))
    }
}

fn validate_totp_login_session(
    cookies: Cookies,
    db: &Arc<Database>,
) -> Result<Sessions, RequestErrors> {
    let session_token_cookie = cookies
        .get("session_token")
        .ok_or(RequestErrors::AuthorizationFailed)?;
    let hashed_session_token = blake3::hash(session_token_cookie.value().as_bytes())
        .to_hex()
        .to_string();
    let session: Sessions = db
        .setup_new_transaction(|conn| {
            let session = Database::search_for_single_row_with_connection::<Sessions>(
                conn,
                &[SessionsDatabaseColumns::SessionToken],
                &[&hashed_session_token],
                QuerySeparator::And,
                SelectionMethod::Everything,
            )?
            .ok_or(AppErrors::Authentication(UnauthorizedSession))?;
            if session.session_token_expiration_date < SystemTime::now() {
                return Err(AppErrors::Authentication(ExpiredSession));
            }
            if session.authentication_completed {
                return Err(AppErrors::Authentication(UnauthorizedSession));
            }
            Ok(session)
        })
        .map_err(|err| match err {
            AppErrors::Authentication(_) => RequestErrors::AuthorizationFailed, // 401
            other => {
                log::error!("authorization transaction failed: {other}");
                RequestErrors::RequestFailed // 500
            }
        })?;
    Ok(session)
}

/// A session looked up by the `refresh_token` cookie, used to rotate the token pair.
pub struct AuthRefreshSession(pub Sessions);

impl FromRequestParts<Arc<Database>> for AuthRefreshSession {
    type Rejection = RequestErrors;
    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        db: &Arc<Database>,
    ) -> Result<Self, Self::Rejection> {
        let cookies = Cookies::from_request_parts(parts, db)
            .await
            .map_err(|_| RequestErrors::RequestFailed)?;
        let session = refresh_authorized(cookies, db)?;
        Ok(AuthRefreshSession(session))
    }
}

fn refresh_authorized(cookies: Cookies, db: &Arc<Database>) -> Result<Sessions, RequestErrors> {
    let refresh_token_cookie = cookies
        .get("refresh_token")
        .ok_or(RequestErrors::AuthorizationFailed)?;
    let hashed_refresh_token = blake3::hash(refresh_token_cookie.value().as_bytes())
        .to_hex()
        .to_string();
    let session: Sessions = db
        .setup_new_transaction(|conn| {
            let session = Database::search_for_single_row_with_connection::<Sessions>(
                conn,
                &[SessionsDatabaseColumns::RefreshToken],
                &[&hashed_refresh_token],
                QuerySeparator::And,
                SelectionMethod::Everything,
            )?
            .ok_or(AppErrors::Authentication(UnauthorizedSession))?;
            if session.refresh_token_expiration_date < SystemTime::now() {
                return Err(AppErrors::Authentication(ExpiredSession));
            }
            if !session.authentication_completed {
                return Err(AppErrors::Authentication(UnauthorizedSession));
            }
            Ok(session)
        })
        .map_err(|err| match err {
            AppErrors::Authentication(_) => RequestErrors::AuthorizationFailed, // 401
            other => {
                log::error!("authorization transaction failed: {other}");
                RequestErrors::RequestFailed // 500
            }
        })?;
    Ok(session)
}
