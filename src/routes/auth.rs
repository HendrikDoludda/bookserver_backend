//! Authentication logic: account creation, email verification, login, password
//! changes/resets, and the session + refresh token pair. The handlers in
//! `api_caller/` call into these functions inside a database transaction.

use argon2::password_hash::{phc::PasswordHash, PasswordHasher, PasswordVerifier};
use argon2::Argon2;
use rusqlite::Connection;

use crate::data_models::authentication_model::{
    ChangePasswordRequest, DeviceInformation, VerifyEmailRequest,
};
use crate::database_related_scripts::{db::Database, db_login_challenges, db_sessions};
use crate::error_types::{AppErrors, AuthenticationError, DatabaseError, EmailErrors};
use crate::models::{
    DatabaseTypes, EmailVerification, EmailVerificationDatabaseColumns, QuerySeparator, Sessions,
    SessionsDatabaseColumns,
};
use crate::routes::email_helper::{EmailInformation, Mailer};
use crate::{
    data_models::authentication_model::UserCreationRequest,
    models::{UserDatabaseColumns, UserMetadata},
};
use std::sync::OnceLock;
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
    mailer: &Mailer,
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

    mailer.send(mail_info).await
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

pub const MIN_PASSWORD_CHARS: usize = 12;
pub const MAX_PASSWORD_CHARS: usize = 128;

/// The one check every new password passes: sign-up, password change and reset.
pub fn check_new_password(password: &str, repeat: &str) -> Result<(), AuthenticationError> {
    if password != repeat {
        return Err(AuthenticationError::MismatchingPasswords);
    }
    let length = password.chars().count();
    if length < MIN_PASSWORD_CHARS {
        return Err(AuthenticationError::PasswordTooShort);
    }
    if length > MAX_PASSWORD_CHARS {
        return Err(AuthenticationError::PasswordTooLong);
    }
    Ok(())
}

/// Argon2-hashes `secret` on Tokio's blocking pool: a hash takes tens of milliseconds,
/// which would stall an async worker thread.
pub async fn hash_secret(secret: String) -> Result<String, AuthenticationError> {
    tokio::task::spawn_blocking(move || {
        hash_string_securely(&secret).map(|hashed| hashed.hashed_string)
    })
    .await
    .map_err(|_| AuthenticationError::ErrorHashingData)?
}

/// Checks `secret` against an Argon2 hash on the blocking pool. Without a hash it checks
/// against a dummy one, so a missing account takes as long to reject as a wrong secret.
pub async fn secret_matches(
    secret: String,
    stored_hash: Option<String>,
) -> Result<bool, AuthenticationError> {
    tokio::task::spawn_blocking(move || {
        let stored_hash = match stored_hash {
            Some(hash) => hash,
            None => dummy_hash()?.to_string(),
        };
        let parsed = PasswordHash::new(&stored_hash).map_err(|err| {
            log::error!("a stored hash is not a valid PHC string: {err}");
            AuthenticationError::StringToPasswordHashConversionFailed
        })?;
        Ok(Argon2::default()
            .verify_password(secret.as_bytes(), &parsed)
            .is_ok())
    })
    .await
    .map_err(|_| AuthenticationError::ErrorHashingData)?
}

static DUMMY_HASH: OnceLock<String> = OnceLock::new();

/// A hash of a random secret, made with the default parameters so checking against it
/// costs exactly what a real check costs.
fn dummy_hash() -> Result<&'static str, AuthenticationError> {
    if let Some(hash) = DUMMY_HASH.get() {
        return Ok(hash);
    }
    let hash = hash_string_securely(&create_new_token()?.normal_string)?.hashed_string;
    Ok(DUMMY_HASH.get_or_init(|| hash))
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

fn verify_new_password() {
    //requires the same new password twice
    //if the new passwords are the same continue
    //if the new password is not the same as the old password continue
}

/// Ends every session of the user, and every login still waiting for its TOTP code —
/// e.g. after their password changed.
pub fn end_all_sessions(conn: &Connection, user_id: i64) -> Result<usize, DatabaseError> {
    db_login_challenges::delete_for_user(conn, user_id)?;
    Database::remove_entry_with_connection_based_on_columns::<Sessions>(
        conn,
        &[SessionsDatabaseColumns::UserId],
        &[&user_id],
        QuerySeparator::And,
    )
}

// === Sessions ================================================================

