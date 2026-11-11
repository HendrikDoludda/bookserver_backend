use rusqlite::{params_from_iter, Connection, ToSql};

use crate::{
    database_related_scripts::{
        db::{each_equal, logged, where_conditions},
        db_from_row::FromRow,
    },
    error_types::DatabaseError,
    models::{
        BookDatabaseColumns, BookMetadata, BookSeriesMetadata, Comparison, LibraryDatabaseColumns,
        LibraryMetadata, QuerySeparator, RecoveryCodes, RecoveryCodesColumns,
        SeriesDatabaseColumns, Sessions, SessionsDatabaseColumns, TOTPDatabaseColumns,
        UserDatabaseColumns, UserMetadata, TOTP,
    },
};

/// Delete every row matching the given conditions, joined with `AND` or `OR`.
/// Useful for tables keyed by something other than `id` (e.g. `user_id`).
/// Returns the number of rows deleted.
pub trait Delete: FromRow {
    type Column: AsRef<str>;
    const TABLE: &'static str;

    /// Each column compared with `=`.
    fn delete(
        conn: &Connection,
        columns: &[Self::Column],
        values: &[&dyn ToSql],
        query_separator: QuerySeparator,
    ) -> Result<usize, DatabaseError> {
        let filter = where_conditions(&each_equal(columns), values, query_separator)?;
        delete_rows(conn, Self::TABLE, &filter, values)
    }

    /// Each column compared with its value by its `Comparison`.
    fn delete_where(
        conn: &Connection,
        conditions: &[(Self::Column, Comparison)],
        values: &[&dyn ToSql],
        query_separator: QuerySeparator,
    ) -> Result<usize, DatabaseError> {
        let filter = where_conditions(conditions, values, query_separator)?;
        delete_rows(conn, Self::TABLE, &filter, values)
    }
}

fn delete_rows(
    conn: &Connection,
    table: &'static str,
    filter: &str,
    values: &[&dyn ToSql],
) -> Result<usize, DatabaseError> {
    let query = format!("DELETE FROM {table} WHERE {filter}");
    conn.execute(&query, params_from_iter(values.iter()))
        .map_err(logged("deleting from", table, DatabaseError::QueryFailure))
}

impl Delete for BookMetadata {
    type Column = BookDatabaseColumns;
    const TABLE: &'static str = "books";
}

impl Delete for BookSeriesMetadata {
    type Column = SeriesDatabaseColumns;
    const TABLE: &'static str = "series";
}

impl Delete for LibraryMetadata {
    type Column = LibraryDatabaseColumns;
    const TABLE: &'static str = "library";
}

impl Delete for UserMetadata {
    type Column = UserDatabaseColumns;
    const TABLE: &'static str = "users";
}

impl Delete for Sessions {
    type Column = SessionsDatabaseColumns;
    const TABLE: &'static str = "sessions";
}

impl Delete for RecoveryCodes {
    type Column = RecoveryCodesColumns;
    const TABLE: &'static str = "recovery_codes";
}

impl Delete for TOTP {
    type Column = TOTPDatabaseColumns;
    const TABLE: &'static str = "totp";
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{capture_logs, insert_user, memory_connection};

    #[test]
    fn a_failed_delete_is_logged_with_its_table_and_cause() {
        let conn = memory_connection();
        insert_user(&conn, "alice");
        // Makes SQLite refuse every write on this connection.
        conn.execute_batch("PRAGMA query_only = ON").unwrap();

        let mut result = None;
        let logs = capture_logs(|| {
            result = Some(UserMetadata::delete(
                &conn,
                &[UserDatabaseColumns::Username],
                &[&"alice"],
                QuerySeparator::And,
            ));
        });

        assert!(matches!(result, Some(Err(DatabaseError::QueryFailure))));
        assert!(
            logs.contains("deleting from users failed: attempt to write a readonly database"),
            "{logs}"
        );
    }

