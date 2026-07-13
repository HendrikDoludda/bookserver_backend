use argon2::password_hash::{PasswordHasher, SaltString};
use argon2::Argon2;

use crate::database_related_scripts::db::Database;
use crate::error_types::AuthenticationError;
use crate::{
    data_models::authentication_model::UserCreationRequest,
    models::{DatabaseEntry, UserDatabaseColumns, UserMetadata},
};
use std::sync::Arc;
use std::time::SystemTime;

use rand::rngs::SysRng; // the OS CSPRNG (was `OsRng` before rand 0.10)
use rand::TryRng; // brings `try_fill_bytes` into scope for SysRng

//set up
pub async fn create_user(
    db: Arc<Database>,
    request: UserCreationRequest,
) -> Result<UserMetadata, AuthenticationError> {
    check_create_user_request_validity(db, request.clone())?;

    // Salt: 16 cryptographically-secure random bytes straight from the OS CSPRNG.
    // We can't use `SaltString::generate(rng)` here — it wants a rand_core 0.6 RNG
    // (via password-hash), but our `rand` 0.10 speaks rand_core 0.10, so we fill the
    // bytes ourselves and encode them instead.
    let mut salt_bytes = [0u8; 16]; // Salt::RECOMMENDED_LENGTH
    SysRng
        .try_fill_bytes(&mut salt_bytes)
        .map_err(|_| AuthenticationError::PasswordHashingFailure)?;
    let salt = SaltString::encode_b64(&salt_bytes)
        .map_err(|_| AuthenticationError::PasswordHashingFailure)?;

    // Hash the password with Argon2 using that salt. The returned PHC string already
    // embeds the salt + parameters, so this single string is what gets stored.
    let hash = Argon2::default()
        .hash_password(request.password.as_bytes(), &salt)
        .map_err(|_| AuthenticationError::PasswordHashingFailure)?
        .to_string();
    let user = UserMetadata {
        username: request.username,
        password_hash: hash,
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

pub fn verify_email() {
    //when user clicks on link from email it should route them to this call
    //here we will check if the link is valid.
    //the email verification table contains the expiration time as well as the code
    //if the token in inactive also fail the verification process
}

pub fn resend_verify_email() {
    //if called re-sends a verification email with a new token
}

fn send_verification_email() {
    //set previous token for user to inactive
    //create link
    //create email body
    //use the email_helper script to send an email
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

fn create_new_session() {
    //generate session token
    //generate refresh token
    //store tokens
    //return tokens
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

pub fn create_session_token() {}

pub fn create_refresh_token() {}

/*One additional recommendation for your stack:
Passwords: argon2
Random values (refresh tokens, salts, etc.): rand (using the OS RNG)
General hashing: blake3
UUIDs: uuid
JWTs: jsonwebtoken or another well-maintained JWT crate


*/
