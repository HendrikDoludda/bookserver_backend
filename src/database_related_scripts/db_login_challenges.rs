//! Half-finished logins: the password was right, the TOTP code is still pending.

use std::time::SystemTime;

use rusqlite::{params, Connection, OptionalExtension};

use crate::{
    database_related_scripts::{db::convert_system_time_to_unix_time, db_from_row::FromRow},
    error_types::DatabaseError,
    models::LoginChallenge,
};

/// Stores a challenge, and drops every expired one on the way so the table stays small.
pub fn create(
    conn: &Connection,
    challenge: &LoginChallenge,
    now: SystemTime,
) -> Result<(), DatabaseError> {
    conn.execute(
        "DELETE FROM login_challenges WHERE expires_at <= ?1",
        [convert_system_time_to_unix_time(now)],
    )
    .map_err(|_| DatabaseError::QueryFailure)?;
    conn.execute(
        "INSERT INTO login_challenges \
             (user_id, token_hashed, expires_at, device_id, device_name, platform) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            challenge.user_id,
            challenge.token_hashed,
            convert_system_time_to_unix_time(challenge.expires_at),
            challenge.device_id,
            challenge.device_name,
            challenge.platform,
        ],
    )
    .map_err(|err| {
        log::error!("storing a login challenge failed: {err}");
        DatabaseError::InsertionFailure
    })?;
    Ok(())
}

/// The challenge with this token hash, unless it has expired.
pub fn find_live(
    conn: &Connection,
    token_hash: &str,
    now: SystemTime,
) -> Result<Option<LoginChallenge>, DatabaseError> {
    conn.query_row(
        "SELECT * FROM login_challenges WHERE token_hashed = ?1 AND expires_at > ?2",
        params![token_hash, convert_system_time_to_unix_time(now)],
        LoginChallenge::from_row,
    )
    .optional()
    .map_err(|err| {
        log::error!("reading a login challenge failed: {err}");
        DatabaseError::QueryFailure
    })
}

pub fn delete_for_user(conn: &Connection, user_id: i64) -> Result<usize, DatabaseError> {
    conn.execute("DELETE FROM login_challenges WHERE user_id = ?1", [user_id])
        .map_err(|_| DatabaseError::QueryFailure)
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, UNIX_EPOCH};

    use super::*;
    use crate::test_support::{insert_user, memory_connection};

    fn at(secs: u64) -> SystemTime {
        UNIX_EPOCH + Duration::from_secs(secs)
    }

    fn challenge(user_id: i64, token_hashed: &str, expires_at: u64) -> LoginChallenge {
        LoginChallenge {
            challenge_id: 0,
            user_id,
            token_hashed: token_hashed.to_string(),
            expires_at: at(expires_at),
            device_id: "device-1".to_string(),
            device_name: "phone".to_string(),
            platform: "ios".to_string(),
        }
    }

    fn count(conn: &Connection) -> i64 {
        conn.query_row("SELECT COUNT(*) FROM login_challenges", [], |row| {
            row.get(0)
        })
        .unwrap()
    }

    #[test]
    fn a_live_challenge_is_found_by_its_token_hash() {
        let conn = memory_connection();
        let alice = insert_user(&conn, "alice");
        create(
            &conn,
            &challenge(alice, "hash", 1_700_000_300),
            at(1_700_000_000),
        )
        .unwrap();

        let found = find_live(&conn, "hash", at(1_700_000_100))
            .unwrap()
            .expect("the challenge is live");
        assert_eq!(found.user_id, alice);
        assert_eq!(found.device_id, "device-1");
        assert!(find_live(&conn, "other", at(1_700_000_100))
            .unwrap()
            .is_none());
    }

    #[test]
    fn an_expired_challenge_is_not_found() {
        let conn = memory_connection();
        let alice = insert_user(&conn, "alice");
        create(
            &conn,
            &challenge(alice, "hash", 1_700_000_300),
            at(1_700_000_000),
        )
        .unwrap();
        assert!(find_live(&conn, "hash", at(1_700_000_300))
            .unwrap()
            .is_none());
    }

    #[test]
    fn creating_a_challenge_drops_expired_ones() {
        let conn = memory_connection();
        let alice = insert_user(&conn, "alice");
        create(
            &conn,
            &challenge(alice, "old", 1_700_000_300),
            at(1_700_000_000),
        )
        .unwrap();
        create(
            &conn,
            &challenge(alice, "new", 1_700_000_900),
            at(1_700_000_600),
        )
        .unwrap();
        assert_eq!(count(&conn), 1);
    }

    #[test]
    fn a_users_challenges_can_all_be_dropped() {
        let conn = memory_connection();
        let alice = insert_user(&conn, "alice");
        let bob = insert_user(&conn, "bob");
        let now = at(1_700_000_000);
        create(&conn, &challenge(alice, "a1", 1_700_000_300), now).unwrap();
        create(&conn, &challenge(alice, "a2", 1_700_000_300), now).unwrap();
        create(&conn, &challenge(bob, "b1", 1_700_000_300), now).unwrap();

        assert_eq!(delete_for_user(&conn, alice).unwrap(), 2);
        assert_eq!(count(&conn), 1);
    }
}
