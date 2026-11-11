//! Changes users make to their own account.

use std::{sync::Arc, time::SystemTime};

use rusqlite::Connection;

use crate::{
    data_models::authentication_model::ChangePasswordRequest,
    db::Database,
    error_types::{AppErrors, AuthenticationError},
    models::{UserDatabaseColumns, UserMetadata},
    routes::{
        auth::{check_new_password, end_other_sessions, hash_secret},
        login::check_current_password,
    },
};

/// Sets a new password once the current one is re-checked, and ends the user's other sessions
/// and pending logins; the session that asked stays.
pub async fn change_password(
    db: Arc<Database>,
    user_id: i64,
    current_session_id: i64,
    request: ChangePasswordRequest,
    now: SystemTime,
) -> Result<(), AppErrors> {
    let ChangePasswordRequest {
        current_password,
        new_password,
        new_password_repeat,
    } = request;
    check_new_password(&new_password, &new_password_repeat)?;
    // A plain compare is enough: the current password is verified next, so equal strings can
    // only mean a change to the same password.
    if new_password == current_password {
        return Err(AuthenticationError::PasswordUnchanged.into());
    }
    let checked = check_current_password(Arc::clone(&db), user_id, current_password, now).await?;
    let new_hash = hash_secret(new_password).await?;

    db.transact(move |conn| {
        save_new_password(
            conn,
            user_id,
            current_session_id,
            &checked.password_hash,
            &new_hash,
        )
    })
    .await
}

