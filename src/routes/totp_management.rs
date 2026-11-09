//! TOTP two-factor authentication: setting it up, confirming it, and its state. The code step at
//! login and turning it off follow in the next step; their plans are the stubs at the bottom.

use std::{sync::Arc, time::SystemTime};

use rand::{rngs::SysRng, TryRng};
use rusqlite::Connection;
use totp_rs::{Builder, Totp as TotpGenerator, TotpError};

use crate::{
    data_models::totp_model::{ConfirmTOTPRequest, RecoveryCodesIssued, TOTPSetup, TOTPStatus},
    db::{convert_system_time_to_unix_time, Database},
    error_types::{AppErrors, AuthenticationError, DatabaseError},
    models::{
        DatabaseTypes, QuerySeparator, RecoveryCodes, RecoveryCodesColumns, TOTPDatabaseColumns,
        UserMetadata, TOTP,
    },
    routes::{
        auth::{end_other_sessions, hash_token},
        email_codes::normalize_code,
        login::{check_current_password, has_totp},
        totp_cipher::TotpCipher,
    },
};

/// The name authenticator apps show next to the account.
pub const ISSUER: &str = "Bookserver";
const SECRET_BYTES: usize = 20;
const RECOVERY_CODE_COUNT: usize = 10;
const RECOVERY_CODE_LENGTH: usize = 16;
// Crockford's base32: no I, L, O or U, the letters people misread.
const RECOVERY_ALPHABET: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";

// === Setup ===================================================================

/// Whether 2FA is on, and how many unused recovery codes are left.
pub async fn status(db: Arc<Database>, user_id: i64) -> Result<TOTPStatus, AppErrors> {
    db.read(move |conn| {
        Ok(TOTPStatus {
            enabled: has_totp(conn, user_id)?,
            recovery_codes_left: unused_recovery_codes(conn, user_id)?,
        })
    })
    .await
}

/// Starts a setup: a new secret for the authenticator app, stored encrypted and pending until a
/// code confirms it. Replaces an earlier pending setup.
pub async fn begin_setup(
    db: Arc<Database>,
    cipher: &TotpCipher,
    user_id: i64,
    now: SystemTime,
) -> Result<TOTPSetup, AppErrors> {
    let username = Arc::clone(&db)
        .read(move |conn| {
            Ok(Database::get_entry_with_connection::<UserMetadata>(conn, user_id)?.username)
        })
        .await?;
    let mut secret = [0u8; SECRET_BYTES];
    SysRng
        .try_fill_bytes(&mut secret)
        .map_err(|_| AuthenticationError::RngCreatorFailed)?;
    let generator = Builder::new()
        .with_secret(secret.to_vec())
        .with_account_name(username)
        .with_issuer(Some(ISSUER))
        .build()
        .map_err(totp_failure)?;
    let setup = TOTPSetup {
        otpauth_url: generator.to_url().map_err(totp_failure)?,
        secret_base32: generator.secret().to_base32(),
    };
    let encrypted = cipher.encrypt(user_id, &secret)?;

    db.transact(move |conn| {
        match find_setup(conn, user_id)? {
            Some(setup) if setup.confirmed => {
                return Err(AuthenticationError::TotpAlreadyEnabled.into());
            }
            Some(_) => {
                Database::remove_entry_with_connection(conn, DatabaseTypes::Totp, user_id)?;
            }
            None => {}
        }
        Database::insert_with_connection(
            conn,
            &TOTP {
                user_id,
                authentication_secret: encrypted,
                confirmed: false,
                last_used_step: None,
                failed_attempts: 0,
                lockouts: 0,
                last_failed_at: None,
                created_at: now,
            },
        )?;
        Ok(())
    })
    .await?;
    Ok(setup)
}

