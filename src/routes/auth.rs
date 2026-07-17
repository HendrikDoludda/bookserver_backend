use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::Argon2;

use crate::data_models::authentication_model::VerifyEmailRequest;
use crate::database_related_scripts::db::Database;
use crate::error_types::{AuthenticationError, EmailErrors};
use crate::models::{DatabaseTypes, EmailVerification, EmailVerificationDatabaseColumns, Sessions};
use crate::routes::email_helper::{create_new_email, EmailInformation};
use crate::{
    data_models::authentication_model::UserCreationRequest,
    models::{UserDatabaseColumns, UserMetadata},
};
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use rand::distr::{Alphanumeric, SampleString};
use rand::rngs::SysRng; // the OS CSPRNG (was `OsRng` before rand 0.10)
use rand::TryRng; // brings `try_fill_bytes` into scope for SysRng

//set up
pub async fn create_user(
    db: &Arc<Database>,
    request: UserCreationRequest,
) -> Result<UserMetadata, AuthenticationError> {
    check_create_user_request_validity(db.clone(), request.clone())?;

    let hashed_password = hash_string_securely(request.password)?;
    let user = UserMetadata {
        user_id: 0,
        username: request.username,
        password_hash: hashed_password.hashed_string,
        email: Some(request.email),
        email_verified: false,
        is_admin: false, //needs to check for exisitng users and then if not make the first user an admin
        created_at: SystemTime::now(),
        last_login: SystemTime::now(),
    };

    Ok(user)
}

fn check_create_user_request_validity(
    db: Arc<Database>,
    request: UserCreationRequest,
) -> Result<(), AuthenticationError> {
    let email_search_result = db
        .search_for_single_row::<UserMetadata>(&[UserDatabaseColumns::Email], &[&request.email])
        .map_err(|_| AuthenticationError::DatabaseSearchFailure)?;
    if email_search_result.is_none() {
        return Err(AuthenticationError::EmailTaken);
    }
    let username_search_result = db
        .search_for_single_row::<UserMetadata>(
            &[UserDatabaseColumns::Username],
            &[&request.username],
        )
        .map_err(|_| AuthenticationError::DatabaseSearchFailure)?;
    if username_search_result.is_none() {
        return Err(AuthenticationError::UsernameTaken);
    }
    if request.password != request.password_repeat {
        return Err(AuthenticationError::MismatchingPasswords);
    }

    Ok(())
}

fn create_crypto_code(code_length: usize) -> String {
    Alphanumeric.sample_string(&mut rand::rng(), code_length)
}

pub async fn send_verification_email(
    user: UserMetadata,
    db: &Arc<Database>,
) -> Result<(), EmailErrors> {
    db.remove_entry(DatabaseTypes::EmailVerificationType, user.user_id);
    let crypto_code = create_crypto_code(8);
    let verification_code = hash_string_securely(crypto_code).map_err(|e| {
        log::error!("failed to hash code: {e}");
        EmailErrors::HashingFailed
    })?;

    let expiration_date = SystemTime::now()
        .checked_add(Duration::from_mins(15))
        .ok_or(EmailErrors::TimeAdjustmentFailure)?;

    let email_verification = EmailVerification {
        user_id: user.user_id,
        verification_token: verification_code.hashed_string,
        expiration_date,
        invalidated: false,
    };
    db.insert(&email_verification).map_err(|e| {
        log::error!("Failed insertion into database: {e}");
        EmailErrors::DatabaseInsertionFailed
    })?;
    create_verification_email(verification_code.normal_string.clone(), user)
        .await
        .map_err(|err| match err {
            EmailErrors::EmailSetUpNotFound => EmailErrors::SentFailedDueToNoConfig {
                code: verification_code.normal_string,
            },
            err => err,
        })
}

//TODO: Text must be replaced with the i18n text which I will create after the authentication
pub async fn create_verification_email(
    code: String,
    user: UserMetadata,
) -> Result<(), EmailErrors> {
    //if called re-sends a verification email with a new token
    let body = format!("Dear {}, \n The verify the account please enter the following code in the application.\n Code: {} \n Best regards,\n{}",user.username,code,"Server Team");
    let subject = format!("Verify Book Server Account");

    let mail_info = EmailInformation {
        username: user.username,
        email: user.email.ok_or(EmailErrors::MissingEmail)?,
        body,
        subject,
    };

    match create_new_email(mail_info).await {
        Ok(()) => Ok(()),
        Err(err) => return Err(err),
    }
}

