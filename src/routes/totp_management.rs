//! TOTP two-factor authentication: setting it up, its state, the code step at login and turning
//! it off, with the recovery codes that stand in for the authenticator app.

use std::{
    sync::Arc,
    time::{Duration, SystemTime},
};

use rand::{rngs::SysRng, TryRng};
use rusqlite::{types::Null, Connection};
use totp_rs::{Builder, Totp as TotpGenerator, TotpError};

use crate::{
    data_models::{
        authentication_model::{DeviceInformation, SessionTokens},
        totp_model::{
            ConfirmTOTPRequest, DisableTOTPRequest, RecoveryCodesIssued, TOTPSetup, TOTPStatus,
        },
    },
    database_related_scripts::db_login_challenges,
    db::{convert_system_time_to_unix_time, Database},
    error_types::{AppErrors, AuthenticationError, DatabaseError},
    models::{
        DatabaseTypes, LoginChallenge, QuerySeparator, RecoveryCodes, RecoveryCodesColumns,
        TOTPDatabaseColumns, UserMetadata, TOTP,
    },
    routes::{
        auth::{end_all_sessions, end_other_sessions, hash_token},
        email_codes::{elapsed, normalize_code},
        login::{check_current_password, has_totp, open_session},
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
const TRIES_PER_ROUND: i64 = 5;
const FIRST_LOCK: Duration = Duration::from_secs(15 * 60);
const LONGEST_LOCK: Duration = Duration::from_secs(24 * 60 * 60);

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

// === Login ===================================================================

/// The code step of a TOTP login: a right code turns the challenge into a session on the
/// challenge's device. A wrong code leaves the challenge alive until it expires.
pub async fn finish_login(
    db: Arc<Database>,
    cipher: &TotpCipher,
    challenge: LoginChallenge,
    code: TypedCode,
    now: SystemTime,
) -> Result<SessionTokens, AppErrors> {
    let user_id = challenge.user_id;
    let token_hash = challenge.token_hashed.clone();
    let totp = Arc::clone(&db)
        .transact(move |conn| {
            // Checked again although the extractor found it: an expired challenge, or one a
            // parallel request has used meanwhile, costs no try.
            if db_login_challenges::find_live(conn, &token_hash, now)?.is_none() {
                return Err(AuthenticationError::UnauthorizedSession.into());
            }
            reserve_code_try(conn, user_id, now).map_err(|err| match err {
                // A challenge whose user has no 2FA any more is void; only a hand edit of the
                // database leaves one behind.
                AppErrors::Authentication(AuthenticationError::TotpNotEnabled) => {
                    AuthenticationError::UnauthorizedSession.into()
                }
                other => other,
            })
        })
        .await?;
    let proof = check_code(Arc::clone(&db), cipher, totp, code, now).await?;

    db.transact(move |conn| {
        let removed = Database::remove_entry_with_connection(
            conn,
            DatabaseTypes::LoginChallenge,
            challenge.challenge_id,
        )?;
        if removed != 1 {
            return Err(AuthenticationError::UnauthorizedSession.into());
        }
        use_proof(conn, user_id, &proof)?;
        let device = DeviceInformation {
            device_id: &challenge.device_id,
            device_name: &challenge.device_name,
            platform: &challenge.platform,
        };
        open_session(conn, user_id, device, now)
    })
    .await
}

// === Disabling ===============================================================

/// Turns 2FA off with the password and an authenticator or recovery code. The setup and the
/// recovery codes are deleted, and every session and pending login of the user ends — the
/// current one too.
pub async fn disable(
    db: Arc<Database>,
    cipher: &TotpCipher,
    user_id: i64,
    request: DisableTOTPRequest,
    now: SystemTime,
) -> Result<(), AppErrors> {
    let DisableTOTPRequest { password, code } = request;
    let enabled = Arc::clone(&db)
        .read(move |conn| Ok(has_totp(conn, user_id)?))
        .await?;
    if !enabled {
        return Err(AuthenticationError::TotpNotEnabled.into());
    }
    check_current_password(Arc::clone(&db), user_id, password, now).await?;
    let totp = Arc::clone(&db)
        .transact(move |conn| reserve_code_try(conn, user_id, now))
        .await?;
    let proof = check_code(
        Arc::clone(&db),
        cipher,
        totp,
        TypedCode::from_either(code),
        now,
    )
    .await?;

    db.transact(move |conn| {
        use_proof(conn, user_id, &proof)?;
        Database::remove_entry_with_connection(conn, DatabaseTypes::Totp, user_id)?;
        delete_recovery_codes(conn, user_id)?;
        end_all_sessions(conn, user_id)?;
        Ok(())
    })
    .await
}

// === Checking a code =========================================================

/// A second-factor code as the user typed it.
pub enum TypedCode {
    /// Six digits from the authenticator app.
    Authenticator(String),
    /// One of the recovery codes shown when 2FA was switched on.
    Recovery(String),
}

impl TypedCode {
    /// A field that takes either kind: six digits are an authenticator code, anything else a
    /// recovery code.
    pub fn from_either(code: String) -> Self {
        let normalized = normalize_code(&code);
        if normalized.len() == 6 && normalized.bytes().all(|byte| byte.is_ascii_digit()) {
            TypedCode::Authenticator(code)
        } else {
            TypedCode::Recovery(code)
        }
    }
}

/// A code that was right when it was checked, for the final transaction to use up.
enum Proof {
    /// The time step of an authenticator code, and the stored (encrypted) secret it was checked
    /// against.
    AuthenticatorStep { step: i64, checked_secret: String },
    /// The hash of an unused recovery code.
    RecoveryCode { code_hash: String },
}

/// Counts a try against the user's wrong-code lock, inside the caller's reserve transaction, and
/// returns the setup it was counted on. The caller commits before checking the code: counted in
/// the same transaction as a failed check, the try would be rolled back with it.
fn reserve_code_try(conn: &Connection, user_id: i64, now: SystemTime) -> Result<TOTP, AppErrors> {
    let totp = find_setup(conn, user_id)?
        .filter(|totp| totp.confirmed)
        .ok_or(AuthenticationError::TotpNotEnabled)?;
    let counters = counters_for_next_try(&totp, now)
        .map_err(|retry_after| AuthenticationError::TooManyAttempts { retry_after })?;
    save_try_counters(conn, user_id, &counters)?;
    Ok(totp)
}

/// Checks a code against the setup its try was counted on. An authenticator code that was
/// already used fails like a wrong one, on the same path.
async fn check_code(
    db: Arc<Database>,
    cipher: &TotpCipher,
    totp: TOTP,
    code: TypedCode,
    now: SystemTime,
) -> Result<Proof, AppErrors> {
    let user_id = totp.user_id;
    match code {
        TypedCode::Authenticator(typed) => {
            let secret = cipher.decrypt(user_id, &totp.authentication_secret)?;
            let step = matching_step(secret, &typed, now)?
                .filter(|&step| is_newer(step, totp.last_used_step))
                .ok_or(AuthenticationError::InvalidValidationCode)?;
            Ok(Proof::AuthenticatorStep {
                step,
                checked_secret: totp.authentication_secret,
            })
        }
        TypedCode::Recovery(typed) => {
            let code_hash = hash_token(&normalize_recovery_code(&typed));
            let lookup = code_hash.clone();
            let unused = db
                .read(move |conn| Ok(find_unused_recovery_code(conn, user_id, &lookup)?))
                .await?
                .is_some();
            if !unused {
                return Err(AuthenticationError::InvalidValidationCode.into());
            }
            Ok(Proof::RecoveryCode { code_hash })
        }
    }
}

/// Uses up a checked code in the caller's final transaction and lifts the wrong-code lock. Checked
/// again first: since the check, a parallel request may have used the code, or 2FA may have been
/// switched off or set up anew.
fn use_proof(conn: &Connection, user_id: i64, proof: &Proof) -> Result<(), AppErrors> {
    let totp = find_setup(conn, user_id)?
        .filter(|totp| totp.confirmed)
        .ok_or(AuthenticationError::TotpNotEnabled)?;
    match proof {
        Proof::AuthenticatorStep {
            step,
            checked_secret,
        } => {
            if totp.authentication_secret != *checked_secret
                || !is_newer(*step, totp.last_used_step)
            {
                return Err(AuthenticationError::InvalidValidationCode.into());
            }
            Database::update_value_with_connection::<TOTP>(
                conn,
                TOTPDatabaseColumns::LastUsedStep,
                step,
                user_id,
            )?;
        }
        Proof::RecoveryCode { code_hash } => {
            let code = find_unused_recovery_code(conn, user_id, code_hash)?
                .ok_or(AuthenticationError::InvalidValidationCode)?;
            Database::update_value_with_connection::<RecoveryCodes>(
                conn,
                RecoveryCodesColumns::Used,
                &true,
                code.recovery_id,
            )?;
        }
    }
    reset_try_counters(conn, user_id)?;
    Ok(())
}

/// Whether an authenticator code's time step comes after the last one used.
fn is_newer(step: i64, last_used_step: Option<i64>) -> bool {
    last_used_step.is_none_or(|last| step > last)
}

// === Wrong-code lock =========================================================

/// The wrong-code lock's counters, stored with every try at a code.
#[derive(Debug, PartialEq, Eq)]
struct TryCounters {
    failed_attempts: i64,
    lockouts: i64,
    last_failed_at: SystemTime,
}

/// The counters for one more try at a code now, or — while the user is locked out — how long
/// the lock still lasts. The fifth try of a round starts a lock; once it is over, the next try
/// starts a new round.
fn counters_for_next_try(totp: &TOTP, now: SystemTime) -> Result<TryCounters, Duration> {
    let tries_this_round = if totp.failed_attempts < TRIES_PER_ROUND {
        totp.failed_attempts
    } else {
        let lock_left = totp.last_failed_at.map_or(Duration::ZERO, |locked_at| {
            lock_length(totp.lockouts).saturating_sub(elapsed(locked_at, now))
        });
        if !lock_left.is_zero() {
            return Err(lock_left);
        }
        0
    };
    let failed_attempts = tries_this_round + 1;
    Ok(TryCounters {
        failed_attempts,
        lockouts: totp.lockouts + i64::from(failed_attempts == TRIES_PER_ROUND),
        last_failed_at: now,
    })
}

/// 15 minutes for the first lock, doubled for each further one, at most 24 hours.
fn lock_length(lockouts: i64) -> Duration {
    // From the eighth lock on the doubled length is past 24 hours anyway; the clamp keeps the
    // shift from overflowing.
    let doublings = lockouts.saturating_sub(1).clamp(0, 7) as u32;
    FIRST_LOCK.saturating_mul(1 << doublings).min(LONGEST_LOCK)
}

fn save_try_counters(
    conn: &Connection,
    user_id: i64,
    counters: &TryCounters,
) -> Result<(), DatabaseError> {
    Database::update_multiple_values_with_connection::<TOTP>(
        conn,
        &[
            TOTPDatabaseColumns::FailedAttempts,
            TOTPDatabaseColumns::Lockouts,
            TOTPDatabaseColumns::LastFailedAt,
        ],
        &[
            &counters.failed_attempts,
            &counters.lockouts,
            &convert_system_time_to_unix_time(counters.last_failed_at),
        ],
        user_id,
    )?;
    Ok(())
}

fn reset_try_counters(conn: &Connection, user_id: i64) -> Result<(), DatabaseError> {
    Database::update_multiple_values_with_connection::<TOTP>(
        conn,
        &[
            TOTPDatabaseColumns::FailedAttempts,
            TOTPDatabaseColumns::Lockouts,
            TOTPDatabaseColumns::LastFailedAt,
        ],
        &[&0, &0, &Null],
        user_id,
    )?;
    Ok(())
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
    delete_recovery_codes(conn, user_id)?;
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

fn delete_recovery_codes(conn: &Connection, user_id: i64) -> Result<usize, DatabaseError> {
    Database::remove_entry_with_connection_based_on_columns::<RecoveryCodes>(
        conn,
        &[RecoveryCodesColumns::UserId],
        &[&user_id],
        QuerySeparator::And,
    )
}

/// The user's unused recovery code with this hash. Always looked up together with the user:
/// the hash alone would find another account's code.
fn find_unused_recovery_code(
    conn: &Connection,
    user_id: i64,
    code_hash: &str,
) -> Result<Option<RecoveryCodes>, DatabaseError> {
    Database::search_for_single_row_with_connection::<RecoveryCodes>(
        conn,
        &[
            RecoveryCodesColumns::UserId,
            RecoveryCodesColumns::CodeHashed,
            RecoveryCodesColumns::Used,
        ],
        &[&user_id, &code_hash, &false],
        QuerySeparator::And,
    )
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

#[cfg(test)]
mod tests {
    use std::{collections::HashSet, time::UNIX_EPOCH};

    use futures::future::join_all;
    use rusqlite::params;
    use totp_rs::Secret;

    use super::*;
    use crate::{
        data_models::authentication_model::LoginResponse,
        routes::{
            auth::hash_secret,
            login::{start_login, CHALLENGE_LIFETIME},
        },
        test_support::{insert_user, memory_connection, test_cipher, TempDatabase},
    };

    const PASSWORD: &str = "correct horse battery";
    /// When 2FA is set up in these tests.
    const NOW: u64 = 1_700_000_000;
    /// When most logins in these tests happen: two time steps after the setup.
    const LATER: u64 = NOW + 60;

    fn at(secs: u64) -> SystemTime {
        UNIX_EPOCH + Duration::from_secs(secs)
    }

    struct Account {
        user_id: i64,
        /// The session 2FA is set up from; the account has a second one on another device.
        session_id: i64,
    }

    async fn account(temp: &TempDatabase, name: &str) -> Account {
        let hash = hash_secret(PASSWORD.to_string()).await.unwrap();
        let name = name.to_string();
        temp.db
            .setup_new_transaction(move |conn| {
                conn.execute(
                    "INSERT INTO users (username, password_hash, email, email_verified) \
                     VALUES (?1, ?2, ?1 || '@example.com', 1)",
                    params![name, hash],
                )
                .unwrap();
                let user_id = conn.last_insert_rowid();
                // Opened at `NOW` like a real login's, so a later login in a test keeps them:
                // it ends only the sessions whose refresh token ran out.
                let refresh_valid_until = (NOW + 30 * 24 * 60 * 60) as i64;
                for device in ["laptop", "phone"] {
                    conn.execute(
                        "INSERT INTO sessions (user_id, device_id, device_name, platform, \
                         refresh_token_hashed, refresh_token_valid_until, session_token_hashed, \
                         session_token_valid_until, last_used_at) \
                         VALUES (?1, ?2, ?2, 'ios', ?3, ?4, ?3, 0, 0)",
                        params![
                            user_id,
                            device,
                            format!("{name}-{device}"),
                            refresh_valid_until
                        ],
                    )
                    .unwrap();
                }
                Ok(Account {
                    user_id,
                    session_id: conn.last_insert_rowid(),
                })
            })
            .unwrap()
    }

    async fn alice(temp: &TempDatabase) -> Account {
        account(temp, "alice").await
    }

    async fn begin(temp: &TempDatabase, account: &Account) -> Result<TOTPSetup, AppErrors> {
        begin_setup(
            Arc::clone(&temp.db),
            &test_cipher(),
            account.user_id,
            at(NOW),
        )
        .await
    }

    async fn confirm(
        temp: &TempDatabase,
        account: &Account,
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
            account.user_id,
            account.session_id,
            request,
            at(NOW),
        )
        .await
    }

    /// An account with 2FA switched on at `NOW`: its authenticator app and its recovery codes.
    async fn enrolled(temp: &TempDatabase, name: &str) -> (Account, TotpGenerator, Vec<String>) {
        let account = account(temp, name).await;
        let app = authenticator(&begin(temp, &account).await.unwrap());
        let codes = confirm(temp, &account, &code_at(&app, NOW), PASSWORD)
            .await
            .unwrap()
            .recovery_codes;
        (account, app, codes)
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

    fn app_code(app: &TotpGenerator, secs: u64) -> TypedCode {
        TypedCode::Authenticator(code_at(app, secs))
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

    fn device(device_id: &'static str) -> DeviceInformation<'static> {
        DeviceInformation {
            device_id,
            device_name: "tablet",
            platform: "ios",
        }
    }

    /// A login challenge, as login hands one out after the right password.
    fn challenge_at(
        temp: &TempDatabase,
        user_id: i64,
        device_id: &'static str,
        secs: u64,
    ) -> LoginChallenge {
        temp.db
            .setup_new_transaction(move |conn| {
                let LoginResponse::TotpRequired {
                    challenge_token, ..
                } = start_login(conn, user_id, device(device_id), at(secs))?
                else {
                    panic!("expected a challenge");
                };
                Ok(
                    db_login_challenges::find_live(conn, &hash_token(&challenge_token), at(secs))?
                        .expect("the challenge is live"),
                )
            })
            .unwrap()
    }

    async fn finish(
        temp: &TempDatabase,
        challenge: LoginChallenge,
        code: TypedCode,
        secs: u64,
    ) -> Result<SessionTokens, AppErrors> {
        finish_login(
            Arc::clone(&temp.db),
            &test_cipher(),
            challenge,
            code,
            at(secs),
        )
        .await
    }

    async fn turn_off(
        temp: &TempDatabase,
        account: &Account,
        password: &str,
        code: &str,
        secs: u64,
    ) -> Result<(), AppErrors> {
        let request = DisableTOTPRequest {
            password: password.to_string(),
            code: code.to_string(),
        };
        disable(
            Arc::clone(&temp.db),
            &test_cipher(),
            account.user_id,
            request,
            at(secs),
        )
        .await
    }

    fn stored_setup(temp: &TempDatabase, user_id: i64) -> Option<TOTP> {
        temp.db
            .setup_new_transaction(move |conn| Ok(find_setup(conn, user_id)?))
            .unwrap()
    }

    /// The stored wrong-code lock: tries this round, locks, and the time of the last try.
    fn lock_state(temp: &TempDatabase, user_id: i64) -> (i64, i64, Option<SystemTime>) {
        let totp = stored_setup(temp, user_id).expect("2FA is set up");
        (totp.failed_attempts, totp.lockouts, totp.last_failed_at)
    }

    fn codes_left(temp: &TempDatabase, user_id: i64) -> usize {
        temp.db
            .setup_new_transaction(move |conn| Ok(unused_recovery_codes(conn, user_id)?))
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

    fn sessions_of(temp: &TempDatabase, account: &Account) -> Vec<i64> {
        column(
            temp,
            "SELECT session_id FROM sessions WHERE user_id = ?1 ORDER BY session_id",
            account.user_id,
        )
    }

    fn stored_code_hashes(temp: &TempDatabase, account: &Account) -> Vec<String> {
        column(
            temp,
            "SELECT code_hash FROM recovery_codes WHERE user_id = ?1 ORDER BY code_hash",
            account.user_id,
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

    fn outcome<T>(result: &Result<T, AppErrors>) -> &'static str {
        match result {
            Ok(_) => "accepted",
            Err(AppErrors::Authentication(err)) => match err {
                AuthenticationError::IncorrectPassword => "password",
                AuthenticationError::InvalidValidationCode => "code",
                AuthenticationError::TooManyAttempts { .. } => "locked",
                AuthenticationError::UnauthorizedSession => "unauthorized",
                AuthenticationError::TotpSetupNotPending => "not pending",
                AuthenticationError::TotpNotEnabled => "not enabled",
                _ => "other error",
            },
            Err(_) => "other error",
        }
    }

    // --- Setup ------------------------------------------------------------------

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
            assert_eq!(outcome(&result), expected, "{code} / {password}");
        }
        assert!(!stored_setup(&temp, alice.user_id).unwrap().confirmed);
        assert_eq!(sessions_of(&temp, &alice).len(), 2, "no session ended");

        let typed_with_a_space = format!("{} {}", &right[..3], &right[3..]);
        let result = confirm(&temp, &alice, &typed_with_a_space, PASSWORD).await;
        assert_eq!(outcome(&result), "accepted");
    }

    #[tokio::test]
    async fn a_setup_is_confirmed_once_and_not_restarted_while_2fa_is_on() {
        let temp = TempDatabase::open();
        let alice = alice(&temp).await;
        let without_setup = confirm(&temp, &alice, "123456", PASSWORD).await;
        assert_eq!(outcome(&without_setup), "not pending");

        let app = authenticator(&begin(&temp, &alice).await.unwrap());
        confirm(&temp, &alice, &code_at(&app, NOW), PASSWORD)
            .await
            .unwrap();

        let again = confirm(&temp, &alice, &code_at(&app, NOW + 30), PASSWORD).await;
        assert_eq!(outcome(&again), "not pending");
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

        let outcomes = [outcome(&a), outcome(&b)];
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

    // --- The wrong-code lock ----------------------------------------------------

    fn tries(failed_attempts: i64, lockouts: i64, last_try: Option<u64>) -> TOTP {
        TOTP {
            user_id: 1,
            authentication_secret: String::new(),
            confirmed: true,
            last_used_step: None,
            failed_attempts,
            lockouts,
            last_failed_at: last_try.map(at),
            created_at: at(0),
        }
    }

    fn counted(failed_attempts: i64, lockouts: i64, secs: u64) -> Result<TryCounters, Duration> {
        Ok(TryCounters {
            failed_attempts,
            lockouts,
            last_failed_at: at(secs),
        })
    }

    #[test]
    fn every_try_counts_and_the_fifth_starts_a_lock() {
        for tries_before in 0..4 {
            assert_eq!(
                counters_for_next_try(&tries(tries_before, 0, Some(NOW)), at(LATER)),
                counted(tries_before + 1, 0, LATER)
            );
        }
        assert_eq!(
            counters_for_next_try(&tries(4, 0, Some(NOW)), at(LATER)),
            counted(5, 1, LATER),
            "the fifth try is still checked, and its time starts the lock"
        );
        assert_eq!(
            counters_for_next_try(&tries(5, 1, Some(LATER)), at(LATER + 1)),
            Err(Duration::from_secs(15 * 60 - 1)),
            "locked, with the time the lock still lasts"
        );
    }

    #[test]
    fn a_lock_opens_exactly_at_its_end_and_a_new_round_begins() {
        let first_lock = tries(5, 1, Some(NOW));
        assert_eq!(
            counters_for_next_try(&first_lock, at(NOW + 15 * 60 - 1)),
            Err(Duration::from_secs(1))
        );
        assert_eq!(
            counters_for_next_try(&first_lock, at(NOW + 15 * 60)),
            counted(1, 1, NOW + 15 * 60)
        );

        let second_lock = tries(5, 2, Some(NOW));
        assert_eq!(
            counters_for_next_try(&second_lock, at(NOW + 30 * 60 - 1)),
            Err(Duration::from_secs(1))
        );
        assert_eq!(
            counters_for_next_try(&second_lock, at(NOW + 30 * 60)),
            counted(1, 2, NOW + 30 * 60)
        );
    }

    #[test]
    fn locks_double_from_15_minutes_up_to_a_day() {
        let minutes: Vec<u64> = (1..=9)
            .map(|lockouts| lock_length(lockouts).as_secs() / 60)
            .collect();
        assert_eq!(minutes, [15, 30, 60, 120, 240, 480, 960, 1440, 1440]);
        assert_eq!(lock_length(i64::MAX), LONGEST_LOCK);
    }

    #[test]
    fn a_clock_that_went_backwards_keeps_the_lock() {
        assert_eq!(
            counters_for_next_try(&tries(5, 1, Some(NOW)), at(NOW - 60 * 60)),
            Err(Duration::from_secs(15 * 60)),
            "the whole lock: a clock that went back can't shorten it"
        );
    }

    #[test]
    fn a_full_round_without_a_time_is_not_locked() {
        assert_eq!(
            counters_for_next_try(&tries(5, 1, None), at(NOW)),
            counted(1, 1, NOW)
        );
    }

    #[test]
    fn six_digits_are_read_as_an_app_code_and_anything_else_as_a_recovery_code() {
        for typed in ["123456", "123 456", "123-456", " 123456 "] {
            assert!(
                matches!(
                    TypedCode::from_either(typed.to_string()),
                    TypedCode::Authenticator(_)
                ),
                "{typed:?}"
            );
        }
        for typed in ["ABCD-EFGH-JKMN-PQRS", "12345", "1234567", "12345a", ""] {
            assert!(
                matches!(
                    TypedCode::from_either(typed.to_string()),
                    TypedCode::Recovery(_)
                ),
                "{typed:?}"
            );
        }
    }

    // --- Using up a code --------------------------------------------------------

    fn add_confirmed_totp(conn: &Connection, user_id: i64, secret: &str, last_used_step: i64) {
        conn.execute(
            "INSERT INTO totp (user_id, authentication_secret, confirmed, last_used_step, \
             failed_attempts, lockouts, last_failed_at) VALUES (?1, ?2, 1, ?3, 3, 2, ?4)",
            params![user_id, secret, last_used_step, NOW as i64],
        )
        .unwrap();
    }

    fn add_recovery_code(conn: &Connection, user_id: i64, code_hash: &str) {
        conn.execute(
            "INSERT INTO recovery_codes (user_id, code_hash) VALUES (?1, ?2)",
            params![user_id, code_hash],
        )
        .unwrap();
    }

    fn stored_lock(conn: &Connection, user_id: i64) -> (i64, i64, Option<i64>) {
        conn.query_row(
            "SELECT failed_attempts, lockouts, last_failed_at FROM totp WHERE user_id = ?1",
            [user_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap()
    }

    fn step(step: i64, checked_secret: &str) -> Proof {
        Proof::AuthenticatorStep {
            step,
            checked_secret: checked_secret.to_string(),
        }
    }

    fn recovery(code_hash: &str) -> Proof {
        Proof::RecoveryCode {
            code_hash: code_hash.to_string(),
        }
    }

    #[test]
    fn an_app_code_is_used_up_once_and_only_against_the_secret_it_was_checked_with() {
        let conn = memory_connection();
        let alice = insert_user(&conn, "alice");
        add_confirmed_totp(&conn, alice, "secret-1", 100);

        for (proof, why) in [
            (step(100, "secret-1"), "the step already used"),
            (step(99, "secret-1"), "an older step"),
            (step(101, "secret-2"), "2FA set up anew since the check"),
        ] {
            assert_eq!(outcome(&use_proof(&conn, alice, &proof)), "code", "{why}");
        }
        assert_eq!(
            stored_lock(&conn, alice),
            (3, 2, Some(NOW as i64)),
            "a refusal changes nothing"
        );

        assert_eq!(
            outcome(&use_proof(&conn, alice, &step(101, "secret-1"))),
            "accepted"
        );
        assert_eq!(
            stored_lock(&conn, alice),
            (0, 0, None),
            "a right code lifts the lock"
        );
        assert_eq!(
            outcome(&use_proof(&conn, alice, &step(101, "secret-1"))),
            "code",
            "once only"
        );
    }

    #[test]
    fn a_recovery_code_is_used_up_once_and_only_by_its_owner() {
        let conn = memory_connection();
        let alice = insert_user(&conn, "alice");
        let bob = insert_user(&conn, "bob");
        add_confirmed_totp(&conn, alice, "secret-a", 100);
        add_confirmed_totp(&conn, bob, "secret-b", 100);
        add_recovery_code(&conn, alice, "hash-a");
        add_recovery_code(&conn, bob, "hash-b");

        assert_eq!(
            outcome(&use_proof(&conn, alice, &recovery("hash-b"))),
            "code",
            "bob's code"
        );
        assert_eq!(
            outcome(&use_proof(&conn, alice, &recovery("hash-a"))),
            "accepted"
        );
        assert_eq!(stored_lock(&conn, alice), (0, 0, None));
        assert_eq!(
            outcome(&use_proof(&conn, alice, &recovery("hash-a"))),
            "code",
            "once only"
        );

        let unused: Vec<String> = conn
            .prepare("SELECT code_hash FROM recovery_codes WHERE used = 0")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert_eq!(unused, ["hash-b"]);
    }

    #[test]
    fn nothing_is_used_up_while_2fa_is_off_or_pending() {
        let conn = memory_connection();
        let alice = insert_user(&conn, "alice");
        add_recovery_code(&conn, alice, "hash-a");
        assert_eq!(
            outcome(&use_proof(&conn, alice, &recovery("hash-a"))),
            "not enabled"
        );

        conn.execute(
            "INSERT INTO totp (user_id, authentication_secret) VALUES (?1, 'pending')",
            [alice],
        )
        .unwrap();
        assert_eq!(
            outcome(&use_proof(&conn, alice, &step(1, "pending"))),
            "not enabled"
        );
    }

    // --- The code step at login -------------------------------------------------

    #[tokio::test]
    async fn the_right_code_turns_the_challenge_into_a_session_on_its_device() {
        let temp = TempDatabase::open();
        let (alice, app, _) = enrolled(&temp, "alice").await;
        let challenge = challenge_at(&temp, alice.user_id, "tablet", LATER);

        let tokens = finish(&temp, challenge, app_code(&app, LATER), LATER)
            .await
            .unwrap();

        assert_eq!(tokens.device_id, "tablet");
        let tablet_sessions: Vec<String> = column(
            &temp,
            "SELECT session_token_hashed FROM sessions \
             WHERE user_id = ?1 AND device_id = 'tablet'",
            alice.user_id,
        );
        assert_eq!(tablet_sessions, [hash_token(&tokens.session_token)]);
        let last_login: Vec<i64> = column(
            &temp,
            "SELECT last_login FROM users WHERE id = ?1",
            alice.user_id,
        );
        assert_eq!(last_login, [LATER as i64]);
        let challenges: Vec<i64> = column(
            &temp,
            "SELECT challenge_id FROM login_challenges WHERE user_id = ?1",
            alice.user_id,
        );
        assert!(challenges.is_empty(), "the challenge is used up");
        assert_eq!(lock_state(&temp, alice.user_id), (0, 0, None));
    }

    #[tokio::test]
    async fn a_wrong_code_is_counted_and_leaves_the_challenge_alive() {
        let temp = TempDatabase::open();
        let (alice, app, _) = enrolled(&temp, "alice").await;
        let challenge = challenge_at(&temp, alice.user_id, "tablet", LATER);
        let wrong = TypedCode::Authenticator(wrong_code(&app, LATER));

        let result = finish(&temp, challenge.clone(), wrong, LATER).await;

        assert_eq!(outcome(&result), "code");
        assert_eq!(lock_state(&temp, alice.user_id), (1, 0, Some(at(LATER))));
        let retry = finish(&temp, challenge, app_code(&app, LATER), LATER).await;
        assert_eq!(outcome(&retry), "accepted");
    }

    #[tokio::test]
    async fn an_app_code_is_accepted_once_and_only_while_it_is_the_newest() {
        let temp = TempDatabase::open();
        let (alice, app, _) = enrolled(&temp, "alice").await;
        let user = alice.user_id;

        let confirming = challenge_at(&temp, user, "tablet", NOW);
        let result = finish(&temp, confirming, app_code(&app, NOW), NOW).await;
        assert_eq!(outcome(&result), "code", "the code that switched 2FA on");

        let first = challenge_at(&temp, user, "tablet", LATER);
        let result = finish(&temp, first, app_code(&app, LATER), LATER).await;
        assert_eq!(outcome(&result), "accepted");

        let second = challenge_at(&temp, user, "laptop", LATER);
        let result = finish(&temp, second, app_code(&app, LATER), LATER).await;
        assert_eq!(outcome(&result), "code", "the same code again");

        let third = challenge_at(&temp, user, "laptop", LATER);
        let result = finish(&temp, third, app_code(&app, LATER - 30), LATER).await;
        assert_eq!(
            outcome(&result),
            "code",
            "an older step, though within the drift"
        );
    }

    #[tokio::test]
    async fn an_expired_or_used_challenge_costs_no_try() {
        let temp = TempDatabase::open();
        let (alice, app, _) = enrolled(&temp, "alice").await;
        let challenge = challenge_at(&temp, alice.user_id, "tablet", LATER);

        let expired = LATER + CHALLENGE_LIFETIME.as_secs();
        let late = finish(&temp, challenge.clone(), app_code(&app, expired), expired).await;
        assert_eq!(outcome(&late), "unauthorized");
        assert_eq!(lock_state(&temp, alice.user_id), (0, 0, None));

        finish(&temp, challenge.clone(), app_code(&app, LATER), LATER)
            .await
            .unwrap();
        let again = finish(&temp, challenge, app_code(&app, LATER + 30), LATER + 30).await;
        assert_eq!(outcome(&again), "unauthorized");
        assert_eq!(lock_state(&temp, alice.user_id), (0, 0, None));
    }

    #[tokio::test]
    async fn five_wrong_codes_lock_and_every_further_lock_doubles() {
        let temp = TempDatabase::open();
        let (alice, app, _) = enrolled(&temp, "alice").await;
        let user = alice.user_id;
        // Every try gets a fresh challenge: a lock outlasts the five minutes of one.
        let try_at = |secs: u64, right: bool| {
            let code = if right {
                code_at(&app, secs)
            } else {
                wrong_code(&app, secs)
            };
            let challenge = challenge_at(&temp, user, "tablet", secs);
            finish(&temp, challenge, TypedCode::Authenticator(code), secs)
        };

        for _ in 0..5 {
            assert_eq!(outcome(&try_at(LATER, false).await), "code");
        }
        assert_eq!(
            outcome(&try_at(LATER, true).await),
            "locked",
            "even the right code"
        );
        assert!(
            matches!(
                try_at(LATER + 60, true).await,
                Err(AppErrors::Authentication(AuthenticationError::TooManyAttempts { retry_after }))
                    if retry_after == Duration::from_secs(14 * 60)
            ),
            "the refusal says how long the lock still lasts"
        );
        assert_eq!(
            lock_state(&temp, user),
            (5, 1, Some(at(LATER))),
            "a refused try counts nothing"
        );

        let second_round = LATER + 15 * 60;
        assert_eq!(outcome(&try_at(second_round - 1, true).await), "locked");
        for _ in 0..5 {
            assert_eq!(outcome(&try_at(second_round, false).await), "code");
        }
        let second_lock_ends = second_round + 30 * 60;
        assert_eq!(
            outcome(&try_at(second_lock_ends - 1, true).await),
            "locked",
            "the second lock lasts 30 minutes"
        );
        assert_eq!(outcome(&try_at(second_lock_ends, true).await), "accepted");
        assert_eq!(
            lock_state(&temp, user),
            (0, 0, None),
            "a right code lifts it"
        );

        let third_round = second_lock_ends + 60;
        for _ in 0..5 {
            assert_eq!(outcome(&try_at(third_round, false).await), "code");
        }
        assert_eq!(
            outcome(&try_at(third_round + 15 * 60 - 1, true).await),
            "locked",
            "after a right code, the locks start again at 15 minutes"
        );
        assert_eq!(
            outcome(&try_at(third_round + 15 * 60, true).await),
            "accepted"
        );
    }

    #[tokio::test]
    async fn parallel_wrong_codes_cannot_slip_past_the_lock() {
        let temp = TempDatabase::open();
        let (alice, app, _) = enrolled(&temp, "alice").await;
        let challenge = challenge_at(&temp, alice.user_id, "tablet", LATER);
        let wrong = wrong_code(&app, LATER);

        let results = join_all((0..8).map(|_| {
            let code = TypedCode::Authenticator(wrong.clone());
            finish(&temp, challenge.clone(), code, LATER)
        }))
        .await;

        let mut outcomes: Vec<&str> = results.iter().map(outcome).collect();
        outcomes.sort_unstable();
        assert_eq!(outcomes, [vec!["code"; 5], vec!["locked"; 3]].concat());
    }

    #[tokio::test]
    async fn of_two_parallel_right_codes_exactly_one_logs_in() {
        let temp = TempDatabase::open();
        let (alice, app, _) = enrolled(&temp, "alice").await;
        let tablet = challenge_at(&temp, alice.user_id, "tablet", LATER);
        let laptop = challenge_at(&temp, alice.user_id, "laptop", LATER);

        let (a, b) = tokio::join!(
            finish(&temp, tablet, app_code(&app, LATER), LATER),
            finish(&temp, laptop, app_code(&app, LATER), LATER),
        );

        let mut outcomes = [outcome(&a), outcome(&b)];
        outcomes.sort_unstable();
        assert_eq!(outcomes, ["accepted", "code"]);
        assert_eq!(
            sessions_of(&temp, &alice).len(),
            2,
            "the session 2FA was set up from, and one new one"
        );
    }

    #[tokio::test]
    async fn a_recovery_code_and_an_app_code_in_parallel_log_in_once() {
        let temp = TempDatabase::open();
        let (alice, app, codes) = enrolled(&temp, "alice").await;
        let challenge = challenge_at(&temp, alice.user_id, "tablet", LATER);
        let recovery_code = TypedCode::Recovery(codes[0].clone());

        let (by_app, by_recovery) = tokio::join!(
            finish(&temp, challenge.clone(), app_code(&app, LATER), LATER),
            finish(&temp, challenge, recovery_code, LATER),
        );

        let mut outcomes = [outcome(&by_app), outcome(&by_recovery)];
        outcomes.sort_unstable();
        assert_eq!(outcomes, ["accepted", "unauthorized"]);
        let expected_left = if by_recovery.is_ok() { 9 } else { 10 };
        assert_eq!(
            codes_left(&temp, alice.user_id),
            expected_left,
            "a recovery code that lost stays unused"
        );
    }

    // --- Recovery codes at login ------------------------------------------------

    #[tokio::test]
    async fn a_recovery_code_logs_in_once_however_it_is_typed() {
        let temp = TempDatabase::open();
        let (alice, _, codes) = enrolled(&temp, "alice").await;
        let typed = codes[0].to_lowercase().replace('-', " ").replace('0', "o");

        let first = challenge_at(&temp, alice.user_id, "tablet", LATER);
        let result = finish(&temp, first, TypedCode::Recovery(typed.clone()), LATER).await;
        assert_eq!(outcome(&result), "accepted");
        assert_eq!(codes_left(&temp, alice.user_id), 9);

        let second = challenge_at(&temp, alice.user_id, "tablet", LATER);
        let again = finish(&temp, second, TypedCode::Recovery(typed), LATER).await;
        assert_eq!(outcome(&again), "code");
    }

    #[tokio::test]
    async fn another_accounts_recovery_code_is_refused_and_stays_unused() {
        let temp = TempDatabase::open();
        let (alice, _, _) = enrolled(&temp, "alice").await;
        let (bob, _, bobs_codes) = enrolled(&temp, "bob").await;
        let challenge = challenge_at(&temp, alice.user_id, "tablet", LATER);

        let result = finish(
            &temp,
            challenge,
            TypedCode::Recovery(bobs_codes[0].clone()),
            LATER,
        )
        .await;

        assert_eq!(outcome(&result), "code");
        assert_eq!(codes_left(&temp, bob.user_id), 10);
    }

    #[tokio::test]
    async fn wrong_recovery_codes_and_wrong_app_codes_share_one_lock() {
        let temp = TempDatabase::open();
        let (alice, app, codes) = enrolled(&temp, "alice").await;
        let challenge = challenge_at(&temp, alice.user_id, "tablet", LATER);
        let attempt = |code: TypedCode| finish(&temp, challenge.clone(), code, LATER);

        for _ in 0..3 {
            let wrong = TypedCode::Authenticator(wrong_code(&app, LATER));
            assert_eq!(outcome(&attempt(wrong).await), "code");
        }
        for _ in 0..2 {
            let wrong = TypedCode::Recovery("AAAA-BBBB-CCCC-DDDD".to_string());
            assert_eq!(outcome(&attempt(wrong).await), "code");
        }

        assert_eq!(outcome(&attempt(app_code(&app, LATER)).await), "locked");
        let right_recovery_code = TypedCode::Recovery(codes[0].clone());
        assert_eq!(outcome(&attempt(right_recovery_code).await), "locked");
    }

    // --- Turning 2FA off --------------------------------------------------------

    #[tokio::test]
    async fn turning_2fa_off_needs_the_password_and_a_code() {
        let temp = TempDatabase::open();
        let (alice, app, _) = enrolled(&temp, "alice").await;

        let right_code = code_at(&app, LATER);
        let wrong_password =
            turn_off(&temp, &alice, "wrong horse battery", &right_code, LATER).await;
        assert_eq!(outcome(&wrong_password), "password");
        assert_eq!(
            lock_state(&temp, alice.user_id),
            (0, 0, None),
            "no code try is spent"
        );

        let wrong = turn_off(&temp, &alice, PASSWORD, &wrong_code(&app, LATER), LATER).await;
        assert_eq!(outcome(&wrong), "code");
        assert_eq!(lock_state(&temp, alice.user_id).0, 1, "counted");

        assert!(stored_setup(&temp, alice.user_id).unwrap().confirmed);
        assert_eq!(stored_code_hashes(&temp, &alice).len(), 10);
        assert_eq!(sessions_of(&temp, &alice), vec![alice.session_id]);
    }

    #[tokio::test]
    async fn turning_2fa_off_deletes_it_and_ends_every_session_and_pending_login() {
        let temp = TempDatabase::open();
        let (alice, app, _) = enrolled(&temp, "alice").await;
        let user = alice.user_id;
        let pending_login = challenge_at(&temp, user, "tablet", LATER);
        let right = code_at(&app, LATER);
        let typed_with_a_space = format!("{} {}", &right[..3], &right[3..]);

        turn_off(&temp, &alice, PASSWORD, &typed_with_a_space, LATER)
            .await
            .unwrap();

        assert!(stored_setup(&temp, user).is_none());
        assert!(stored_code_hashes(&temp, &alice).is_empty());
        assert!(
            sessions_of(&temp, &alice).is_empty(),
            "this session ends too"
        );
        let late = finish(&temp, pending_login, app_code(&app, LATER + 30), LATER + 30).await;
        assert_eq!(outcome(&late), "unauthorized");
        let next_login = temp
            .db
            .setup_new_transaction(move |conn| {
                start_login(conn, user, device("tablet"), at(LATER + 60))
            })
            .unwrap();
        assert!(
            matches!(next_login, LoginResponse::Ok(_)),
            "no code step any more"
        );
    }

    #[tokio::test]
    async fn turning_2fa_off_works_with_a_recovery_code_too() {
        let temp = TempDatabase::open();
        let (alice, _, codes) = enrolled(&temp, "alice").await;

        turn_off(&temp, &alice, PASSWORD, &codes[3], LATER)
            .await
            .unwrap();

        assert!(stored_setup(&temp, alice.user_id).is_none());
        assert!(stored_code_hashes(&temp, &alice).is_empty());
        assert!(sessions_of(&temp, &alice).is_empty());
    }

    #[tokio::test]
    async fn turning_2fa_off_while_it_is_off_or_pending_is_refused() {
        let temp = TempDatabase::open();
        let alice = alice(&temp).await;

        let off = turn_off(&temp, &alice, PASSWORD, "123456", LATER).await;
        assert_eq!(outcome(&off), "not enabled");

        begin(&temp, &alice).await.unwrap();
        let pending = turn_off(&temp, &alice, PASSWORD, "123456", LATER).await;
        assert_eq!(
            outcome(&pending),
            "not enabled",
            "a pending setup isn't on yet"
        );
        assert!(stored_setup(&temp, alice.user_id).is_some(), "and stays");
    }

    #[tokio::test]
    async fn of_two_parallel_disables_one_wins() {
        let temp = TempDatabase::open();
        let (alice, app, _) = enrolled(&temp, "alice").await;
        let code = code_at(&app, LATER);

        let (a, b) = tokio::join!(
            turn_off(&temp, &alice, PASSWORD, &code, LATER),
            turn_off(&temp, &alice, PASSWORD, &code, LATER),
        );

        let mut outcomes = [outcome(&a), outcome(&b)];
        outcomes.sort_unstable();
        assert_eq!(outcomes, ["accepted", "not enabled"]);
    }

    // --- Recovery code format ---------------------------------------------------

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