/// Confirms the pending setup with a code from the app and the password. Answers the recovery
/// codes — shown only now — and ends the user's other sessions, so every live session has
/// passed the second factor.
pub async fn confirm_setup(
    db: Arc<Database>,
    cipher: &TotpCipher,
    user_id: i64,
    current_session_id: i64,
    request: ConfirmTOTPRequest,
    now: SystemTime,
) -> Result<RecoveryCodesIssued, AppErrors> {
    let ConfirmTOTPRequest { code, password } = request;
    let pending = Arc::clone(&db)
        .read(move |conn| Ok(find_setup(conn, user_id)?))
        .await?
        .filter(|setup| !setup.confirmed)
        .ok_or(AuthenticationError::TotpSetupNotPending)?;
    check_current_password(Arc::clone(&db), user_id, password, now).await?;

    // Not counted against the wrong-code lock: whoever asks was just handed the secret.
    let secret = cipher.decrypt(user_id, &pending.authentication_secret)?;
    let step =
        matching_step(secret, &code, now)?.ok_or(AuthenticationError::InvalidValidationCode)?;

    let codes = generate_recovery_codes()?;
    let hashes: Vec<String> = codes
        .iter()
        .map(|code| hash_token(&normalize_recovery_code(code)))
        .collect();
    let checked_secret = pending.authentication_secret;
    db.transact(move |conn| {
        // A parallel confirm, or a setup started again, changed the row since the code was
        // checked; the write lock keeps it from changing between this read and the update.
        let still_pending = find_setup(conn, user_id)?
            .is_some_and(|setup| !setup.confirmed && setup.authentication_secret == checked_secret);
        if !still_pending {
            return Err(AuthenticationError::TotpSetupNotPending.into());
        }
        Database::update_multiple_values_with_connection::<TOTP>(
            conn,
            &[
                TOTPDatabaseColumns::Confirmed,
                TOTPDatabaseColumns::LastUsedStep,
            ],
            &[&true, &step],
            user_id,
        )?;
        replace_recovery_codes(conn, user_id, &hashes)?;
        end_other_sessions(conn, user_id, current_session_id)?;
        Ok(())
    })
    .await?;
    Ok(RecoveryCodesIssued {
        recovery_codes: codes,
    })
}

/// The user's setup, confirmed or pending.
fn find_setup(conn: &Connection, user_id: i64) -> Result<Option<TOTP>, DatabaseError> {
    Database::search_for_single_row_with_connection::<TOTP>(
        conn,
        &[TOTPDatabaseColumns::UserId],
        &[&user_id],
        QuerySeparator::And,
    )
}

/// The time step a typed code belongs to, if it is right for `secret` at `now`; one step of clock
/// drift either way is accepted. Spaces and dashes are ignored.
fn matching_step(
    secret: Vec<u8>,
    typed: &str,
    now: SystemTime,
) -> Result<Option<i64>, AuthenticationError> {
    let generator: TotpGenerator = Builder::new()
        .with_secret(secret)
        .build()
        .map_err(totp_failure)?;
    let unix_now = u64::try_from(convert_system_time_to_unix_time(now))
        .map_err(|_| AuthenticationError::TotpFailure)?;
    Ok(generator
        .check(&normalize_code(typed), unix_now)
        .and_then(|step| i64::try_from(step).ok()))
}

fn totp_failure(err: TotpError) -> AuthenticationError {
    tracing::error!("building a TOTP generator failed: {err}");
    AuthenticationError::TotpFailure
}

// === Recovery codes ==========================================================

/// Ten codes of 16 random Crockford base32 characters — 80 bits each — as `XXXX-XXXX-XXXX-XXXX`.
fn generate_recovery_codes() -> Result<Vec<String>, AuthenticationError> {
    (0..RECOVERY_CODE_COUNT)
        .map(|_| {
            let mut bytes = [0u8; RECOVERY_CODE_LENGTH];
            SysRng
                .try_fill_bytes(&mut bytes)
                .map_err(|_| AuthenticationError::RngCreatorFailed)?;
            // 256 is a multiple of 32, so the low five bits keep every character equally likely.
            let characters: Vec<char> = bytes
                .iter()
                .map(|byte| char::from(RECOVERY_ALPHABET[usize::from(byte & 31)]))
                .collect();
            Ok(characters
                .chunks(4)
                .map(|group| group.iter().collect::<String>())
                .collect::<Vec<_>>()
                .join("-"))
        })
        .collect()
}