pub async fn verify_verification_code(
    request: VerifyEmailRequest,
    db: &Arc<Database>,
) -> Result<(), AuthenticationError> {
    let stored_user = db
        .search_for_single_row::<UserMetadata>(&[UserDatabaseColumns::Email], &[&request.email])
        .map_err(|e| {
            log::error!("Database returned error: {e}");
            AuthenticationError::DatabaseSearchFailure
        })?
        .ok_or(AuthenticationError::UserNotFound)?;

    let verification_token = db
        .search_for_single_row::<EmailVerification>(
            &[
                EmailVerificationDatabaseColumns::UserId,
                EmailVerificationDatabaseColumns::Invalidated,
            ],
            &[&stored_user.user_id, &false],
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
        stored_user.user_id,
        request.email_verification_token.as_bytes(),
        hashed_token,
        db,
    )
}

fn evaluate_verification_token(
    user_id: i64,
    password: &[u8],
    stored_password: PasswordHash<'_>,
    db: &Arc<Database>,
) -> Result<(), AuthenticationError> {
    if Argon2::default()
        .verify_password(password, &stored_password)
        .is_ok()
    {
        db.setup_new_transaction(|conn| {
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
        })
        .map_err(|err| {
            log::error!(
                "Failed a transaction updating both the user and the email verification: {err}"
            );
            AuthenticationError::DatabaseUpdateFailed
        })?;
        Ok(())
    } else {
        return Err(AuthenticationError::IncorrectCredentials);
    }
}

//authentication
pub fn log_in() {
    //user submits a password and username
    //retrieve stored password from db for user
    //using argon2 you verify the password if it matches the stored one
    //has a built in verification function which should be used
    //if not verified give the option to send verification email and stop here
    //otherwise continue
    //if totp is enabled first start the totp log in process and then create a new session
    //if successful you return the refresh token and the session token.
}

pub fn refresh_session_token() {
    //if refresh token is valid continue
    //generate new session token
    //generate new refresh token
    //update previous tokens with new ones
    //return new tokens to user
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

pub fn change_password() {
    //requires the old password and the same new password twice
    //if old password passes the argon verification continue
    //verify new password
    //hash and store the new password for the user
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

pub fn create_new_session(
    db: Arc<Database>,
    user_id: i64,
) -> Result<CreatedTokensReadable, AuthenticationError> {
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
    let session = Sessions {
        session_id: 0, //will get assigned when entered into the db
        user_id,
        device_id: "".to_string(),
        device_name: "".to_string(),
        platform: "".to_string(),
        refresh_token: refresh_token.hashed_string,
        session_token: session_token.hashed_string,
        refresh_token_expiration_date,
        session_token_expiration_date,
        created_at: current_time,
        last_used_at: current_time,
        authentication_completed: false,
    };
    db.insert(&session)
        .map_err(|_| AuthenticationError::FailedDataStoring)?;
    Ok(CreatedTokensReadable {
        session_token: session_token.normal_string,
        refresh_token: refresh_token.normal_string,
        refresh_token_valid_until: refresh_token_expiration_date,
        session_token_valid_until: session_token_expiration_date,
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

struct HashedString {
    normal_string: String, //to send to front end
    hashed_string: String, //to store
}

struct CreatedTokensReadable {
    session_token: String,
    refresh_token: String,
    session_token_valid_until: SystemTime,
    refresh_token_valid_until: SystemTime,
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

Your flow would look something like this:
Validate username/email.
Create and insert the user.
Mark the user as verified = false.
Generate and store a verification token.
Try to send the email.
Success → return "check your email."
Email server not configured → auto-verify.
Other email error → tell the user the account was created but the verification email couldn't be sent.
User can log in at any time.
If verified == false, redirect them to the verification flow.
Allow resending the verification email with a cooldown.
Periodically delete accounts that have remained unverified for 30 days.
A few things I'd recommend as you continue:
Store when the last verification email was sent. That makes enforcing a cooldown straightforward and avoids generating unnecessary tokens.
Invalidate previous verification codes when issuing a new one (it looks like you're already removing the previous entry).
Don't reveal whether an email address exists on the resend endpoint. It's usually better to always respond with something like:
"If an account requiring verification exists, a new verification email has been sent."
This prevents someone from probing which email addresses are registered.
Let users request another verification email after logging in. Since they can already authenticate, it's convenient to have a "Resend verification email" button on the verification page.
*/
