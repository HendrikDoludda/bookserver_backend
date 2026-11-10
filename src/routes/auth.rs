//! Account, password and session building blocks shared by the auth flows (`login.rs`,
//! `registration.rs`, `password_reset.rs`): the checks a new username, email address or
//! password passes, hashing, and the session + refresh token pair.

use argon2::password_hash::{phc::PasswordHash, PasswordHasher, PasswordVerifier};
use argon2::Argon2;
use rusqlite::Connection;

use crate::data_models::authentication_model::{ChangePasswordRequest, DeviceInformation};
use crate::database_related_scripts::{db::Database, db_login_challenges};
use crate::error_types::{AuthenticationError, DatabaseError};
use crate::models::{
    DatabaseTypes, QuerySeparator, Sessions, SessionsDatabaseColumns, UserDatabaseColumns,
    UserMetadata,
};
use std::sync::OnceLock;
use std::time::{Duration, SystemTime};

use rand::rngs::SysRng; // the OS CSPRNG (was `OsRng` before rand 0.10)
use rand::TryRng; // brings `try_fill_bytes` into scope for SysRng

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

// === Usernames & email addresses =============================================

pub const MIN_USERNAME_CHARS: usize = 3;
pub const MAX_USERNAME_CHARS: usize = 32;
pub const MAX_EMAIL_CHARS: usize = 254;

/// The one check every new username passes: 3 to 32 of `A–Z a–z 0–9 _ . -`.
pub fn check_username(username: &str) -> Result<(), AuthenticationError> {
    // Never `@`, which makes login read the input as an email, nor `:`, which totp-rs
    // refuses in an account name.
    let allowed = |c: char| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-');
    let length_ok = (MIN_USERNAME_CHARS..=MAX_USERNAME_CHARS).contains(&username.len());
    if length_ok && username.chars().all(allowed) {
        Ok(())
    } else {
        Err(AuthenticationError::InvalidUsername)
    }
}

