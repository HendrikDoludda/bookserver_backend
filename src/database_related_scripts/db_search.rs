use crate::data_models::models::{
    BookDatabaseColumns, BookMetadata, BookSeriesMetadata, EmailVerification,
    EmailVerificationDatabaseColumns, LibraryDatabaseColumns, LibraryMetadata, QuerySeparator,
    RecoveryCodes, RecoveryCodesColumns, SeriesDatabaseColumns, Sessions, SessionsDatabaseColumns,
    TOTPDatabaseColumns, UserDatabaseColumns, UserMetadata, TOTP,
};
use crate::database_related_scripts::db_from_row::FromRow;
use crate::error_types::DatabaseError;
use crate::models::{ResetPasswordRequest, ResetPasswordRequestColumns};
use rusqlite::{Connection, Result, ToSql};

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
        if columns.len() != values.len() {
            return Err(DatabaseError::InvalidParameters);
        }
        let values_to_check = columns
            .iter()
            .map(|column| format!("{} = ?", column.as_ref()))
            .collect::<Vec<String>>()
            .join(query_separator.as_str());
        let query = format!(
            "SELECT * FROM {} WHERE {} LIMIT 1",
            Self::TABLE,
            values_to_check
        );
        let result = conn.query_row(&query, values, Self::from_row);
        match result {
            Ok(entry) => Ok(Some(entry)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(err) => {
                log::error!("searching {} failed: {err}", Self::TABLE);
                Err(DatabaseError::QueryFailure)
            }
        }
    }

    fn search_for_multiple_rows(
        conn: &Connection,
        columns: &[Self::Column],
        values: &[&dyn ToSql],
        query_separator: QuerySeparator,
    ) -> Result<Vec<Self>, DatabaseError> {
        if columns.len() != values.len() {
            return Err(DatabaseError::InvalidParameters);
        }

        let values_to_check = columns
            .iter()
            .map(|column| format!("{} = ?", column.as_ref()))
            .collect::<Vec<String>>()
            .join(query_separator.as_str());
        let query = format!("SELECT * FROM {} WHERE {}", Self::TABLE, values_to_check);

        let mut stmt = conn
            .prepare(&query)
            .map_err(|_| DatabaseError::TaskPreparationFailure)?;

        let result = stmt
            .query_map(values, Self::from_row)
            .map_err(|_| DatabaseError::QueryFailure)?
            .collect::<Result<Vec<Self>, _>>()
            .map_err(|_| DatabaseError::OperationFailure)?;
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

impl Search for EmailVerification {
    type Column = EmailVerificationDatabaseColumns;
    const TABLE: &'static str = "email_verification";
}

impl Search for ResetPasswordRequest {
    type Column = ResetPasswordRequestColumns;
    const TABLE: &'static str = "reset_password_requests";
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
    use crate::test_support::{insert_user, memory_connection};

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
