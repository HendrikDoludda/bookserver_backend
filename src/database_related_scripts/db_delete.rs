use rusqlite::{params_from_iter, Connection, ToSql};

use crate::{
    database_related_scripts::db_from_row::FromRow,
    error_types::DatabaseError,
    models::{
        BookDatabaseColumns, BookMetadata, BookSeriesMetadata, EmailVerification,
        EmailVerificationDatabaseColumns, LibraryDatabaseColumns, LibraryMetadata, QuerySeparator,
        RecoveryCodes, RecoveryCodesColumns, ResetPasswordRequest, ResetPasswordRequestColumns,
        SeriesDatabaseColumns, Sessions, SessionsDatabaseColumns, TOTPDatabaseColumns,
        UserDatabaseColumns, UserMetadata, TOTP,
    },
};

pub trait Delete: FromRow {
    type Column: AsRef<str>;
    const TABLE: &'static str;
    fn delete(
        conn: &Connection,
        columns: &[Self::Column],
        values: &[&dyn ToSql],
        query_separator: QuerySeparator,
    ) -> Result<(), DatabaseError> {
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
            .map_err(|_| DatabaseError::QueryFailure)?;
        Ok(())
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

impl Delete for EmailVerification {
    type Column = EmailVerificationDatabaseColumns;
    const TABLE: &'static str = "email_verification";
}

impl Delete for ResetPasswordRequest {
    type Column = ResetPasswordRequestColumns;
    const TABLE: &'static str = "reset_password_requests";
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