/// The final write of a password change. The stored hash must still be the one the current
/// password was checked against: a reset or a parallel change since then made it wrong.
fn save_new_password(
    conn: &Connection,
    user_id: i64,
    current_session_id: i64,
    checked_hash: &str,
    new_hash: &str,
) -> Result<(), AppErrors> {
    let stored = Database::get_entry_with_connection::<UserMetadata>(conn, user_id)?;
    if stored.password_hash != checked_hash {
        return Err(AuthenticationError::IncorrectPassword.into());
    }
    Database::update_value_with_connection::<UserMetadata>(
        conn,
        UserDatabaseColumns::PasswordHash,
        &new_hash,
        user_id,
    )?;
    end_other_sessions(conn, user_id, current_session_id)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use rusqlite::params;

    use super::*;
    use crate::{
        data_models::authentication_model::{LoginRequest, LoginResponse},
        db::convert_system_time_to_unix_time,
        routes::login::log_in,
        test_support::{insert_user, memory_connection, TempDatabase},
    };

    const OLD: &str = "correct horse battery";
    const NEW: &str = "battery staple horse";
    const OTHER: &str = "staple horse battery";

    async fn add_user(temp: &TempDatabase, name: &str) -> i64 {
        let hash = hash_secret(OLD.to_string()).await.unwrap();
        let name = name.to_string();
        temp.db
            .setup_new_transaction(move |conn| {
                conn.execute(
                    "INSERT INTO users (username, password_hash, email, email_verified) \
                     VALUES (?1, ?2, ?1 || '@example.com', 1)",
                    params![name, hash],
                )
                .unwrap();
                Ok(conn.last_insert_rowid())
            })
            .unwrap()
    }

    /// A live session on `device`, which names it and its token hashes; answers its id.
    fn add_session(temp: &TempDatabase, user_id: i64, device: &str, now: SystemTime) -> i64 {
        let device = device.to_string();
        let valid_until = convert_system_time_to_unix_time(now) + 30 * 24 * 60 * 60;
        temp.db
            .setup_new_transaction(move |conn| {
                conn.execute(
                    "INSERT INTO sessions (user_id, device_id, device_name, platform, \
                     refresh_token_hashed, refresh_token_valid_until, session_token_hashed, \
                     session_token_valid_until, last_used_at) \
                     VALUES (?1, ?2, ?2, 'ios', ?2 || '-refresh', ?3, ?2 || '-session', ?3, 0)",
                    params![user_id, device, valid_until],
                )
                .unwrap();
                Ok(conn.last_insert_rowid())
            })
            .unwrap()
    }

    /// A login that passed the password and waits for its TOTP code.
    fn add_pending_login(temp: &TempDatabase, user_id: i64, now: SystemTime) {
        let expires_at = convert_system_time_to_unix_time(now) + 5 * 60;
        temp.db
            .setup_new_transaction(move |conn| {
                conn.execute(
                    "INSERT INTO login_challenges (user_id, token_hashed, expires_at, device_id, \
                     device_name, platform) VALUES (?1, 'challenge', ?2, 'watch', 'watch', 'ios')",
                    params![user_id, expires_at],
                )
                .unwrap();
                Ok(())
            })
            .unwrap();
    }

    fn count(temp: &TempDatabase, sql: &'static str) -> i64 {
        temp.db
            .setup_new_transaction(move |conn| {
                Ok(conn.query_row(sql, [], |row| row.get(0)).unwrap())
            })
            .unwrap()
    }

    fn session_ids(temp: &TempDatabase) -> Vec<i64> {
        temp.db
            .setup_new_transaction(|conn| {
                let ids = conn
                    .prepare("SELECT session_id FROM sessions ORDER BY session_id")
                    .unwrap()
                    .query_map([], |row| row.get(0))
                    .unwrap()
                    .collect::<Result<Vec<i64>, _>>()
                    .unwrap();
                Ok(ids)
            })
            .unwrap()
    }

    async fn change(
        temp: &TempDatabase,
        user_id: i64,
        session_id: i64,
        current: &str,
        new: &str,
        repeat: &str,
        now: SystemTime,
    ) -> Result<(), AppErrors> {
        let request = ChangePasswordRequest {
            current_password: current.to_string(),
            new_password: new.to_string(),
            new_password_repeat: repeat.to_string(),
        };
        change_password(Arc::clone(&temp.db), user_id, session_id, request, now).await
    }

    async fn login(temp: &TempDatabase, password: &str, now: SystemTime) -> &'static str {
        let request = LoginRequest {
            username_or_email: "alice".to_string(),
            password: password.to_string(),
            device_name: "laptop".to_string(),
            device_id: None,
            platform: "macos".to_string(),
        };
        match log_in(Arc::clone(&temp.db), request, now).await {
            Ok(LoginResponse::Ok(_)) => "accepted",
            Err(AppErrors::Authentication(AuthenticationError::IncorrectCredentials)) => {
                "incorrect"
            }
            _ => "other",
        }
    }

    fn outcome(result: &Result<(), AppErrors>) -> &'static str {
        match result {
            Ok(()) => "accepted",
            Err(AppErrors::Authentication(err)) => match err {
                AuthenticationError::IncorrectPassword => "incorrect password",
                AuthenticationError::PasswordUnchanged => "unchanged",
                AuthenticationError::MismatchingPasswords => "mismatch",
                AuthenticationError::PasswordTooShort => "too short",
                _ => "other auth error",
            },
            Err(_) => "other error",
        }
    }

    #[tokio::test]
    async fn the_new_password_replaces_the_old_one_and_only_the_asking_session_stays() {
        let temp = TempDatabase::open();
        let now = SystemTime::now();
        let alice = add_user(&temp, "alice").await;
        let bob = add_user(&temp, "bob").await;
        let asking = add_session(&temp, alice, "alice-phone", now);
        add_session(&temp, alice, "alice-tablet", now);
        let bobs = add_session(&temp, bob, "bob-laptop", now);
        add_pending_login(&temp, alice, now);

        change(&temp, alice, asking, OLD, NEW, NEW, now)
            .await
            .unwrap();

        assert_eq!(session_ids(&temp), vec![asking, bobs]);
        assert_eq!(
            count(&temp, "SELECT COUNT(*) FROM login_challenges"),
            0,
            "a login waiting for its TOTP code ends too"
        );
        assert_eq!(login(&temp, OLD, now).await, "incorrect");
        assert_eq!(login(&temp, NEW, now).await, "accepted");
    }

    fn alices_failed_tries(temp: &TempDatabase) -> i64 {
        count(
            temp,
            "SELECT COALESCE(SUM(attempts), 0) FROM login_failures WHERE name = 'alice'",
        )
    }

    #[tokio::test]
    async fn a_wrong_current_password_is_refused_and_counts_on_logins_throttle() {
        let temp = TempDatabase::open();
        let now = SystemTime::now();
        let alice = add_user(&temp, "alice").await;
        let session = add_session(&temp, alice, "alice-phone", now);

        let result = change(&temp, alice, session, "wrong horse battery", NEW, NEW, now).await;

        assert_eq!(outcome(&result), "incorrect password");
        assert_eq!(alices_failed_tries(&temp), 1);
        assert_eq!(login(&temp, OLD, now).await, "accepted", "nothing changed");
    }

    #[tokio::test]
    async fn an_unchanged_mismatched_or_short_password_is_refused_before_any_check() {
        let temp = TempDatabase::open();
        let now = SystemTime::now();
        let alice = add_user(&temp, "alice").await;
        let session = add_session(&temp, alice, "alice-phone", now);
        // A check of the right password would clear these; untouched, they prove none ran.
        for _ in 0..3 {
            assert_eq!(login(&temp, "wrong horse battery", now).await, "incorrect");
        }

        for (new, repeat, expected) in [
            (OLD, OLD, "unchanged"),
            (NEW, OTHER, "mismatch"),
            ("too short", "too short", "too short"),
        ] {
            let result = change(&temp, alice, session, OLD, new, repeat, now).await;
            assert_eq!(outcome(&result), expected, "{new:?}");
        }
        assert_eq!(alices_failed_tries(&temp), 3, "no password was checked");
        assert_eq!(login(&temp, OLD, now).await, "accepted");
    }

    #[test]
    fn a_hash_changed_since_the_check_is_refused_at_the_final_write() {
        let conn = memory_connection();
        let alice = insert_user(&conn, "alice");
        let stored_hash = || -> String {
            conn.query_row(
                "SELECT password_hash FROM users WHERE id = ?1",
                [alice],
                |row| row.get(0),
            )
            .unwrap()
        };

        let stale = save_new_password(&conn, alice, 1, "the hash that was checked", "new hash");
        assert_eq!(outcome(&stale), "incorrect password");
        assert_eq!(stored_hash(), "not-a-hash");

        save_new_password(&conn, alice, 1, "not-a-hash", "new hash").unwrap();
        assert_eq!(stored_hash(), "new hash");
    }

    #[tokio::test]
    async fn of_two_parallel_changes_one_wins_and_the_other_password_never_works() {
        let temp = TempDatabase::open();
        let now = SystemTime::now();
        let alice = add_user(&temp, "alice").await;
        let session = add_session(&temp, alice, "alice-phone", now);

        let (a, b) = tokio::join!(
            change(&temp, alice, session, OLD, NEW, NEW, now),
            change(&temp, alice, session, OLD, OTHER, OTHER, now),
        );

        let mut outcomes = [outcome(&a), outcome(&b)];
        outcomes.sort_unstable();
        assert_eq!(outcomes, ["accepted", "incorrect password"]);
        let (winner, loser) = if a.is_ok() {
            (NEW, OTHER)
        } else {
            (OTHER, NEW)
        };
        assert_eq!(login(&temp, loser, now).await, "incorrect");
        assert_eq!(login(&temp, winner, now).await, "accepted");
    }
}
