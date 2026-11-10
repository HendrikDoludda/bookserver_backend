//! Logging in, refreshing a session's tokens, and logging out.

use std::{
    sync::Arc,
    time::{Duration, SystemTime},
};

use rusqlite::Connection;
use uuid::Uuid;

use crate::{
    data_models::authentication_model::{
        DeviceInformation, LoginRequest, LoginResponse, SessionTokens,
    },
    database_related_scripts::{
        db::convert_system_time_to_unix_time, db_login_challenges, db_login_throttle,
    },
    db::Database,
    error_types::{AppErrors, AuthenticationError, DatabaseError},
    models::{
        LoginChallenge, QuerySeparator, Sessions, SessionsDatabaseColumns, TOTPDatabaseColumns,
        UserDatabaseColumns, UserMetadata, TOTP,
    },
    routes::auth::{
        create_new_session, create_new_token, create_token_pair, hash_token, secret_matches,
        CreatedTokensReadable,
    },
};

pub const CHALLENGE_LIFETIME: Duration = Duration::from_secs(5 * 60);
const MAX_LOGIN_TRIES: i64 = 10;
const LOGIN_TRY_WINDOW: Duration = Duration::from_secs(15 * 60);

/// Checks the password and starts a session — or, for a TOTP user, a challenge that the code
/// step turns into one. A wrong password and an unknown name get the same 401 in the same
/// time; the 403 for an unverified email comes only after a correct password.
pub async fn log_in(
    db: Arc<Database>,
    request: LoginRequest,
    now: SystemTime,
) -> Result<LoginResponse, AppErrors> {
    let LoginRequest {
        username_or_email,
        password,
        device_name,
        device_id,
        platform,
    } = request;
    let CheckedPassword { user, throttle_key } =
        check_password(Arc::clone(&db), &username_or_email, password, now).await?;
    let device_id = device_id_or_new(device_id);

    let response = db
        .transact(move |conn| {
            // The password was right, so the tries are forgotten even for an unverified email.
            db_login_throttle::clear(conn, &throttle_key)?;
            if !user.email_verified {
                return Ok(None);
            }
            let device = DeviceInformation {
                device_id: &device_id,
                device_name: &device_name,
                platform: &platform,
            };
            Ok(Some(start_login(conn, user.user_id, device, now)?))
        })
        .await?;
    response.ok_or_else(|| AuthenticationError::EmailNotVerified.into())
}

/// A name and password that matched an account.
pub(crate) struct CheckedPassword {
    pub user: UserMetadata,
    /// The name's key in the login throttle, for clearing its tries.
    pub throttle_key: String,
}

/// Checks a name and password the way login does: the try counts against the name first, and
/// a wrong password and an unknown name get the same 401 in the same time.
pub(crate) async fn check_password(
    db: Arc<Database>,
    name: &str,
    password: String,
    now: SystemTime,
) -> Result<CheckedPassword, AppErrors> {
    let name = name.trim().to_string();
    let throttle_key = name.to_ascii_lowercase();

    // Counted before the password is checked, and for names without an account too: counted
    // after a failed check it could be rolled back, and a 429 must not reveal which names exist.
    let key = throttle_key.clone();
    let user = db
        .transact(move |conn| {
            if let Some(retry_after) =
                db_login_throttle::reserve(conn, &key, now, LOGIN_TRY_WINDOW, MAX_LOGIN_TRIES)?
            {
                return Err(AuthenticationError::TooManyAttempts { retry_after }.into());
            }
            Ok(find_user_for_login(conn, &name)?)
        })
        .await?;

    // Without an account this checks against a dummy hash, so both failures take as long.
    let stored_hash = user.as_ref().map(|user| user.password_hash.clone());
    let password_is_right = secret_matches(password, stored_hash).await?;
    match user {
        Some(user) if password_is_right => Ok(CheckedPassword { user, throttle_key }),
        _ => Err(AuthenticationError::IncorrectCredentials.into()),
    }
}

