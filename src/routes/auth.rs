//! Authentication logic: account creation, email verification, login, password
//! changes/resets, and the session + refresh token pair. The handlers in
//! `api_caller/` call into these functions inside a database transaction.

use argon2::password_hash::{phc::PasswordHash, PasswordHasher, PasswordVerifier};
use argon2::Argon2;
use rusqlite::{Connection, ToSql};
use url::Url;

use crate::data_models::authentication_model::{
    ChangePasswordRequest, DeviceInformation, LoginRequest, VerifyEmailRequest,
};
use crate::database_related_scripts::db::Database;
use crate::db::convert_system_time_to_unix_time;
use crate::error_types::{AppErrors, AuthenticationError, EmailErrors};
use crate::models::{
    DatabaseTypes, EmailVerification, EmailVerificationDatabaseColumns, QuerySeparator, Sessions,
    SessionsDatabaseColumns, TOTPDatabaseColumns, TOTP,
};
use crate::routes::email_helper::{create_new_email, EmailInformation};
use crate::{
    data_models::authentication_model::UserCreationRequest,
    models::{UserDatabaseColumns, UserMetadata},
};
use std::time::{Duration, SystemTime};

use rand::distr::{Alphanumeric, SampleString};
use rand::rngs::SysRng; // the OS CSPRNG (was `OsRng` before rand 0.10)
use rand::TryRng; // brings `try_fill_bytes` into scope for SysRng

pub fn create_new_user(
    conn: &Connection,
    first_user: UserCreationChecks,
    request: UserCreationRequest,
) -> Result<i64, AuthenticationError> {
    let hashed_password = hash_string_securely(&request.password)?;
    let user = UserMetadata {
        user_id: 0,
        username: request.username,
        password_hash: hashed_password.hashed_string,
        email: request.email,
        email_verified: false,
        is_admin: first_user.make_admin,
        created_at: SystemTime::now(),
        last_login: None,
    };

    let id = Database::insert_with_connection(conn, &user).map_err(|err| {
        log::error!("failed to insert the new user into the database: {err}");
        AuthenticationError::DatabaseInsertFailed
    })?;
    Ok(id)
}

pub struct UserCreationChecks {
    pub make_admin: bool,
}

pub struct VerificationEmailInformation {
    pub code: String,
    pub user: UserMetadata,
}

pub struct HashedString {
    pub normal_string: String, // sent to the client
    pub hashed_string: String, // stored in the database
}

pub struct CreatedTokensReadable {
    pub session_token: String,
    pub refresh_token: String,
    pub session_token_valid_until: SystemTime,
    pub refresh_token_valid_until: SystemTime,
}

pub struct NewSessionTokenData {
    pub session_token: String,
    pub session_token_hashed: String,
    pub refresh_token: String,
    pub refresh_token_hashed: String,
    pub session_token_valid_until: SystemTime,
    pub refresh_token_valid_until: SystemTime,
}

pub struct UserTotpEnabledResult {
    pub user_has_totp_enabled: bool,
}

pub struct ResetAndCancelationTokens {
    pub reset_token: String,
    pub cancelation_token: String,
    pub reset_token_hashed: String,
    pub cancelation_token_hashed: String,
}

pub fn validate_new_user(
    conn: &Connection,
    request: UserCreationRequest,
) -> Result<UserCreationChecks, AuthenticationError> {
    if request.password != request.password_repeat {
        return Err(AuthenticationError::MismatchingPasswords);
    }
    let existing_user = Database::search_for_single_row_with_connection::<UserMetadata>(
        conn,
        &[UserDatabaseColumns::Email, UserDatabaseColumns::Username],
        &[&request.email, &request.username],
        QuerySeparator::Or,
    )
    .map_err(|_| AuthenticationError::DatabaseSearchFailure)?;
    // ASCII-only, like SQLite's NOCASE on both columns: the database found this row
    // case-insensitively, so the comparison here must agree with it.
    if let Some(user) = existing_user {
        if user.email.eq_ignore_ascii_case(&request.email) {
            return Err(AuthenticationError::EmailTaken);
        }
        if user.username.eq_ignore_ascii_case(&request.username) {
            return Err(AuthenticationError::UsernameTaken);
        }
    }
    let existing_admin = Database::search_for_single_row_with_connection::<UserMetadata>(
        conn,
        &[UserDatabaseColumns::IsAdmin],
        &[&true],
        QuerySeparator::And,
    )
    .map_err(|_| AuthenticationError::DatabaseSearchFailure)?;
    Ok(UserCreationChecks {
        make_admin: existing_admin.is_none(),
    })
}

