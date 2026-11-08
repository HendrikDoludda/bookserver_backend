//! Login tries per typed name, counted whether or not the name belongs to an account.

use std::time::{Duration, SystemTime};

use rusqlite::{params, Connection};

use crate::{
    database_related_scripts::db::convert_system_time_to_unix_time, error_types::DatabaseError,
};

/// Counts one login try for `name`. `false` while the name is locked: it already had
/// `max_tries` tries in the window that began with its first counted try.
pub fn reserve(
    conn: &Connection,
    name: &str,
    now: SystemTime,
    window: Duration,
    max_tries: i64,
) -> Result<bool, DatabaseError> {
    let now = convert_system_time_to_unix_time(now);
    // Drops every window that is over, this name's included, so the table stays small and
    // an expired lock starts again from zero.
    conn.execute(
        "DELETE FROM login_failures WHERE window_started_at <= ?1",
        [now - window.as_secs() as i64],
    )
    .map_err(|_| DatabaseError::QueryFailure)?;
    let changed = conn
        .execute(
            "INSERT INTO login_failures (name, attempts, window_started_at) VALUES (?1, 1, ?2) \
             ON CONFLICT (name) DO UPDATE SET attempts = attempts + 1 WHERE attempts < ?3",
            params![name, now, max_tries],
        )
        .map_err(|_| DatabaseError::UpdatingFailure)?;
    Ok(changed == 1)
}

/// Forgets the tries for `name`, after a successful login.
pub fn clear(conn: &Connection, name: &str) -> Result<(), DatabaseError> {
    conn.execute("DELETE FROM login_failures WHERE name = ?1", [name])
        .map_err(|_| DatabaseError::QueryFailure)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::time::UNIX_EPOCH;

    use super::*;
    use crate::test_support::memory_connection;

    const WINDOW: Duration = Duration::from_secs(15 * 60);

    fn at(secs: u64) -> SystemTime {
        UNIX_EPOCH + Duration::from_secs(secs)
    }

    fn rows(conn: &Connection) -> i64 {
        conn.query_row("SELECT COUNT(*) FROM login_failures", [], |row| row.get(0))
            .unwrap()
    }

    #[test]
    fn a_name_is_locked_after_the_maximum_tries() {
        let conn = memory_connection();
        let now = at(1_700_000_000);
        for _ in 0..3 {
            assert!(reserve(&conn, "alice", now, WINDOW, 3).unwrap());
        }
        assert!(!reserve(&conn, "alice", now, WINDOW, 3).unwrap());
        assert!(
            reserve(&conn, "bob", now, WINDOW, 3).unwrap(),
            "other names are not affected"
        );
    }

    #[test]
    fn the_lock_ends_with_its_window() {
        let conn = memory_connection();
        let start = at(1_700_000_000);
        for _ in 0..3 {
            reserve(&conn, "alice", start, WINDOW, 3).unwrap();
        }
        assert!(!reserve(
            &conn,
            "alice",
            start + WINDOW - Duration::from_secs(1),
            WINDOW,
            3
        )
        .unwrap());
        assert!(reserve(&conn, "alice", start + WINDOW, WINDOW, 3).unwrap());
    }

    #[test]
    fn clearing_forgets_the_tries() {
        let conn = memory_connection();
        let now = at(1_700_000_000);
        for _ in 0..3 {
            reserve(&conn, "alice", now, WINDOW, 3).unwrap();
        }
        clear(&conn, "alice").unwrap();
        assert!(reserve(&conn, "alice", now, WINDOW, 3).unwrap());
    }

    #[test]
    fn old_windows_of_other_names_are_dropped() {
        let conn = memory_connection();
        reserve(&conn, "old", at(1_700_000_000), WINDOW, 3).unwrap();
        reserve(&conn, "new", at(1_700_000_000) + WINDOW, WINDOW, 3).unwrap();
        assert_eq!(rows(&conn), 1);
    }
}