/// A recovery code as people type it: any case, with spaces or dashes, `O` for `0` and `I` or
/// `L` for `1`.
pub fn normalize_recovery_code(input: &str) -> String {
    normalize_code(input)
        .chars()
        .map(|c| match c {
            'O' => '0',
            'I' | 'L' => '1',
            other => other,
        })
        .collect()
}

/// Replaces all of the user's recovery codes with these hashes, all unused.
fn replace_recovery_codes(
    conn: &Connection,
    user_id: i64,
    hashes: &[String],
) -> Result<(), DatabaseError> {
    Database::remove_entry_with_connection_based_on_columns::<RecoveryCodes>(
        conn,
        &[RecoveryCodesColumns::UserId],
        &[&user_id],
        QuerySeparator::And,
    )?;
    for hash in hashes {
        Database::insert_with_connection(
            conn,
            &RecoveryCodes {
                recovery_id: 0,
                user_id,
                code_hashed: hash.clone(),
                used: false,
            },
        )?;
    }
    Ok(())
}

fn unused_recovery_codes(conn: &Connection, user_id: i64) -> Result<usize, DatabaseError> {
    let unused = Database::search_for_multiple_rows_with_connection::<RecoveryCodes>(
        conn,
        &[RecoveryCodesColumns::UserId, RecoveryCodesColumns::Used],
        &[&user_id, &false],
        QuerySeparator::And,
    )?;
    Ok(unused.len())
}

// === Verification ============================================================

pub fn verify_totp_log_in() {
    //user sends in the session code
    //verify code
    //when validated create session and refresh token
    //send to user
}

pub fn authenticate_using_recovery_code() {
    //user uses recovery code
    //if recovery code has been used return error
    //checks against the hashed recovery code in the db
    //if check is true then log in
    //mark code as used
}

// === Disabling ===============================================================

pub fn disable_totp() {
    // Require current password (or another strong verification).
    // Delete TOTP secret.
    // Delete all recovery codes.
    //invalidate all active sessions (refresh tokens, session tokens,recovery tokens)
    // Mark TOTP as disabled.
}

#[cfg(test)]
mod tests {
    use std::{
        collections::HashSet,
        time::{Duration, UNIX_EPOCH},
    };

    use rusqlite::params;
    use totp_rs::Secret;

    use super::*;
    use crate::{
        routes::auth::hash_secret,
        test_support::{test_cipher, TempDatabase},
    };

    const PASSWORD: &str = "correct horse battery";
    const NOW: u64 = 1_700_000_000;

    fn at(secs: u64) -> SystemTime {
        UNIX_EPOCH + Duration::from_secs(secs)
    }

    struct Alice {
        user_id: i64,
        /// The session she sets 2FA up from; she has a second one on another device.
        session_id: i64,
    }

    async fn alice(temp: &TempDatabase) -> Alice {
        let hash = hash_secret(PASSWORD.to_string()).await.unwrap();
        temp.db
            .setup_new_transaction(move |conn| {
                conn.execute(
                    "INSERT INTO users (username, password_hash, email, email_verified) \
                     VALUES ('alice', ?1, 'alice@example.com', 1)",
                    [hash],
                )
                .unwrap();
                let user_id = conn.last_insert_rowid();
                for device in ["laptop", "phone"] {
                    conn.execute(
                        "INSERT INTO sessions (user_id, device_id, device_name, platform, \
                         refresh_token_hashed, refresh_token_valid_until, session_token_hashed, \
                         session_token_valid_until, last_used_at) \
                         VALUES (?1, ?2, ?2, 'ios', ?2, 0, ?2, 0, 0)",
                        params![user_id, device],
                    )
                    .unwrap();
                }
                Ok(Alice {
                    user_id,
                    session_id: conn.last_insert_rowid(),
                })
            })
            .unwrap()
    }