fn create_crypto_code(code_length: usize) -> String {
    Alphanumeric.sample_string(&mut rand::rng(), code_length)
}

pub fn send_verification_email(
    conn: &Connection,
    user_id: i64,
) -> Result<VerificationEmailInformation, EmailErrors> {
    Database::remove_entry_with_connection(conn, DatabaseTypes::EmailVerificationType, user_id)
        .map_err(|_| EmailErrors::VerificationTokenRemovalFailure)?;
    let crypto_code = create_crypto_code(8);
    let verification_code = hash_string_securely(&crypto_code).map_err(|e| {
        log::error!("failed to hash code: {e}");
        EmailErrors::HashingFailed
    })?;
    let user = Database::search_for_single_row_with_connection::<UserMetadata>(
        conn,
        &[UserDatabaseColumns::UserId],
        &[&user_id],
        QuerySeparator::And,
    )
    .map_err(|_| EmailErrors::FailedToRetrieveUserEntry)?
    .ok_or(EmailErrors::UserEntryNotFound)?;

    let expiration_date = SystemTime::now()
        .checked_add(Duration::from_mins(15))
        .ok_or(EmailErrors::TimeAdjustmentFailure)?;

    let email_verification = EmailVerification {
        user_id,
        verification_token: verification_code.hashed_string,
        expiration_date,
        invalidated: false,
    };

    Database::insert_with_connection(conn, &email_verification).map_err(|e| {
        log::error!("Failed insertion into database: {e}");
        EmailErrors::DatabaseInsertionFailed
    })?;
    let response = VerificationEmailInformation {
        code: verification_code.normal_string.clone(),
        user,
    };
    Ok(response)
}

/// Sends the verification email; calling it again re-sends one with a new token.
// TODO: replace the text with i18n strings once the catalog exists.
pub async fn create_verification_email(
    information: &VerificationEmailInformation,
) -> Result<(), EmailErrors> {
    let body = format!(
        "Dear {},\nTo verify the account please enter the following code in the application.\nCode: {}\nBest regards,\n{}",
        information.user.username, information.code, "Server Team"
    );
    let subject = "Verify Book Server Account".to_string();

    let mail_info: EmailInformation = EmailInformation {
        username: information.user.username.clone(),
        email: information.user.email.clone(),
        body,
        subject,
    };

    create_new_email(mail_info).await
}

pub fn search_for_user_using_email_or_username(
    conn: &Connection,
    username_or_email: &String,
) -> Result<UserMetadata, AppErrors> {
    let stored_user = Database::search_for_single_row_with_connection::<UserMetadata>(
        conn,
        &[UserDatabaseColumns::Email, UserDatabaseColumns::Username],
        &[&username_or_email, &username_or_email],
        QuerySeparator::Or,
    )
    .map_err(|e| {
        log::error!("Database returned error: {e}");
        AuthenticationError::DatabaseSearchFailure
    })?
    .ok_or(AuthenticationError::IncorrectCredentials)?;
    Ok(stored_user)
}