/// The current password, re-checked by an account endpoint: login's check, so it shares login's
/// throttle, but a wrong password answers 400 `incorrect_password` — the app reads a 401 as an
/// ended session.
pub(crate) async fn check_current_password(
    db: Arc<Database>,
    user_id: i64,
    password: String,
    now: SystemTime,
) -> Result<(), AppErrors> {
    let username = Arc::clone(&db)
        .read(move |conn| {
            Ok(Database::get_entry_with_connection::<UserMetadata>(conn, user_id)?.username)
        })
        .await?;
    let CheckedPassword { user, throttle_key } =
        match check_password(Arc::clone(&db), &username, password, now).await {
            Err(AppErrors::Authentication(AuthenticationError::IncorrectCredentials)) => {
                return Err(AuthenticationError::IncorrectPassword.into());
            }
            checked => checked?,
        };
    // Looked up by name, so make sure the name still belongs to this account.
    if user.user_id != user_id {
        return Err(AuthenticationError::IncorrectPassword.into());
    }
    // The right password was counted as a try too; left in the count, repeated attempts with a
    // wrong code would lock the user's login.
    db.transact(move |conn| Ok(db_login_throttle::clear(conn, &throttle_key)?))
        .await
}

/// What a proven identity leads to — a correct password, or a redeemed verification code:
/// a session, or for a TOTP user a challenge that the code step turns into one.
pub(crate) fn start_login(
    conn: &Connection,
    user_id: i64,
    device: DeviceInformation,
    now: SystemTime,
) -> Result<LoginResponse, AppErrors> {
    if has_totp(conn, user_id)? {
        return start_challenge(conn, user_id, &device, now);
    }
    Ok(LoginResponse::Ok(open_session(conn, user_id, device, now)?))
}

/// Starts a session on the device and records the login — the end of every login, with or
/// without the TOTP code step.
pub(crate) fn open_session(
    conn: &Connection,
    user_id: i64,
    device: DeviceInformation,
    now: SystemTime,
) -> Result<SessionTokens, AppErrors> {
    let device_id = device.device_id.to_string();
    let tokens = create_new_session(conn, user_id, device, now)?;
    Database::update_value_with_connection::<UserMetadata>(
        conn,
        UserDatabaseColumns::LastLogin,
        &convert_system_time_to_unix_time(now),
        user_id,
    )?;
    Ok(session_tokens(tokens, device_id))
}

/// The device id the client sent back, or a new one: the server hands out device ids, so
/// anything that isn't a UUID is replaced.
pub(crate) fn device_id_or_new(device_id: Option<String>) -> String {
    device_id
        .filter(|id| Uuid::parse_str(id).is_ok())
        .unwrap_or_else(|| Uuid::new_v4().to_string())
}

/// Input with an `@` is looked up as an email, anything else as a username. A single
/// `email = ? OR username = ?` lookup lets a username that equals another account's email
/// hide that account.
pub(crate) fn find_user_for_login(
    conn: &Connection,
    name: &str,
) -> Result<Option<UserMetadata>, DatabaseError> {
    let column = if name.contains('@') {
        UserDatabaseColumns::Email
    } else {
        UserDatabaseColumns::Username
    };
    Database::search_for_single_row_with_connection::<UserMetadata>(
        conn,
        &[column],
        &[&name],
        QuerySeparator::And,
    )
}

/// Whether the user has 2FA on. A setup still waiting for its first code doesn't count — it
/// would demand a code the user can't produce yet.
pub(crate) fn has_totp(conn: &Connection, user_id: i64) -> Result<bool, DatabaseError> {
    let totp = Database::search_for_single_row_with_connection::<TOTP>(
        conn,
        &[TOTPDatabaseColumns::UserId, TOTPDatabaseColumns::Confirmed],
        &[&user_id, &true],
        QuerySeparator::And,
    )?;
    Ok(totp.is_some())
}