    async fn begin(temp: &TempDatabase, alice: &Alice) -> Result<TOTPSetup, AppErrors> {
        begin_setup(Arc::clone(&temp.db), &test_cipher(), alice.user_id, at(NOW)).await
    }

    async fn confirm(
        temp: &TempDatabase,
        alice: &Alice,
        code: &str,
        password: &str,
    ) -> Result<RecoveryCodesIssued, AppErrors> {
        let request = ConfirmTOTPRequest {
            code: code.to_string(),
            password: password.to_string(),
        };
        confirm_setup(
            Arc::clone(&temp.db),
            &test_cipher(),
            alice.user_id,
            alice.session_id,
            request,
            at(NOW),
        )
        .await
    }

    /// What the authenticator app makes of a setup.
    fn authenticator(setup: &TOTPSetup) -> TotpGenerator {
        Builder::new()
            .with_secret(Secret::try_from_base32(&setup.secret_base32).unwrap())
            .build()
            .unwrap()
    }

    fn code_at(app: &TotpGenerator, secs: u64) -> String {
        app.generate(secs).to_string()
    }

    /// Six digits that are right for none of the steps `check` accepts at `secs`.
    fn wrong_code(app: &TotpGenerator, secs: u64) -> String {
        let right: Vec<String> = [secs - 30, secs, secs + 30]
            .iter()
            .map(|&time| code_at(app, time))
            .collect();
        (0..4)
            .map(|n| format!("{n:06}"))
            .find(|code| !right.contains(code))
            .unwrap()
    }

    fn stored_setup(temp: &TempDatabase, user_id: i64) -> Option<TOTP> {
        temp.db
            .setup_new_transaction(move |conn| Ok(find_setup(conn, user_id)?))
            .unwrap()
    }