/// The one check every new email address passes: exactly one `@` with text on both sides,
/// printable ASCII only, at most 254 characters.
pub fn check_email(email: &str) -> Result<(), AuthenticationError> {
    // mail-send and mail-builder write the address unescaped into `RCPT TO:<…>` and the
    // `To:` header, so whitespace, control characters and these delimiters are refused.
    let allowed = |c: char| c.is_ascii_graphic() && !"()<>[]:;\\,\"".contains(c);
    let one_at_between_text = matches!(
        email.split_once('@'),
        Some((local, domain)) if !local.is_empty() && !domain.is_empty() && !domain.contains('@')
    );
    if email.len() <= MAX_EMAIL_CHARS && email.chars().all(allowed) && one_at_between_text {
        Ok(())
    } else {
        Err(AuthenticationError::InvalidEmail)
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

/// Ends every session of the user except `keep`, and every login still waiting for its TOTP
/// code — e.g. once 2FA is switched on.
pub fn end_other_sessions(
    conn: &Connection,
    user_id: i64,
    keep: i64,
) -> Result<usize, DatabaseError> {
    db_login_challenges::delete_for_user(conn, user_id)?;
    let sessions = Database::search_for_multiple_rows_with_connection::<Sessions>(
        conn,
        &[SessionsDatabaseColumns::UserId],
        &[&user_id],
        QuerySeparator::And,
    )?;
    let mut ended = 0;
    for session in sessions.iter().filter(|session| session.session_id != keep) {
        ended += Database::remove_entry_with_connection(
            conn,
            DatabaseTypes::Session,
            session.session_id,
        )?;
    }
    Ok(ended)
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
    Database::remove_entry_with_connection_based_on_columns::<Sessions>(
        conn,
        &[
            SessionsDatabaseColumns::UserId,
            SessionsDatabaseColumns::DeviceId,
        ],
        &[&user_id, &device_information.device_id],
        QuerySeparator::And,
    )
    .map_err(|_| AuthenticationError::FailedDataStoring)?;
    Database::insert_with_connection(conn, &session)
        .map_err(|_| AuthenticationError::FailedDataStoring)?;
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

    #[test]
    fn a_username_is_3_to_32_letters_digits_underscores_dots_or_dashes() {
        let longest = "a".repeat(MAX_USERNAME_CHARS);
        for good in ["abc", "Alice_B.c-9", longest.as_str()] {
            assert!(check_username(good).is_ok(), "{good:?}");
        }

        let too_long = "a".repeat(MAX_USERNAME_CHARS + 1);
        for bad in [
            "",
            "ab",
            too_long.as_str(),
            "al@ice",
            "al:ice",
            "al ice",
            "ålice",
            "alice\n",
        ] {
            assert!(
                matches!(
                    check_username(bad),
                    Err(AuthenticationError::InvalidUsername)
                ),
                "{bad:?}"
            );
        }
    }

    #[test]
    fn an_email_is_one_at_between_text_in_printable_ascii() {
        let domain = "@example.com";
        let longest = format!("{}{domain}", "a".repeat(MAX_EMAIL_CHARS - domain.len()));
        assert_eq!(longest.len(), MAX_EMAIL_CHARS);
        for good in [
            "alice@example.com",
            "a.b+tag@sub.example.co",
            "o'neil@example.com",
            longest.as_str(),
        ] {
            assert!(check_email(good).is_ok(), "{good:?}");
        }

        let too_long = format!("a{longest}");
        for bad in [
            "",
            "alice",
            "@example.com",
            "alice@",
            "a@b@example.com",
            "älice@example.com",
            too_long.as_str(),
        ] {
            assert!(
                matches!(check_email(bad), Err(AuthenticationError::InvalidEmail)),
                "{bad:?}"
            );
        }
    }

    #[test]
    fn an_email_with_a_character_that_could_break_out_of_an_smtp_line_is_refused() {
        for c in [
            '\r', '\n', '\t', '\0', ' ', '(', ')', '<', '>', '[', ']', ':', ';', '\\', ',', '"',
        ] {
            let email = format!("ali{c}ce@example.com");
            assert!(
                matches!(check_email(&email), Err(AuthenticationError::InvalidEmail)),
                "{email:?}"
            );
        }
        assert!(check_email("alice@example.com\r\nDATA").is_err());
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

    #[test]
    fn ending_the_other_sessions_keeps_this_one_and_other_users_sessions() {
        let conn = memory_connection();
        let alice = crate::test_support::insert_user(&conn, "alice");
        let bob = crate::test_support::insert_user(&conn, "bob");
        let mut alices_sessions = Vec::new();
        for (user_id, token) in [(alice, "a1"), (alice, "a2"), (alice, "a3"), (bob, "b1")] {
            conn.execute(
                "INSERT INTO sessions (user_id, device_id, device_name, platform, \
                 refresh_token_hashed, refresh_token_valid_until, session_token_hashed, \
                 session_token_valid_until, last_used_at) \
                 VALUES (?1, ?2, 'phone', 'ios', ?2, 0, ?2, 0, 0)",
                rusqlite::params![user_id, token],
            )
            .unwrap();
            if user_id == alice {
                alices_sessions.push(conn.last_insert_rowid());
            }
        }
        conn.execute(
            "INSERT INTO login_challenges (user_id, token_hashed, expires_at, device_id, \
             device_name, platform) VALUES (?1, 'challenge', 0, 'device', 'phone', 'ios')",
            [alice],
        )
        .unwrap();
        let current = alices_sessions[1];

        assert_eq!(end_other_sessions(&conn, alice, current).unwrap(), 2);

        let remaining: Vec<i64> = conn
            .prepare("SELECT session_id FROM sessions ORDER BY session_id")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        let bobs: i64 = conn
            .query_row(
                "SELECT session_id FROM sessions WHERE user_id = ?1",
                [bob],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(remaining, vec![current, bobs]);
        let challenges: i64 = conn
            .query_row("SELECT COUNT(*) FROM login_challenges", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(challenges, 0, "a pending TOTP login ends too");
    }
}
