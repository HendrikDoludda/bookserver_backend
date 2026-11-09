use rusqlite::{params_from_iter, Connection, ToSql};

use crate::{
    database_related_scripts::db_from_row::FromRow,
    error_types::DatabaseError,
    models::{
        BookDatabaseColumns, BookMetadata, BookSeriesMetadata, LibraryDatabaseColumns,
        LibraryMetadata, QuerySeparator, RecoveryCodes, RecoveryCodesColumns,
        SeriesDatabaseColumns, Sessions, SessionsDatabaseColumns, TOTPDatabaseColumns,
        UserDatabaseColumns, UserMetadata, TOTP,
    },
};

/// Delete every row matching the given column values, joined with `AND` or `OR`.
/// Useful for tables keyed by something other than `id` (e.g. `user_id`).
/// Returns the number of rows deleted.
pub trait Delete: FromRow {
    type Column: AsRef<str>;
    const TABLE: &'static str;

    fn delete(
        conn: &Connection,
        columns: &[Self::Column],
        values: &[&dyn ToSql],
        query_separator: QuerySeparator,
    ) -> Result<usize, DatabaseError> {
        if columns.is_empty() || columns.len() != values.len() {
            return Err(DatabaseError::InvalidParameters);
        }
        let columns_to_check = columns
            .iter()
            .map(|column| format!("{} = ?", column.as_ref()))
            .collect::<Vec<String>>()
            .join(query_separator.as_str());
        let query = format!("DELETE FROM {} WHERE {}", Self::TABLE, columns_to_check);

        conn.execute(&query, params_from_iter(values.iter()))
            .map_err(|_| DatabaseError::QueryFailure)
    }
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
    use crate::test_support::{insert_user, memory_connection};

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
}