pub fn verify_verification_code(
    conn: &Connection,
    request: &VerifyEmailRequest,
    user_id: i64,
) -> Result<(), AppErrors> {
    let verification_token = Database::search_for_single_row_with_connection::<EmailVerification>(
        conn,
        &[
            EmailVerificationDatabaseColumns::UserId,
            EmailVerificationDatabaseColumns::Invalidated,
        ],
        &[&user_id, &false],
        QuerySeparator::And,
    )
    .map_err(|e| {
        log::error!("Database returned error: {e}");
        AuthenticationError::DatabaseSearchFailure
    })?
    .ok_or(AuthenticationError::EmailVerificationNotFound)?;

    if verification_token.expiration_date < SystemTime::now() {
        return Err(AppErrors::Authentication(
            AuthenticationError::ExpiredAuthenticationCode,
        ));
    }

    let hashed_token = PasswordHash::new(&verification_token.verification_token).map_err(|e| {
        log::error!("PasswordHash::new failed to convert string to PasswordHash: {e}");
        AuthenticationError::StringToPasswordHashConversionFailed
    })?;
    evaluate_verification_token(
        conn,
        user_id,
        request.email_verification_token.as_bytes(),
        hashed_token,
    )
}

fn evaluate_verification_token(
    conn: &Connection,
    user_id: i64,
    password: &[u8],
    stored_password: PasswordHash,
) -> Result<(), AppErrors> {
    if Argon2::default()
        .verify_password(password, &stored_password)
        .is_ok()
    {
        Database::update_value_with_connection::<UserMetadata>(
            conn,
            UserDatabaseColumns::EmailVerified,
            &true,
            user_id,
        )?;

        Database::update_value_with_connection::<EmailVerification>(
            conn,
            EmailVerificationDatabaseColumns::Invalidated,
            &true,
            user_id,
        )?;

        Ok(())
    } else {
        Err(AppErrors::Authentication(
            AuthenticationError::IncorrectCredentials,
        ))
    }
}

// === Login ===================================================================

pub fn log_in(
    request: &LoginRequest,
    conn: &Connection,
    user: &UserMetadata,
) -> Result<UserTotpEnabledResult, AuthenticationError> {
    validate_submitted_password(user, &request.password)?;
    let user_totp_settings = Database::search_for_single_row_with_connection::<TOTP>(
        conn,
        &[TOTPDatabaseColumns::UserId],
        &[&user.user_id],
        QuerySeparator::And,
    )
    .map_err(|err| {
        log::error!("search request failed: {err}");
        AuthenticationError::DatabaseSearchFailure
    })?;
    Ok(UserTotpEnabledResult {
        user_has_totp_enabled: user_totp_settings.is_some(),
    })
}

pub fn validate_submitted_password(
    user: &UserMetadata,
    password: &String,
) -> Result<(), AuthenticationError> {
    let password_hash = PasswordHash::new(&user.password_hash).map_err(|err| {
        log::error!("Failed to hash password: {err}");
        AuthenticationError::StringToPasswordHashConversionFailed
    })?;
    if Argon2::default()
        .verify_password(password.as_bytes(), &password_hash)
        .is_err()
    {
        return Err(AuthenticationError::IncorrectCredentials);
    }
    Ok(())
}

// === Password changes & resets ===============================================

pub fn generate_password_reset_codes() -> Result<ResetAndCancelationTokens, AuthenticationError> {
    //create email body for resetting email
    //create a reset token
    //create link that redirects to reset password
    //attach to email body
    //send to user
    //if email is valid or invalid always display the same message
    //"If email is valid an email has been sent"
    let reset_token = create_crypto_code(8);
    let cancelation_token = create_crypto_code(32);

    let reset_token_hashed = hash_string_securely(&reset_token)
        .map_err(|_| AuthenticationError::ErrorHashingData)?
        .hashed_string;
    let cancelation_token_hashed = hash_string_securely(&cancelation_token)
        .map_err(|_| AuthenticationError::ErrorHashingData)?
        .hashed_string;

    Ok(ResetAndCancelationTokens {
        reset_token,
        cancelation_token,
        reset_token_hashed,
        cancelation_token_hashed,
    })
}