fn start_challenge(
    conn: &Connection,
    user_id: i64,
    device: &DeviceInformation,
    now: SystemTime,
) -> Result<LoginResponse, AppErrors> {
    let token = create_new_token()?;
    let expires_at = now + CHALLENGE_LIFETIME;
    db_login_challenges::create(
        conn,
        &LoginChallenge {
            challenge_id: 0,
            user_id,
            token_hashed: token.hashed_string,
            expires_at,
            device_id: device.device_id.to_string(),
            device_name: device.device_name.to_string(),
            platform: device.platform.to_string(),
        },
        now,
    )?;
    Ok(LoginResponse::TotpRequired {
        challenge_token: token.normal_string,
        challenge_expires_at: convert_system_time_to_unix_time(expires_at),
    })
}

fn session_tokens(tokens: CreatedTokensReadable, device_id: String) -> SessionTokens {
    SessionTokens {
        session_token: tokens.session_token,
        session_expires_at: convert_system_time_to_unix_time(tokens.session_token_valid_until),
        refresh_token: tokens.refresh_token,
        refresh_expires_at: convert_system_time_to_unix_time(tokens.refresh_token_valid_until),
        device_id,
    }
}

/// Gives the session behind a refresh token new tokens. Of two parallel refreshes with the
/// same token only the first succeeds; the second gets 401.
pub async fn refresh(
    db: Arc<Database>,
    session: Sessions,
    now: SystemTime,
) -> Result<SessionTokens, AppErrors> {
    db.transact(move |conn| {
        // Read again inside the write lock: a parallel refresh with the same token may have
        // replaced it since the extractor's lookup.
        let current = Database::search_for_single_row_with_connection::<Sessions>(
            conn,
            &[SessionsDatabaseColumns::SessionId],
            &[&session.session_id],
            QuerySeparator::And,
        )?;
        if current.is_none_or(|current| current.refresh_token != session.refresh_token) {
            return Err(AuthenticationError::UnauthorizedSession.into());
        }
        let tokens = create_token_pair(now)?;
        Database::update_multiple_values_with_connection::<Sessions>(
            conn,
            &[
                SessionsDatabaseColumns::SessionToken,
                SessionsDatabaseColumns::SessionTokenExpirationDate,
                SessionsDatabaseColumns::RefreshToken,
                SessionsDatabaseColumns::RefreshTokenExpirationDate,
                SessionsDatabaseColumns::LastUsedAt,
            ],
            &[
                &tokens.session_token_hashed,
                &convert_system_time_to_unix_time(tokens.session_token_valid_until),
                &tokens.refresh_token_hashed,
                &convert_system_time_to_unix_time(tokens.refresh_token_valid_until),
                &convert_system_time_to_unix_time(now),
            ],
            session.session_id,
        )?;
        Ok(SessionTokens {
            session_token: tokens.session_token,
            session_expires_at: convert_system_time_to_unix_time(tokens.session_token_valid_until),
            refresh_token: tokens.refresh_token,
            refresh_expires_at: convert_system_time_to_unix_time(tokens.refresh_token_valid_until),
            device_id: session.device_id,
        })
    })
    .await
}

/// Ends the session behind `token` — its session or its refresh token, expired or not.
/// Finding nothing is fine: the client is logged out either way.
pub async fn end_session(db: Arc<Database>, token: Option<String>) -> Result<(), AppErrors> {
    let Some(token) = token else {
        return Ok(());
    };
    let hash = hash_token(&token);
    db.transact(move |conn| {
        Database::remove_entry_with_connection_based_on_columns::<Sessions>(
            conn,
            &[
                SessionsDatabaseColumns::SessionToken,
                SessionsDatabaseColumns::RefreshToken,
            ],
            &[&hash, &hash],
            QuerySeparator::Or,
        )?;
        Ok(())
    })
    .await
}

#[cfg(test)]
mod tests {
    use rusqlite::params;

    use super::*;
    use crate::{routes::auth::hash_secret, test_support::TempDatabase};