    fn column<T: rusqlite::types::FromSql + Send + 'static>(
        temp: &TempDatabase,
        sql: &'static str,
        user_id: i64,
    ) -> Vec<T> {
        temp.db
            .setup_new_transaction(move |conn| {
                Ok(conn
                    .prepare(sql)
                    .unwrap()
                    .query_map([user_id], |row| row.get(0))
                    .unwrap()
                    .collect::<Result<_, _>>()
                    .unwrap())
            })
            .unwrap()
    }

    fn sessions_of(temp: &TempDatabase, alice: &Alice) -> Vec<i64> {
        column(
            temp,
            "SELECT session_id FROM sessions WHERE user_id = ?1 ORDER BY session_id",
            alice.user_id,
        )
    }

    fn stored_code_hashes(temp: &TempDatabase, alice: &Alice) -> Vec<String> {
        column(
            temp,
            "SELECT code_hash FROM recovery_codes WHERE user_id = ?1 ORDER BY code_hash",
            alice.user_id,
        )
    }

    fn hashes_of(codes: &[String]) -> Vec<String> {
        let mut hashes: Vec<String> = codes
            .iter()
            .map(|code| hash_token(&normalize_recovery_code(code)))
            .collect();
        hashes.sort();
        hashes
    }

    fn failed_with(result: &Result<RecoveryCodesIssued, AppErrors>) -> &'static str {
        match result {
            Ok(_) => "accepted",
            Err(AppErrors::Authentication(AuthenticationError::IncorrectPassword)) => "password",
            Err(AppErrors::Authentication(AuthenticationError::InvalidValidationCode)) => "code",
            Err(AppErrors::Authentication(AuthenticationError::TotpSetupNotPending)) => {
                "not pending"
            }
            Err(_) => "other error",
        }
    }

    #[tokio::test]
    async fn starting_a_setup_hands_out_a_secret_and_stores_it_encrypted_and_pending() {
        let temp = TempDatabase::open();
        let alice = alice(&temp).await;

        let setup = begin(&temp, &alice).await.unwrap();

        assert_eq!(
            setup.otpauth_url,
            format!(
                "otpauth://totp/Bookserver:alice?secret={}&issuer=Bookserver",
                setup.secret_base32
            )
        );
        let secret = Secret::try_from_base32(&setup.secret_base32).unwrap();
        assert_eq!(secret.as_bytes().len(), 20, "160 bits");
        let stored = stored_setup(&temp, alice.user_id).unwrap();
        assert!(!stored.confirmed);
        assert!(!stored.authentication_secret.contains(&setup.secret_base32));
        assert_eq!(
            test_cipher()
                .decrypt(alice.user_id, &stored.authentication_secret)
                .unwrap(),
            secret.as_bytes()
        );
    }

    #[tokio::test]
    async fn starting_again_replaces_a_pending_setup() {
        let temp = TempDatabase::open();
        let alice = alice(&temp).await;

        let first = begin(&temp, &alice).await.unwrap();
        let second = begin(&temp, &alice).await.unwrap();

        assert_ne!(first.secret_base32, second.secret_base32);
        let stored = stored_setup(&temp, alice.user_id).unwrap();
        assert_eq!(
            test_cipher()
                .decrypt(alice.user_id, &stored.authentication_secret)
                .unwrap(),
            Secret::try_from_base32(&second.secret_base32)
                .unwrap()
                .as_bytes()
        );
        let rows: Vec<i64> = column(
            &temp,
            "SELECT user_id FROM totp WHERE user_id = ?1",
            alice.user_id,
        );
        assert_eq!(rows.len(), 1);
    }

    #[tokio::test]
    async fn confirming_turns_2fa_on_with_ten_codes_and_ends_the_other_sessions() {
        let temp = TempDatabase::open();
        let alice = alice(&temp).await;
        let app = authenticator(&begin(&temp, &alice).await.unwrap());

        let codes = confirm(&temp, &alice, &code_at(&app, NOW), PASSWORD)
            .await
            .unwrap()
            .recovery_codes;

        assert_eq!(codes.len(), 10);
        assert_eq!(codes.iter().collect::<HashSet<_>>().len(), 10, "distinct");
        assert_eq!(
            stored_code_hashes(&temp, &alice),
            hashes_of(&codes),
            "only the hashes are stored"
        );
        let stored = stored_setup(&temp, alice.user_id).unwrap();
        assert!(stored.confirmed);
        assert_eq!(
            stored.last_used_step,
            Some((NOW / 30) as i64),
            "the confirming code can't be used again"
        );
        assert_eq!(sessions_of(&temp, &alice), vec![alice.session_id]);
    }

    #[tokio::test]
    async fn confirming_needs_the_password_and_a_right_code() {
        let temp = TempDatabase::open();
        let alice = alice(&temp).await;
        let app = authenticator(&begin(&temp, &alice).await.unwrap());
        let right = code_at(&app, NOW);
        let wrong = wrong_code(&app, NOW);

        for (code, password, expected) in [
            (right.as_str(), "wrong horse battery", "password"),
            (wrong.as_str(), PASSWORD, "code"),
            ("12a456", PASSWORD, "code"),
        ] {
            let result = confirm(&temp, &alice, code, password).await;
            assert_eq!(failed_with(&result), expected, "{code} / {password}");
        }
        assert!(!stored_setup(&temp, alice.user_id).unwrap().confirmed);
        assert_eq!(sessions_of(&temp, &alice).len(), 2, "no session ended");

        let typed_with_a_space = format!("{} {}", &right[..3], &right[3..]);
        let result = confirm(&temp, &alice, &typed_with_a_space, PASSWORD).await;
        assert_eq!(failed_with(&result), "accepted");
    }

    #[tokio::test]
    async fn a_setup_is_confirmed_once_and_not_restarted_while_2fa_is_on() {
        let temp = TempDatabase::open();
        let alice = alice(&temp).await;
        let without_setup = confirm(&temp, &alice, "123456", PASSWORD).await;
        assert_eq!(failed_with(&without_setup), "not pending");

        let app = authenticator(&begin(&temp, &alice).await.unwrap());
        confirm(&temp, &alice, &code_at(&app, NOW), PASSWORD)
            .await
            .unwrap();

        let again = confirm(&temp, &alice, &code_at(&app, NOW + 30), PASSWORD).await;
        assert_eq!(failed_with(&again), "not pending");
        assert!(matches!(
            begin(&temp, &alice).await,
            Err(AppErrors::Authentication(
                AuthenticationError::TotpAlreadyEnabled
            ))
        ));
    }

    #[tokio::test]
    async fn of_two_parallel_confirms_one_wins_and_only_its_codes_are_stored() {
        let temp = TempDatabase::open();
        let alice = alice(&temp).await;
        let app = authenticator(&begin(&temp, &alice).await.unwrap());
        let code = code_at(&app, NOW);

        let (a, b) = tokio::join!(
            confirm(&temp, &alice, &code, PASSWORD),
            confirm(&temp, &alice, &code, PASSWORD),
        );

        let outcomes = [failed_with(&a), failed_with(&b)];
        assert!(
            outcomes.contains(&"accepted") && outcomes.contains(&"not pending"),
            "{outcomes:?}"
        );
        let winner = [a, b].into_iter().find_map(Result::ok).unwrap();
        assert_eq!(
            stored_code_hashes(&temp, &alice),
            hashes_of(&winner.recovery_codes)
        );
    }

    #[tokio::test]
    async fn the_status_tells_whether_2fa_is_on_and_how_many_codes_are_left() {
        let temp = TempDatabase::open();
        let alice = alice(&temp).await;
        let state = || status(Arc::clone(&temp.db), alice.user_id);
        let seen = |status: TOTPStatus| (status.enabled, status.recovery_codes_left);

        assert_eq!(seen(state().await.unwrap()), (false, 0));
        let app = authenticator(&begin(&temp, &alice).await.unwrap());
        assert_eq!(seen(state().await.unwrap()), (false, 0), "pending is off");

        confirm(&temp, &alice, &code_at(&app, NOW), PASSWORD)
            .await
            .unwrap();
        assert_eq!(seen(state().await.unwrap()), (true, 10));

        temp.db
            .setup_new_transaction(|conn| {
                conn.execute(
                    "UPDATE recovery_codes SET used = 1 WHERE recovery_id = \
                     (SELECT MIN(recovery_id) FROM recovery_codes)",
                    [],
                )
                .unwrap();
                Ok(())
            })
            .unwrap();
        assert_eq!(seen(state().await.unwrap()), (true, 9));
    }

    #[test]
    fn recovery_codes_are_ten_distinct_groups_of_crockford_characters() {
        let codes = generate_recovery_codes().unwrap();
        assert_eq!(codes.len(), RECOVERY_CODE_COUNT);
        assert_eq!(codes.iter().collect::<HashSet<_>>().len(), codes.len());
        for code in &codes {
            let groups: Vec<&str> = code.split('-').collect();
            assert_eq!(groups.len(), 4, "{code}");
            for group in groups {
                assert_eq!(group.len(), 4, "{code}");
                assert!(
                    group.bytes().all(|c| RECOVERY_ALPHABET.contains(&c)),
                    "{code}"
                );
            }
        }
    }

    #[test]
    fn recovery_codes_are_read_the_way_people_type_them() {
        assert_eq!(
            normalize_recovery_code(" abcd-efgh 0ijk-lmno "),
            "ABCDEFGH01JK1MN0"
        );
        for code in generate_recovery_codes().unwrap() {
            let typed = code.to_lowercase().replace('-', " ");
            assert_eq!(
                normalize_recovery_code(&typed),
                normalize_recovery_code(&code)
            );
        }
    }
}
