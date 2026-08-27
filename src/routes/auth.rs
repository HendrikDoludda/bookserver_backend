use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::Argon2;
use mail_send::mail_auth::Dkim2Result::Pass;
use rusqlite::{Connection, ToSql};
use serde_json::error::Category::Data;

use crate::data_models::authentication_model::{
    ChangePasswordRequest, DeviceInformation, LoginRequest, VerifyEmailRequest,
};
use crate::database_related_scripts::db::Database;
use crate::db::convert_system_time_to_unix_time;
use crate::error_types::{AppErrors, AuthenticationError, DatabaseError, EmailErrors};
use crate::models::{
    DatabaseTypes, EmailVerification, EmailVerificationDatabaseColumns, QuerySeparator,
    SelectionMethod, Sessions, SessionsDatabaseColumns, TOTPDatabaseColumns, TOTP,
};
use crate::routes::email_helper::{create_new_email, EmailInformation};
use crate::{
    data_models::authentication_model::UserCreationRequest,
    models::{UserDatabaseColumns, UserMetadata},
};
use std::alloc::System;
use std::default;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use rand::distr::{Alphanumeric, SampleString};
use rand::rngs::SysRng; // the OS CSPRNG (was `OsRng` before rand 0.10)
use rand::TryRng; // brings `try_fill_bytes` into scope for SysRng

pub fn create_new_user(
    conn: &Connection,
    first_user: UserCreationChecks,
    request: UserCreationRequest,
) -> Result<i64, AuthenticationError> {
    let hashed_password = hash_string_securely(request.password)?;
    let user = UserMetadata {
        user_id: 0,
        username: request.username,
        password_hash: hashed_password.hashed_string,
        email: Some(request.email),
        email_verified: false,
        is_admin: first_user.make_admin,
        created_at: SystemTime::now(),
        last_login: SystemTime::now(),
    };

    let id = Database::insert_with_connection(conn, &user).map_err(|err| {
        log::error!("failed to insert the new user into the database: {err}");
        AuthenticationError::DatabaseInsertFailed
    })?;
    Ok(id)
}

struct UserCreationChecks {
    pub make_admin: bool,
}
struct VerificationEmailInformation {
    pub code: String,
    pub user: UserMetadata,
}
pub struct HashedString {
    pub normal_string: String, //to send to front end
    pub hashed_string: String, //to store
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
        crate::models::SelectionMethod::PassedInColumns,
    )
    .map_err(|_| AuthenticationError::DatabaseSearchFailure)?;
    if let Some(user) = existing_user {
        if user.email == Some(request.email) {
            return Err(AuthenticationError::EmailTaken);
        }
        if user.username == request.username {
            return Err(AuthenticationError::UsernameTaken);
        }
    }
    let existing_admin = Database::search_for_single_row_with_connection::<UserMetadata>(
        conn,
        &[UserDatabaseColumns::IsAdmin],
        &[&true],
        QuerySeparator::And,
        SelectionMethod::PassedInColumns,
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
    let verification_code = hash_string_securely(crypto_code).map_err(|e| {
        log::error!("failed to hash code: {e}");
        EmailErrors::HashingFailed
    })?;
    let user = Database::search_for_single_row_with_connection::<UserMetadata>(
        conn,
        &[UserDatabaseColumns::UserId],
        &[&user_id],
        QuerySeparator::And,
        SelectionMethod::Everything,
    )
    .map_err(|_| EmailErrors::FailedToRetrieveUserEntry)?
    .ok_or(EmailErrors::UserEntryNotFound)?;

    let expiration_date = SystemTime::now()
        .checked_add(Duration::from_mins(15))
        .ok_or(EmailErrors::TimeAdjustmentFailure)?;

    let email_verification = EmailVerification {
        user_id: user_id,
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
        user: user,
    };
    Ok(response)
}