/// Starts a session on this device, replacing the session the user had there.
pub fn create_new_session(
    conn: &Connection,
    user_id: i64,
    device_information: DeviceInformation,
    now: SystemTime,
) -> Result<CreatedTokensReadable, AuthenticationError> {
    let tokens = create_token_pair(now)?;
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
        created_at: now,
        last_used_at: now,
    };
    db_sessions::delete_for_device(conn, user_id, device_information.device_id).map_err(|err| {
        log::error!("removing the device's earlier session failed: {err}");
        AuthenticationError::FailedDataStoring
    })?;
    Database::insert_with_connection(conn, &session).map_err(|err| {
        log::error!("storing a new session failed: {err}");
        AuthenticationError::FailedDataStoring
    })?;
    Ok(CreatedTokensReadable {
        session_token: tokens.session_token,
        refresh_token: tokens.refresh_token,
        refresh_token_valid_until: tokens.refresh_token_valid_until,
        session_token_valid_until: tokens.session_token_valid_until,
    })
}

pub fn create_token_pair(now: SystemTime) -> Result<NewSessionTokenData, AuthenticationError> {
    let session_token = create_new_token()?;
    let refresh_token = create_new_token()?;
    let hours_refresh_token_is_valid = 24 * 30; //24 for hours in a day and 30 for valid duration of refresh token,
    let refresh_token_expiration_date = now
        .checked_add(Duration::from_hours(hours_refresh_token_is_valid))
        .ok_or(AuthenticationError::TokenLifetimeCalculationFailed)?;
    let session_token_expiration_date = now
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
    let hashed_token = hash_token(&final_token);
    Ok(HashedString {
        normal_string: final_token,
        hashed_string: hashed_token,
    })
}

/// How a session, refresh or challenge token is stored and looked up. BLAKE3 without salt
/// is enough here, and is what makes lookup possible: the tokens are 256 random bits,
/// so there is nothing to brute-force.
pub fn hash_token(token: &str) -> String {
    blake3::hash(token.as_bytes()).to_hex().to_string()
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

    #[test]
    fn a_new_password_needs_12_to_128_characters_and_a_matching_repeat() {
        let check = |password: &str| check_new_password(password, password);

        assert!(matches!(
            check(&"a".repeat(11)),
            Err(AuthenticationError::PasswordTooShort)
        ));
        assert!(check(&"a".repeat(12)).is_ok());
        assert!(check(&"a".repeat(128)).is_ok());
        assert!(matches!(
            check(&"a".repeat(129)),
            Err(AuthenticationError::PasswordTooLong)
        ));
        // Characters, not bytes: twelve `ü` are 24 bytes but one valid 12-character password.
        assert!(check(&"ü".repeat(12)).is_ok());

        assert!(matches!(
            check_new_password("correct horse battery", "correct horse batterY"),
            Err(AuthenticationError::MismatchingPasswords)
        ));
    }

    #[tokio::test]
    async fn a_hashed_secret_matches_only_itself_and_the_dummy_matches_nothing() {
        let hash = hash_secret("correct horse battery".to_string())
            .await
            .unwrap();

        assert!(
            secret_matches("correct horse battery".to_string(), Some(hash.clone()))
                .await
                .unwrap()
        );
        assert!(
            !secret_matches("wrong horse battery".to_string(), Some(hash))
                .await
                .unwrap()
        );
        assert!(!secret_matches("correct horse battery".to_string(), None)
            .await
            .unwrap());
    }

    #[test]
    fn ending_all_sessions_leaves_other_users_alone() {
        let conn = memory_connection();
        let alice = crate::test_support::insert_user(&conn, "alice");
        let bob = crate::test_support::insert_user(&conn, "bob");
        for (user_id, token) in [(alice, "a1"), (alice, "a2"), (bob, "b1")] {
            conn.execute(
                "INSERT INTO sessions (user_id, device_id, device_name, platform, \
                 refresh_token_hashed, refresh_token_valid_until, session_token_hashed, \
                 session_token_valid_until, last_used_at) \
                 VALUES (?1, ?2, 'phone', 'ios', ?2, 0, ?2, 0, 0)",
                rusqlite::params![user_id, token],
            )
            .unwrap();
        }

        for (user_id, token) in [(alice, "challenge-a"), (bob, "challenge-b")] {
            conn.execute(
                "INSERT INTO login_challenges (user_id, token_hashed, expires_at, device_id, \
                 device_name, platform) VALUES (?1, ?2, 0, 'device', 'phone', 'ios')",
                rusqlite::params![user_id, token],
            )
            .unwrap();
        }

        assert_eq!(end_all_sessions(&conn, alice).unwrap(), 2);
        let count = |table: &str| -> i64 {
            conn.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                row.get(0)
            })
            .unwrap()
        };
        assert_eq!(count("sessions"), 1, "bob's session stays");
        assert_eq!(
            count("login_challenges"),
            1,
            "a pending TOTP login ends too"
        );
    }
}
