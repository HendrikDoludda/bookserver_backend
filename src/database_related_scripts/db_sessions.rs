//! Session writes the generic traits can't express: replacing a device's session, rotating
//! tokens only while the presented one still matches, and deleting by either token.

use std::time::SystemTime;

use rusqlite::{params, Connection};

use crate::{
    database_related_scripts::db::convert_system_time_to_unix_time, error_types::DatabaseError,
};

/// Deletes the user's session on this device, if there is one.
pub fn delete_for_device(
    conn: &Connection,
    user_id: i64,
    device_id: &str,
) -> Result<usize, DatabaseError> {
    conn.execute(
        "DELETE FROM sessions WHERE user_id = ?1 AND device_id = ?2",
        params![user_id, device_id],
    )
    .map_err(|_| DatabaseError::QueryFailure)
}

/// The new token hashes and expiry times for [`rotate_tokens`].
pub struct NewTokens<'a> {
    pub session_hash: &'a str,
    pub session_expires_at: SystemTime,
    pub refresh_hash: &'a str,
    pub refresh_expires_at: SystemTime,
}

/// Replaces both tokens, but only while the session still has `old_refresh_hash`: of two
/// refreshes with the same token, the second finds nothing to change. `true` when it rotated.
pub fn rotate_tokens(
    conn: &Connection,
    session_id: i64,
    old_refresh_hash: &str,
    new: &NewTokens,
    now: SystemTime,
) -> Result<bool, DatabaseError> {
    let changed = conn
        .execute(
            "UPDATE sessions SET \
                 session_token_hashed = ?1, session_token_valid_until = ?2, \
                 refresh_token_hashed = ?3, refresh_token_valid_until = ?4, last_used_at = ?5 \
             WHERE session_id = ?6 AND refresh_token_hashed = ?7",
            params![
                new.session_hash,
                convert_system_time_to_unix_time(new.session_expires_at),
                new.refresh_hash,
                convert_system_time_to_unix_time(new.refresh_expires_at),
                convert_system_time_to_unix_time(now),
                session_id,
                old_refresh_hash,
            ],
        )
        .map_err(|_| DatabaseError::UpdatingFailure)?;
    Ok(changed == 1)
}

/// Deletes the session that has this hash as its session or its refresh token.
pub fn delete_by_token_hash(conn: &Connection, token_hash: &str) -> Result<usize, DatabaseError> {
    conn.execute(
        "DELETE FROM sessions WHERE session_token_hashed = ?1 OR refresh_token_hashed = ?1",
        [token_hash],
    )
    .map_err(|_| DatabaseError::QueryFailure)
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, UNIX_EPOCH};

    use super::*;
    use crate::test_support::{insert_user, memory_connection};

    fn add_session(conn: &Connection, user_id: i64, device: &str, session: &str, refresh: &str) {
        conn.execute(
            "INSERT INTO sessions (user_id, device_id, device_name, platform, \
             refresh_token_hashed, refresh_token_valid_until, session_token_hashed, \
             session_token_valid_until, last_used_at) \
             VALUES (?1, ?2, 'phone', 'ios', ?3, 0, ?4, 0, 0)",
            params![user_id, device, refresh, session],
        )
        .unwrap();
    }

    fn count(conn: &Connection) -> i64 {
        conn.query_row("SELECT COUNT(*) FROM sessions", [], |row| row.get(0))
            .unwrap()
    }

    #[test]
    fn a_device_has_at_most_one_session_per_user() {
        let conn = memory_connection();
        let alice = insert_user(&conn, "alice");
        let bob = insert_user(&conn, "bob");
        add_session(&conn, alice, "device-1", "s1", "r1");

        let same_device_again = conn.execute(
            "INSERT INTO sessions (user_id, device_id, device_name, platform, \
             refresh_token_hashed, refresh_token_valid_until, session_token_hashed, \
             session_token_valid_until, last_used_at) \
             VALUES (?1, 'device-1', 'phone', 'ios', 'r2', 0, 's2', 0, 0)",
            [alice],
        );
        assert!(same_device_again.is_err());

        // The same device id on another account is a different session.
        add_session(&conn, bob, "device-1", "s3", "r3");
        assert_eq!(count(&conn), 2);

        assert_eq!(delete_for_device(&conn, alice, "device-1").unwrap(), 1);
        assert_eq!(count(&conn), 1);
    }

    #[test]
    fn a_session_token_is_unique() {
        let conn = memory_connection();
        let alice = insert_user(&conn, "alice");
        add_session(&conn, alice, "device-1", "same", "r1");
        let duplicate = conn.execute(
            "INSERT INTO sessions (user_id, device_id, device_name, platform, \
             refresh_token_hashed, refresh_token_valid_until, session_token_hashed, \
             session_token_valid_until, last_used_at) \
             VALUES (?1, 'device-2', 'phone', 'ios', 'r2', 0, 'same', 0, 0)",
            [alice],
        );
        assert!(duplicate.is_err());
    }

    #[test]
    fn tokens_rotate_only_while_the_old_refresh_token_matches() {
        let conn = memory_connection();
        let alice = insert_user(&conn, "alice");
        add_session(&conn, alice, "device-1", "s1", "r1");
        let session_id = conn.last_insert_rowid();
        let new = NewTokens {
            session_hash: "s2",
            session_expires_at: UNIX_EPOCH + Duration::from_secs(1_700_000_900),
            refresh_hash: "r2",
            refresh_expires_at: UNIX_EPOCH + Duration::from_secs(1_702_592_000),
        };
        let now = UNIX_EPOCH + Duration::from_secs(1_700_000_000);

        assert!(rotate_tokens(&conn, session_id, "r1", &new, now).unwrap());
        assert!(
            !rotate_tokens(&conn, session_id, "r1", &new, now).unwrap(),
            "the old refresh token no longer matches"
        );

        let (session, refresh): (String, String) = conn
            .query_row(
                "SELECT session_token_hashed, refresh_token_hashed FROM sessions",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!((session.as_str(), refresh.as_str()), ("s2", "r2"));
    }

    #[test]
    fn either_token_deletes_its_session() {
        let conn = memory_connection();
        let alice = insert_user(&conn, "alice");
        add_session(&conn, alice, "device-1", "s1", "r1");
        add_session(&conn, alice, "device-2", "s2", "r2");

        assert_eq!(delete_by_token_hash(&conn, "s1").unwrap(), 1);
        assert_eq!(delete_by_token_hash(&conn, "r2").unwrap(), 1);
        assert_eq!(delete_by_token_hash(&conn, "unknown").unwrap(), 0);
        assert_eq!(count(&conn), 0);
    }
}
