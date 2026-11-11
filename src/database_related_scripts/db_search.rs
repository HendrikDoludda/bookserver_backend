use crate::data_models::models::{
    BookDatabaseColumns, BookMetadata, BookSeriesMetadata, LibraryDatabaseColumns, LibraryMetadata,
    QuerySeparator, RecoveryCodes, RecoveryCodesColumns, SeriesDatabaseColumns, Sessions,
    SessionsDatabaseColumns, TOTPDatabaseColumns, UserDatabaseColumns, UserMetadata, TOTP,
};
use crate::database_related_scripts::{
    db::{each_equal, logged, where_conditions},
    db_from_row::FromRow,
};
use crate::error_types::DatabaseError;
use rusqlite::{Connection, OptionalExtension, Result, ToSql};

/// Find rows by arbitrary column values, joined with `AND` or `OR`.
/// Column names come from trusted enums; values are always bound parameters.
pub trait Search: FromRow {
    type Column: AsRef<str>;
    const TABLE: &'static str;

    /// Returns the first matching row, or `None` if nothing matches.
    fn search_for_row(
        conn: &Connection,
        columns: &[Self::Column],
        values: &[&dyn ToSql],
        query_separator: QuerySeparator,
    ) -> Result<Option<Self>, DatabaseError> {
        let filter = where_conditions(&each_equal(columns), values, query_separator)?;
        let query = format!("SELECT * FROM {} WHERE {} LIMIT 1", Self::TABLE, filter);
        conn.query_row(&query, values, Self::from_row)
            .optional()
            .map_err(logged("reading", Self::TABLE, DatabaseError::QueryFailure))
    }

    fn search_for_multiple_rows(
        conn: &Connection,
        columns: &[Self::Column],
        values: &[&dyn ToSql],
        query_separator: QuerySeparator,
    ) -> Result<Vec<Self>, DatabaseError> {
        let filter = where_conditions(&each_equal(columns), values, query_separator)?;
        let query = format!("SELECT * FROM {} WHERE {}", Self::TABLE, filter);

        let mut stmt = conn.prepare(&query).map_err(logged(
            "reading",
            Self::TABLE,
            DatabaseError::TaskPreparationFailure,
        ))?;

        let result = stmt
            .query_map(values, Self::from_row)
            .map_err(logged("reading", Self::TABLE, DatabaseError::QueryFailure))?
            .collect::<Result<Vec<Self>, _>>()
            .map_err(logged(
                "reading",
                Self::TABLE,
                DatabaseError::OperationFailure,
            ))?;
        Ok(result)
    }
}

impl Search for BookMetadata {
    type Column = BookDatabaseColumns;
    const TABLE: &'static str = "books";
}

impl Search for BookSeriesMetadata {
    type Column = SeriesDatabaseColumns;
    const TABLE: &'static str = "series";
}

impl Search for LibraryMetadata {
    type Column = LibraryDatabaseColumns;
    const TABLE: &'static str = "library";
}

impl Search for UserMetadata {
    type Column = UserDatabaseColumns;
    const TABLE: &'static str = "users";
}

impl Search for Sessions {
    type Column = SessionsDatabaseColumns;
    const TABLE: &'static str = "sessions";
}

impl Search for RecoveryCodes {
    type Column = RecoveryCodesColumns;
    const TABLE: &'static str = "recovery_codes";
}

impl Search for TOTP {
    type Column = TOTPDatabaseColumns;
    const TABLE: &'static str = "totp";
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{capture_logs, insert_user, memory_connection};

    #[test]
    fn an_unreadable_row_is_logged_once_per_search_and_a_missing_one_not_at_all() {
        let conn = memory_connection();
        let user_id = insert_user(&conn, "alice");
        insert_totp(&conn, user_id, &"not a number");
        let by_user = |user: i64| {
            TOTP::search_for_row(
                &conn,
                &[TOTPDatabaseColumns::UserId],
                &[&user],
                QuerySeparator::And,
            )
        };

        let mut results = Vec::new();
        let logs = capture_logs(|| {
            results.push(by_user(user_id).map(|_| ()));
            results.push(
                TOTP::search_for_multiple_rows(
                    &conn,
                    &[TOTPDatabaseColumns::UserId],
                    &[&user_id],
                    QuerySeparator::And,
                )
                .map(|_| ()),
            );
        });
        assert!(matches!(results[0], Err(DatabaseError::QueryFailure)));
        assert!(matches!(results[1], Err(DatabaseError::OperationFailure)));
        assert_eq!(logs.matches("reading totp failed:").count(), 2, "{logs}");

        let mut missing = None;
        let logs = capture_logs(|| missing = Some(by_user(user_id + 1)));
        assert!(matches!(missing, Some(Ok(None))));
        assert_eq!(logs, "", "finding nothing is no error");
    }

    fn insert_totp(conn: &Connection, user_id: i64, created_at: &dyn ToSql) {
        conn.execute(
            "INSERT INTO totp (user_id, authentication_secret, created_at) VALUES (?1, 'secret', ?2)",
            rusqlite::params![user_id, created_at],
        )
        .unwrap();
    }

    #[test]
    fn search_finds_a_matching_row() {
        let conn = memory_connection();
        let user_id = insert_user(&conn, "alice");
        insert_totp(&conn, user_id, &1_700_000_000_i64);

        let found = TOTP::search_for_row(
            &conn,
            &[TOTPDatabaseColumns::UserId],
            &[&user_id],
            QuerySeparator::And,
        )
        .unwrap()
        .expect("the row exists");
        assert_eq!(found.user_id, user_id);
        assert_eq!(found.authentication_secret, "secret");
    }

    #[test]
    fn search_without_a_match_is_none() {
        let conn = memory_connection();
        let found = TOTP::search_for_row(
            &conn,
            &[TOTPDatabaseColumns::UserId],
            &[&42],
            QuerySeparator::And,
        )
        .unwrap();
        assert!(found.is_none());
    }

    #[test]
    fn a_search_without_columns_or_with_a_value_per_column_missing_is_refused() {
        let conn = memory_connection();
        let user_id = insert_user(&conn, "alice");
        insert_totp(&conn, user_id, &1_700_000_000_i64);
        let one_column = [TOTPDatabaseColumns::UserId];

        let row_without_columns = TOTP::search_for_row(&conn, &[], &[], QuerySeparator::And);
        let rows_without_columns =
            TOTP::search_for_multiple_rows(&conn, &[], &[], QuerySeparator::And);
        let row_without_value = TOTP::search_for_row(&conn, &one_column, &[], QuerySeparator::And);
        let rows_without_value =
            TOTP::search_for_multiple_rows(&conn, &one_column, &[], QuerySeparator::And);

        assert!(matches!(
            row_without_columns,
            Err(DatabaseError::InvalidParameters)
        ));
        assert!(matches!(
            rows_without_columns,
            Err(DatabaseError::InvalidParameters)
        ));
        assert!(matches!(
            row_without_value,
            Err(DatabaseError::InvalidParameters)
        ));
        assert!(matches!(
            rows_without_value,
            Err(DatabaseError::InvalidParameters)
        ));
    }

    #[test]
    fn an_unreadable_row_is_a_query_failure_not_a_missing_one() {
        let conn = memory_connection();
        let user_id = insert_user(&conn, "alice");
        insert_totp(&conn, user_id, &"not a number");

        let result = TOTP::search_for_row(
            &conn,
            &[TOTPDatabaseColumns::UserId],
            &[&user_id],
            QuerySeparator::And,
        );
        assert!(matches!(result, Err(DatabaseError::QueryFailure)));
    }
}