    const PASSWORD: &str = "correct horse battery";

    async fn add_user(
        temp: &TempDatabase,
        username: &str,
        email: &str,
        password: &str,
        verified: bool,
    ) -> i64 {
        let hash = hash_secret(password.to_string()).await.unwrap();
        let (username, email) = (username.to_string(), email.to_string());
        temp.db
            .setup_new_transaction(move |conn| {
                conn.execute(
                    "INSERT INTO users (username, password_hash, email, email_verified) \
                     VALUES (?1, ?2, ?3, ?4)",
                    params![username, hash, email, verified],
                )
                .unwrap();
                Ok(conn.last_insert_rowid())
            })
            .unwrap()
    }

    async fn alice(temp: &TempDatabase) -> i64 {
        add_user(temp, "alice", "alice@example.com", PASSWORD, true).await
    }

    async fn login(
        temp: &TempDatabase,
        name: &str,
        password: &str,
        device_id: Option<&str>,
        now: SystemTime,
    ) -> Result<LoginResponse, AppErrors> {
        let request = LoginRequest {
            username_or_email: name.to_string(),
            password: password.to_string(),
            device_name: "phone".to_string(),
            device_id: device_id.map(str::to_string),
            platform: "ios".to_string(),
        };
        log_in(Arc::clone(&temp.db), request, now).await
    }

    fn tokens(result: Result<LoginResponse, AppErrors>) -> SessionTokens {
        match result {
            Ok(LoginResponse::Ok(tokens)) => tokens,
            Ok(LoginResponse::TotpRequired { .. }) => panic!("expected a session, got a challenge"),
            Err(err) => panic!("expected a session, got: {err}"),
        }
    }