    #[test]
    fn delete_reports_how_many_rows_matched() {
        let conn = memory_connection();
        insert_user(&conn, "alice");
        insert_user(&conn, "bob");

        let deleted = UserMetadata::delete(
            &conn,
            &[UserDatabaseColumns::Username],
            &[&"alice"],
            QuerySeparator::And,
        )
        .unwrap();
        assert_eq!(deleted, 1);

        let deleted_again = UserMetadata::delete(
            &conn,
            &[UserDatabaseColumns::Username],
            &[&"alice"],
            QuerySeparator::And,
        )
        .unwrap();
        assert_eq!(deleted_again, 0);

        let remaining: i64 = conn
            .query_row("SELECT COUNT(*) FROM users", [], |row| row.get(0))
            .unwrap();
        assert_eq!(remaining, 1);
    }

    #[test]
    fn delete_without_columns_is_refused() {
        let conn = memory_connection();
        let result = UserMetadata::delete(&conn, &[], &[], QuerySeparator::And);
        assert!(matches!(result, Err(DatabaseError::InvalidParameters)));
    }

    fn insert_session(conn: &Connection, user_id: i64, device_id: &str, refresh_valid_until: i64) {
        conn.execute(
            "INSERT INTO sessions (user_id, device_id, device_name, platform, \
             refresh_token_hashed, refresh_token_valid_until, session_token_hashed, \
             session_token_valid_until, last_used_at) \
             VALUES (?1, ?2, 'phone', 'ios', ?2, ?3, ?2, 0, 0)",
            rusqlite::params![user_id, device_id, refresh_valid_until],
        )
        .unwrap();
    }

    fn remaining_devices(conn: &Connection) -> Vec<String> {
        conn.prepare("SELECT device_id FROM sessions ORDER BY device_id")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap()
    }

    #[test]
    fn at_most_deletes_the_rows_up_to_and_including_the_bound() {
        let conn = memory_connection();
        let alice = insert_user(&conn, "alice");
        insert_session(&conn, alice, "below", 99);
        insert_session(&conn, alice, "at", 100);
        insert_session(&conn, alice, "above", 101);

        let deleted = Sessions::delete_where(
            &conn,
            &[(
                SessionsDatabaseColumns::RefreshTokenExpirationDate,
                Comparison::AtMost,
            )],
            &[&100],
            QuerySeparator::And,
        )
        .unwrap();

        assert_eq!(deleted, 2);
        assert_eq!(remaining_devices(&conn), ["above"]);
    }

    #[test]
    fn delete_where_mixes_a_bound_with_an_equal_column() {
        let conn = memory_connection();
        let alice = insert_user(&conn, "alice");
        let bob = insert_user(&conn, "bob");
        insert_session(&conn, alice, "alice-old", 50);
        insert_session(&conn, alice, "alice-new", 150);
        insert_session(&conn, bob, "bob-old", 50);

        let deleted = Sessions::delete_where(
            &conn,
            &[
                (SessionsDatabaseColumns::UserId, Comparison::Equal),
                (
                    SessionsDatabaseColumns::RefreshTokenExpirationDate,
                    Comparison::AtMost,
                ),
            ],
            &[&alice, &100],
            QuerySeparator::And,
        )
        .unwrap();

        assert_eq!(deleted, 1);
        assert_eq!(remaining_devices(&conn), ["alice-new", "bob-old"]);
    }

    #[test]
    fn delete_where_without_conditions_or_with_a_value_missing_deletes_nothing() {
        let conn = memory_connection();
        let alice = insert_user(&conn, "alice");
        insert_session(&conn, alice, "phone", 50);

        let without_conditions = Sessions::delete_where(&conn, &[], &[], QuerySeparator::And);
        let without_a_value = Sessions::delete_where(
            &conn,
            &[
                (SessionsDatabaseColumns::UserId, Comparison::Equal),
                (
                    SessionsDatabaseColumns::RefreshTokenExpirationDate,
                    Comparison::AtMost,
                ),
            ],
            &[&alice],
            QuerySeparator::And,
        );

        assert!(matches!(
            without_conditions,
            Err(DatabaseError::InvalidParameters)
        ));
        assert!(matches!(
            without_a_value,
            Err(DatabaseError::InvalidParameters)
        ));
        assert_eq!(remaining_devices(&conn), ["phone"]);
    }
}
