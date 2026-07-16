use argon2::password_hash::{PasswordHasher, SaltString};
use argon2::Argon2;

use crate::database_related_scripts::db::Database;
use crate::error_types::{AuthenticationError, EmailErrors};
use crate::models::{DatabaseTypes, EmailVerification, Sessions};
use crate::routes::email_helper::{create_new_email, EmailInformation};
use crate::{
    data_models::authentication_model::UserCreationRequest,
    models::{UserDatabaseColumns, UserMetadata},
};
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use rand::rngs::SysRng; // the OS CSPRNG (was `OsRng` before rand 0.10)
use rand::{RngExt, TryRng}; // brings `try_fill_bytes` into scope for SysRng

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
    //if not create user
    //send success message
    //show verify using email
    //send verification email
    //if yes send already has an account. or username already taken
}

fn check_create_user_request_validity(
    db: Arc<Database>,
    request: UserCreationRequest,
) -> Result<(), AuthenticationError> {
    let email_search_result = db
        .search_for_single_row::<UserMetadata>(&[UserDatabaseColumns::Email], &[&request.email])
        .map_err(|_| AuthenticationError::DatabaseSearchFailure)?;
    if (email_search_result.is_none()) {
        return Err(AuthenticationError::EmailTaken);
    }
    let username_search_result = db
        .search_for_single_row::<UserMetadata>(
            &[UserDatabaseColumns::Username],
            &[&request.username],
        )
        .map_err(|_| AuthenticationError::DatabaseSearchFailure)?;
    if (username_search_result.is_none()) {
        return Err(AuthenticationError::UsernameTaken);
    }
    if (request.password != request.password_repeat) {
        return Err(AuthenticationError::MismatchingPasswords);
    }

    Ok(())
}

pub async fn send_verification_email(
    user: UserMetadata,
    db: &Arc<Database>,
) -> Result<(), EmailErrors> {
    db.remove_entry(DatabaseTypes::EmailVerificationType, user.user_id);
    let mut rng = rand::rng();
    let code = rng.random_range(0..=999_999);
    let verification_code = format!("{:06}", code);
    let expiration_date = SystemTime::now()
        .checked_add(Duration::from_mins(15))
        .ok_or(EmailErrors::TimeAdjustmentFailure)?;

    let email_verification = EmailVerification {
        user_id: user.user_id,
        verification_token: blake3::hash(verification_code.as_bytes())
            .to_hex()
            .to_string(),
        expiration_date,
        invalidated: false,
    };
    db.insert(&email_verification).map_err(|e| {
        log::error!("Failed insertion into database: {e}");
        EmailErrors::DatabaseInsertionFailed
    })?;
    create_verification_email(verification_code.clone(), user)
        .await
        .map_err(|err| match err {
            EmailErrors::EmailSetUpNotFound => EmailErrors::SentFailedDueToNoConfig {
                code: verification_code,
            },
            err => err,
        })
    //set previous token for user to inactive
    //create link
    //create email body
    //use the email_helper script to send an email
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


*/