pub async fn send_reset_password_email(
    reset_token: &str,
    cancelation_token: &str,
    server_address: &Url,
    email: &str,
) -> Result<(), EmailErrors> {
    //must look into getting the server address even if it is just storing the value somewhere
    let mut cancelation_link = server_address
        .join("/password-reset/cancel")
        .map_err(|_| EmailErrors::MissingEmail)?;
    cancelation_link
        .query_pairs_mut()
        .append_pair("token", cancelation_token);
    let body = format!("Hello,
    \n You have requested to reset your password. Use this code in the app to apply the new password.
    \n {}
    \nIn case you did not request the password reset use this link to invalidate the request: {}",reset_token,cancelation_link);
    let subject = "Password Reset".to_string();

    let email_info = EmailInformation {
        username: "Anonymous".to_owned(),
        email: email.to_owned(),
        body,
        subject,
    };

    create_new_email(email_info).await
}

pub fn update_password_to_new_password(
    conn: &Connection,
    request: ChangePasswordRequest,
    user_id: i64,
) -> Result<(), AuthenticationError> {
    //requires the old password and the same new password twice
    //if old password passes the argon verification continue
    //verify new password
    //hash and store the new password for the user
    let password_hash = hash_string_securely(&request.new_password)?;
    Database::update_value_with_connection::<UserMetadata>(
        conn,
        UserDatabaseColumns::PasswordHash,
        &password_hash.hashed_string,
        user_id,
    )
    .map_err(|_| AuthenticationError::DatabaseUpdateFailed)?;
    Ok(())
}

pub fn reset_password() {
    //requires the same new password twice and verify new password
    //if the token is valid continue
    //remove reset token
    //store new password for the user.
}

fn verify_new_password() {
    //requires the same new password twice
    //if the new passwords are the same continue
    //if the new password is not the same as the old password continue
}

fn password_changed() {
    //remove all active sessions and refresh tokens.
}

// === Sessions ================================================================

pub fn log_out() {
    //delete the session and refresh token entry to inactive
    //return success
}

pub fn update_existing_session(
    conn: &Connection,
    session: Sessions,
) -> Result<CreatedTokensReadable, AuthenticationError> {
    let tokens = create_token_pair()?;
    let columns = [
        SessionsDatabaseColumns::RefreshToken,
        SessionsDatabaseColumns::SessionToken,
        SessionsDatabaseColumns::RefreshTokenExpirationDate,
        SessionsDatabaseColumns::SessionTokenExpirationDate,
        SessionsDatabaseColumns::LastUsedAt,
    ];
    let refresh_expiration = convert_system_time_to_unix_time(tokens.refresh_token_valid_until);
    let session_expiration = convert_system_time_to_unix_time(tokens.session_token_valid_until);
    let now = convert_system_time_to_unix_time(SystemTime::now());
    let values: &[&dyn ToSql] = &[
        &tokens.refresh_token_hashed,
        &tokens.session_token_hashed,
        &refresh_expiration,
        &session_expiration,
        &now,
    ];
    // For more safety, turn this update into a search-and-update that confirms the
    // stored token is still the one that was presented. That prevents a race where two
    // concurrent refreshes both overwrite the token. Not needed at this scale, but it
    // could matter later.
    Database::update_multiple_values_with_connection::<Sessions>(
        conn,
        &columns,
        values,
        session.session_id,
    )
    .map_err(|err| {
        log::error!("Failed the updating of the refresh token: {err}");
        AuthenticationError::DatabaseUpdateFailed
    })?;
    Ok(CreatedTokensReadable {
        session_token: tokens.session_token,
        refresh_token: tokens.refresh_token,
        refresh_token_valid_until: tokens.refresh_token_valid_until,
        session_token_valid_until: tokens.session_token_valid_until,
    })
}

pub fn create_new_session(
    conn: &Connection,
    user_id: i64,
    device_information: DeviceInformation,
    user_totp_settings: UserTotpEnabledResult,
) -> Result<CreatedTokensReadable, AuthenticationError> {
    let tokens = create_token_pair()?;
    let authentication_complete = !user_totp_settings.user_has_totp_enabled;
    let session = Sessions {
        session_id: 0, //will get assigned when entered into the db
        user_id,
        device_id: device_information.device_id.to_string(),
        device_name: device_information.device_name.to_string(),
        platform: device_information.platform.to_string(),
        refresh_token: tokens.refresh_token_hashed,
        session_token: tokens.session_token_hashed,
        refresh_token_expiration_date: tokens.refresh_token_valid_until,
        session_token_expiration_date: tokens.session_token_valid_until,
        created_at: SystemTime::now(),
        last_used_at: SystemTime::now(),
        authentication_completed: authentication_complete, //will be changed on log in when the user has totp authentication is enabled.
    };
    Database::insert_with_connection(conn, &session)
        .map_err(|_| AuthenticationError::FailedDataStoring)?;
    Ok(CreatedTokensReadable {
        session_token: tokens.session_token,
        refresh_token: tokens.refresh_token,
        refresh_token_valid_until: tokens.refresh_token_valid_until,
        session_token_valid_until: tokens.session_token_valid_until,
    })
}

fn create_token_pair() -> Result<NewSessionTokenData, AuthenticationError> {
    let session_token = create_new_token()?;
    let refresh_token = create_new_token()?;
    let hours_refresh_token_is_valid = 24 * 30; //24 for hours in a day and 30 for valid duration of refresh token,
    let current_time = SystemTime::now();
    let refresh_token_expiration_date = current_time
        .checked_add(Duration::from_hours(hours_refresh_token_is_valid))
        .ok_or(AuthenticationError::TokenLifetimeCalculationFailed)?;
    let session_token_expiration_date = current_time
        .checked_add(Duration::from_mins(15))
        .ok_or(AuthenticationError::TokenLifetimeCalculationFailed)?;
    Ok(NewSessionTokenData {
        session_token: session_token.normal_string,
        session_token_hashed: session_token.hashed_string,
        refresh_token: refresh_token.normal_string,
        refresh_token_hashed: refresh_token.hashed_string,
        session_token_valid_until: session_token_expiration_date,
        refresh_token_valid_until: refresh_token_expiration_date,
    })
}

pub fn create_new_token() -> Result<HashedString, AuthenticationError> {
    let mut token = [0u8; 32];
    SysRng
        .try_fill_bytes(&mut token)
        .map_err(|_| AuthenticationError::RngCreatorFailed)?;
    let final_token = hex::encode(token);
    let hashed_token = blake3::hash(final_token.as_bytes()).to_hex().to_string();
    Ok(HashedString {
        normal_string: final_token,
        hashed_string: hashed_token,
    })
}

fn hash_string_securely(to_hash: &str) -> Result<HashedString, AuthenticationError> {
    let hasher = Argon2::default();
    let hash = hasher
        .hash_password(to_hash.as_bytes())
        .map_err(|_| AuthenticationError::ErrorHashingData)?;
    Ok(HashedString {
        normal_string: to_hash.to_owned(),
        hashed_string: hash.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::memory_connection;

    fn sign_up(username: &str, email: &str) -> UserCreationRequest {
        UserCreationRequest {
            email: email.to_string(),
            username: username.to_string(),
            password: "correct horse battery".to_string(),
            password_repeat: "correct horse battery".to_string(),
        }
    }

    fn with_alice() -> Connection {
        let conn = memory_connection();
        conn.execute(
            "INSERT INTO users (username, password_hash, email) \
             VALUES ('Alice', 'hash', 'Alice@Example.com')",
            [],
        )
        .unwrap();
        conn
    }

    #[test]
    fn an_email_differing_only_in_case_is_taken() {
        let conn = with_alice();
        let result = validate_new_user(&conn, sign_up("bob", "alice@example.com"));
        assert!(matches!(result, Err(AuthenticationError::EmailTaken)));
    }

    #[test]
    fn a_username_differing_only_in_case_is_taken() {
        let conn = with_alice();
        let result = validate_new_user(&conn, sign_up("ALICE", "other@example.com"));
        assert!(matches!(result, Err(AuthenticationError::UsernameTaken)));
    }

    #[test]
    fn a_new_username_and_email_pass() {
        let conn = with_alice();
        let result = validate_new_user(&conn, sign_up("bob", "bob@example.com"));
        assert!(result.is_ok());
    }
}
