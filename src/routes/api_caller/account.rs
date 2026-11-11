//! The account handlers: changes users make to their own account.

use std::{sync::Arc, time::SystemTime};

use axum::{extract::State, http::StatusCode};

use crate::{
    data_models::authentication_model::{
        ChangeEmailRequest, ChangePasswordRequest, ChangeUserNameRequest, ConfirmEmailChangeRequest,
    },
    db::Database,
    error_types::AppErrors,
    routes::{
        account_management::{self, EmailChangeRequested},
        email_helper::Mailer,
    },
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

/// `PUT /api/v1/account/username`
pub async fn change_username(
    session: AuthSession,
    State(db): State<Arc<Database>>,
    JsonBody(request): JsonBody<ChangeUserNameRequest>,
) -> Result<StatusCode, AppErrors> {
    account_management::change_username(db, session.0.user_id, request).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// `PUT /api/v1/account/email`: 202 while a code goes to the new address in the background, or
/// 204 when email is off and the address changed at once.
pub async fn change_email(
    session: AuthSession,
    State(db): State<Arc<Database>>,
    State(mailer): State<Mailer>,
    JsonBody(request): JsonBody<ChangeEmailRequest>,
) -> Result<StatusCode, AppErrors> {
    let user_id = session.0.user_id;
    let now = SystemTime::now();
    let requested =
        account_management::request_email_change(Arc::clone(&db), &mailer, user_id, request, now)
            .await?;
    match requested {
        EmailChangeRequested::Applied => Ok(StatusCode::NO_CONTENT),
        EmailChangeRequested::AwaitsCode { new_email } => {
            tokio::spawn(account_management::send_email_change_code(
                db, mailer, user_id, new_email, now,
            ));
            Ok(StatusCode::ACCEPTED)
        }
    }
}

/// `POST /api/v1/account/email/confirm`
pub async fn confirm_email_change(
    session: AuthSession,
    State(db): State<Arc<Database>>,
    JsonBody(request): JsonBody<ConfirmEmailChangeRequest>,
) -> Result<StatusCode, AppErrors> {
    account_management::confirm_email_change(db, session.0.user_id, request, SystemTime::now())
        .await?;
    Ok(StatusCode::NO_CONTENT)
}