//TODO: Text must be replaced with the i18n text which I will create after the authentication
pub async fn create_verification_email(
    information: &VerificationEmailInformation,
) -> Result<(), EmailErrors> {
    //if called re-sends a verification email with a new token
    let body = format!("Dear {}, \n The verify the account please enter the following code in the application.\n Code: {} \n Best regards,\n{}",&information.user.username,information.code,"Server Team");
    let subject = format!("Verify Book Server Account");

    let mail_info: EmailInformation = EmailInformation {
        username: information.user.username.clone(),
        email: information
            .user
            .email
            .clone()
            .ok_or(EmailErrors::MissingEmail)?,
        body,
        subject,
    };

    match create_new_email(mail_info).await {
        Ok(()) => Ok(()),
        Err(err) => return Err(err),
    }
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
        SelectionMethod::Everything,
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
        SelectionMethod::Everything,
    )
    .map_err(|e| {
        log::error!("Database returned error: {e}");
        AuthenticationError::DatabaseSearchFailure
    })?
    .ok_or(AuthenticationError::EmailVerificationNotFound)?;

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
    stored_password: PasswordHash<'_>,
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
        return Err(AppErrors::Authentication(
            AuthenticationError::IncorrectCredentials,
        ));
    }
}

//authentication
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
        SelectionMethod::Everything,
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

//modification
pub fn request_reset_password() {
    //create email body for resetting email
    //create a reset token
    //create link that redirects to reset password
    //attach to email body
    //send to user
    //if email is valid or invalid always display the same message
    //"If email is valid an email has been sent"
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
    let password_hash = hash_string_securely(request.new_password)?;
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

//sign out
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
    let refresh_expirtation = convert_system_time_to_unix_time(tokens.refresh_token_valid_until);
    let session_expiration = convert_system_time_to_unix_time(tokens.session_token_valid_until);
    let now = convert_system_time_to_unix_time(SystemTime::now());
    let values: &[&dyn ToSql] = &[
        &tokens.refresh_token,
        &tokens.session_token,
        &refresh_expirtation,
        &session_expiration,
        &now,
    ];
    //For more safety change update to search and update
    //This will allow for searching for a specific entry and confirming that the token is still the same as before
    //to prevent racing conditions where the entry will overwrite the token twice
    //For small applications not necessary but could be improtant later on
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

fn hash_string_securely(to_hash: String) -> Result<HashedString, AuthenticationError> {
    let mut salt_bytes = [0u8; 16];
    SysRng
        .try_fill_bytes(&mut salt_bytes)
        .map_err(|_| AuthenticationError::RngCreatorFailed)?;
    let salt =
        SaltString::encode_b64(&salt_bytes).map_err(|_| AuthenticationError::ErrorCreatingSalt)?;
    let hash = Argon2::default()
        .hash_password(to_hash.as_bytes(), &salt)
        .map_err(|_| AuthenticationError::ErrorHashingData)?;
    Ok(HashedString {
        normal_string: to_hash,
        hashed_string: hash.to_string(),
    })
}

/*One additional recommendation for your stack:
Passwords: argon2
Random values (refresh tokens, salts, etc.): rand (using the OS RNG)
General hashing: blake3
UUIDs: uuid
JWTs: jsonwebtoken or another well-maintained JWT crate


On first run:
Generate a 256-bit random key.
Store it in a protected secrets file.
Use that key for:
keyed BLAKE3 hashing of refresh tokens
encrypting sensitive configuration values
signing internal data (if appropriate)
The database then contains only encrypted or hashed values, while the secrets file contains the key needed to use them.
One recommendation
I would avoid inventing your own encryption format. Rust has excellent, well-reviewed libraries for authenticated encryption, such as those implementing AES-GCM or ChaCha20-Poly1305. These provide both confidentiality and integrity, reducing the chance of subtle security mistakes.
For your application, a clean architecture would be:
Database: application data, sessions, encrypted secrets.
Secrets file: one randomly generated 256-bit master key (and possibly a few deployment-specific secrets).
Application: loads the master key at startup and uses it for keyed hashing and encryption/decryption as needed.
That gives you a good separation of concerns without making deployment overly complicated.
*/