    fn rejected_with(result: &Result<LoginResponse, AppErrors>) -> &'static str {
        match result {
            Err(AppErrors::Authentication(AuthenticationError::IncorrectCredentials)) => {
                "incorrect"
            }
            Err(AppErrors::Authentication(AuthenticationError::TooManyAttempts { .. })) => {
                "too many"
            }
            Err(AppErrors::Authentication(AuthenticationError::EmailNotVerified)) => "unverified",
            Err(_) => "other error",
            Ok(_) => "accepted",
        }
    }

    fn query<T: rusqlite::types::FromSql + Send + 'static>(
        temp: &TempDatabase,
        sql: &'static str,
        param: impl rusqlite::ToSql + Send + 'static,
    ) -> T {
        temp.db
            .setup_new_transaction(move |conn| {
                Ok(conn.query_row(sql, [param], |row| row.get(0)).unwrap())
            })
            .unwrap()
    }

    fn session_exists(temp: &TempDatabase, session_token: &str) -> bool {
        let count: i64 = query(
            temp,
            "SELECT COUNT(*) FROM sessions WHERE session_token_hashed = ?1",
            hash_token(session_token),
        );
        count == 1
    }

    fn sessions_of(temp: &TempDatabase, user_id: i64) -> i64 {
        query(
            temp,
            "SELECT COUNT(*) FROM sessions WHERE user_id = ?1",
            user_id,
        )
    }

    fn session_row(temp: &TempDatabase, session_token: &str) -> Sessions {
        let hash = hash_token(session_token);
        temp.db
            .setup_new_transaction(move |conn| {
                Ok(Database::search_for_single_row_with_connection::<Sessions>(
                    conn,
                    &[crate::models::SessionsDatabaseColumns::SessionToken],
                    &[&hash],
                    QuerySeparator::And,
                )?
                .expect("the session exists"))
            })
            .unwrap()
    }

    #[tokio::test]
    async fn a_correct_password_starts_a_session() {
        let temp = TempDatabase::open();
        let alice = alice(&temp).await;
        let now = SystemTime::now();

        let tokens = tokens(login(&temp, "alice", PASSWORD, None, now).await);

        assert!(Uuid::parse_str(&tokens.device_id).is_ok());
        assert!(session_exists(&temp, &tokens.session_token));
        let unix_now = convert_system_time_to_unix_time(now);
        assert_eq!(tokens.session_expires_at, unix_now + 15 * 60);
        assert_eq!(tokens.refresh_expires_at, unix_now + 30 * 24 * 60 * 60);
        let last_login: i64 = query(&temp, "SELECT last_login FROM users WHERE id = ?1", alice);
        assert_eq!(last_login, unix_now);
    }

    #[tokio::test]
    async fn a_wrong_password_and_an_unknown_name_get_the_same_answer() {
        let temp = TempDatabase::open();
        let alice = alice(&temp).await;
        let now = SystemTime::now();

        let wrong_password = login(&temp, "alice", "wrong horse battery", None, now).await;
        let unknown_name = login(&temp, "nobody", PASSWORD, None, now).await;

        assert_eq!(rejected_with(&wrong_password), "incorrect");
        assert_eq!(rejected_with(&unknown_name), "incorrect");
        assert_eq!(sessions_of(&temp, alice), 0);
    }

    #[tokio::test]
    async fn an_email_is_looked_up_as_an_email_even_if_it_is_someone_elses_username() {
        let temp = TempDatabase::open();
        let alice = alice(&temp).await;
        add_user(
            &temp,
            "alice@example.com",
            "mallory@example.com",
            "mallory's password",
            true,
        )
        .await;
        let now = SystemTime::now();

        let tokens = tokens(login(&temp, "ALICE@example.com", PASSWORD, None, now).await);
        assert_eq!(session_row(&temp, &tokens.session_token).user_id, alice);
    }

    #[tokio::test]
    async fn ten_failed_tries_lock_a_name_whether_or_not_it_has_an_account() {
        let temp = TempDatabase::open();
        alice(&temp).await;
        let now = SystemTime::now();

        for _ in 0..10 {
            assert_eq!(
                rejected_with(&login(&temp, "nobody", "guess", None, now).await),
                "incorrect"
            );
        }
        assert_eq!(
            rejected_with(&login(&temp, "nobody", "guess", None, now).await),
            "too many"
        );

        // `Alice` and `alice` are one name for the count.
        for name in ["alice", "Alice"].repeat(5) {
            assert_eq!(
                rejected_with(&login(&temp, name, "wrong", None, now).await),
                "incorrect"
            );
        }
        let right_but_locked = login(&temp, "alice", PASSWORD, None, now).await;
        assert_eq!(rejected_with(&right_but_locked), "too many");

        let after_the_window = now + LOGIN_TRY_WINDOW;
        tokens(login(&temp, "alice", PASSWORD, None, after_the_window).await);
    }

    #[tokio::test]
    async fn a_locked_name_says_how_long_the_lock_still_lasts() {
        let temp = TempDatabase::open();
        // Whole seconds, like the stored window start, so the time left comes out exact.
        let start = SystemTime::UNIX_EPOCH + Duration::from_secs(1_700_000_000);
        for _ in 0..MAX_LOGIN_TRIES {
            login(&temp, "nobody", "guess", None, start).await.ok();
        }

        let five_minutes = Duration::from_secs(5 * 60);
        let refused = login(&temp, "nobody", "guess", None, start + five_minutes).await;

        assert!(matches!(
            refused,
            Err(AppErrors::Authentication(AuthenticationError::TooManyAttempts { retry_after }))
                if retry_after == LOGIN_TRY_WINDOW - five_minutes
        ));
    }

    #[tokio::test]
    async fn a_successful_login_resets_the_count() {
        let temp = TempDatabase::open();
        alice(&temp).await;
        let now = SystemTime::now();

        for _ in 0..2 {
            for _ in 0..9 {
                login(&temp, "alice", "wrong", None, now).await.ok();
            }
            tokens(login(&temp, "alice", PASSWORD, None, now).await);
        }
    }

    #[tokio::test]
    async fn an_unverified_account_is_refused_only_after_a_correct_password() {
        let temp = TempDatabase::open();
        let bob = add_user(&temp, "bob", "bob@example.com", PASSWORD, false).await;
        let now = SystemTime::now();

        assert_eq!(
            rejected_with(&login(&temp, "bob", "wrong", None, now).await),
            "incorrect"
        );
        assert_eq!(
            rejected_with(&login(&temp, "bob", PASSWORD, None, now).await),
            "unverified"
        );
        assert_eq!(sessions_of(&temp, bob), 0);
    }

    fn add_totp(temp: &TempDatabase, user_id: i64, confirmed: bool) {
        temp.db
            .setup_new_transaction(move |conn| {
                conn.execute(
                    "INSERT INTO totp (user_id, authentication_secret, confirmed) \
                     VALUES (?1, 'secret', ?2)",
                    params![user_id, confirmed],
                )
                .unwrap();
                Ok(())
            })
            .unwrap();
    }

    #[tokio::test]
    async fn a_setup_still_waiting_for_its_code_does_not_ask_for_one() {
        let temp = TempDatabase::open();
        let alice = alice(&temp).await;
        add_totp(&temp, alice, false);

        tokens(login(&temp, "alice", PASSWORD, None, SystemTime::now()).await);
    }

    #[tokio::test]
    async fn the_current_password_is_checked_against_logins_throttle() {
        let temp = TempDatabase::open();
        let alice = alice(&temp).await;
        let now = SystemTime::now();
        let check = |password: &str| {
            check_current_password(Arc::clone(&temp.db), alice, password.to_string(), now)
        };
        let tries = || -> i64 {
            query(
                &temp,
                "SELECT COALESCE(SUM(attempts), 0) FROM login_failures WHERE name = ?1",
                "alice",
            )
        };

        assert!(matches!(
            check("wrong horse battery").await,
            Err(AppErrors::Authentication(
                AuthenticationError::IncorrectPassword
            ))
        ));
        assert_eq!(tries(), 1, "a wrong password counts like a failed login");

        check(PASSWORD).await.unwrap();
        assert_eq!(tries(), 0, "the right password clears the count");

        for _ in 0..MAX_LOGIN_TRIES {
            check("wrong horse battery").await.ok();
        }
        assert!(matches!(
            check(PASSWORD).await,
            Err(AppErrors::Authentication(
                AuthenticationError::TooManyAttempts { .. }
            ))
        ));
        assert_eq!(
            rejected_with(&login(&temp, "alice", PASSWORD, None, now).await),
            "too many",
            "the lock is login's own"
        );
    }

    #[tokio::test]
    async fn a_totp_user_gets_a_challenge_instead_of_a_session() {
        let temp = TempDatabase::open();
        let alice = alice(&temp).await;
        add_totp(&temp, alice, true);
        let now = SystemTime::now();

        let Ok(LoginResponse::TotpRequired {
            challenge_token,
            challenge_expires_at,
        }) = login(&temp, "alice", PASSWORD, None, now).await
        else {
            panic!("expected a TOTP challenge");
        };

        assert_eq!(
            challenge_expires_at,
            convert_system_time_to_unix_time(now + CHALLENGE_LIFETIME)
        );
        let stored: i64 = query(
            &temp,
            "SELECT COUNT(*) FROM login_challenges WHERE token_hashed = ?1",
            hash_token(&challenge_token),
        );
        assert_eq!(stored, 1);
        assert_eq!(sessions_of(&temp, alice), 0, "no session before the code");
    }

    #[tokio::test]
    async fn logging_in_again_on_a_device_replaces_its_session() {
        let temp = TempDatabase::open();
        let alice = alice(&temp).await;
        let now = SystemTime::now();

        let first = tokens(login(&temp, "alice", PASSWORD, None, now).await);
        let again = tokens(login(&temp, "alice", PASSWORD, Some(&first.device_id), now).await);
        assert_eq!(again.device_id, first.device_id);
        assert_eq!(sessions_of(&temp, alice), 1);
        assert!(
            !session_exists(&temp, &first.session_token),
            "the old session ended"
        );

        let other_device = tokens(login(&temp, "alice", PASSWORD, None, now).await);
        assert_ne!(other_device.device_id, first.device_id);
        assert_eq!(sessions_of(&temp, alice), 2);

        let made_up = tokens(login(&temp, "alice", PASSWORD, Some("not-a-uuid"), now).await);
        assert_ne!(
            made_up.device_id, "not-a-uuid",
            "the server hands out device ids"
        );
        assert_eq!(sessions_of(&temp, alice), 3);
    }

    #[tokio::test]
    async fn a_refresh_gives_new_tokens_and_the_old_ones_stop_working() {
        let temp = TempDatabase::open();
        alice(&temp).await;
        let now = SystemTime::now();
        let first = tokens(login(&temp, "alice", PASSWORD, None, now).await);
        let session = session_row(&temp, &first.session_token);

        let renewed = refresh(Arc::clone(&temp.db), session.clone(), now)
            .await
            .unwrap();

        assert_ne!(renewed.session_token, first.session_token);
        assert_ne!(renewed.refresh_token, first.refresh_token);
        assert_eq!(renewed.device_id, first.device_id);
        assert!(session_exists(&temp, &renewed.session_token));
        assert!(!session_exists(&temp, &first.session_token));

        let replayed = refresh(Arc::clone(&temp.db), session, now).await;
        assert!(matches!(
            replayed,
            Err(AppErrors::Authentication(
                AuthenticationError::UnauthorizedSession
            ))
        ));
    }

    #[tokio::test]
    async fn a_refresh_of_a_session_ended_since_its_lookup_answers_401() {
        let temp = TempDatabase::open();
        alice(&temp).await;
        let now = SystemTime::now();
        let first = tokens(login(&temp, "alice", PASSWORD, None, now).await);
        // What the extractor found, before a logout on another request ended the session.
        let session = session_row(&temp, &first.session_token);
        end_session(Arc::clone(&temp.db), Some(first.refresh_token))
            .await
            .unwrap();

        let result = refresh(Arc::clone(&temp.db), session, now).await;

        assert!(matches!(
            result,
            Err(AppErrors::Authentication(
                AuthenticationError::UnauthorizedSession
            ))
        ));
    }

    #[tokio::test]
    async fn of_two_parallel_refreshes_exactly_one_wins() {
        let temp = TempDatabase::open();
        alice(&temp).await;
        let now = SystemTime::now();
        let first = tokens(login(&temp, "alice", PASSWORD, None, now).await);
        let session = session_row(&temp, &first.session_token);

        let (a, b) = tokio::join!(
            refresh(Arc::clone(&temp.db), session.clone(), now),
            refresh(Arc::clone(&temp.db), session, now),
        );
        assert_eq!([a.is_ok(), b.is_ok()].iter().filter(|ok| **ok).count(), 1);
    }

    #[tokio::test]
    async fn logging_out_ends_the_session_behind_either_token() {
        let temp = TempDatabase::open();
        let alice = alice(&temp).await;
        let now = SystemTime::now();

        let first = tokens(login(&temp, "alice", PASSWORD, None, now).await);
        end_session(Arc::clone(&temp.db), Some(first.refresh_token))
            .await
            .unwrap();
        assert!(!session_exists(&temp, &first.session_token));

        let second = tokens(login(&temp, "alice", PASSWORD, None, now).await);
        end_session(Arc::clone(&temp.db), Some(second.session_token.clone()))
            .await
            .unwrap();
        assert!(!session_exists(&temp, &second.session_token));

        end_session(Arc::clone(&temp.db), Some("unknown".to_string()))
            .await
            .unwrap();
        end_session(Arc::clone(&temp.db), None).await.unwrap();
        assert_eq!(sessions_of(&temp, alice), 0);
    }
}
