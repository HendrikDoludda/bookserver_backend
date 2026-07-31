use crate::data_models::models::{
    BookDatabaseColumns, BookMetadata, BookSeriesMetadata, EmailVerification,
    EmailVerificationDatabaseColumns, LibraryDatabaseColumns, LibraryMetadata, RecoveryCodes,
    RecoveryCodesColumns, SeriesDatabaseColumns, Sessions, SessionsDatabaseColumns,
    TOTPDatabaseColumns, UserDatabaseColumns, UserMetadata, TOTP,
};
use crate::error_types::DatabaseError;
use rusqlite::{params, params_from_iter, Connection, ToSql};
use tracing_subscriber::registry::Data;

pub trait Update {
    type Column: AsRef<str>;

    const TABLE: &'static str;

    fn update(
        conn: &Connection,
        column: Self::Column,
        new_value: &dyn ToSql,
        id: i64,
    ) -> Result<(), DatabaseError> {
        let query = format!(
            "UPDATE {} SET {} = ?1 WHERE id = ?2",
            Self::TABLE,
            column.as_ref()
        );

        conn.execute(&query, params![new_value, id])
            .map_err(|_| DatabaseError::UpdatingFailure)?;

        Ok(())
    }

    fn update_multiple(
        conn: &Connection,
        columns: &[Self::Column],
        values: &[&dyn ToSql],
        id: i64,
    ) -> Result<(), DatabaseError> {
        if columns.len() != values.len() {
            return Err(DatabaseError::InvalidParameters);
        }

        let assignments = columns
            .iter()
            .enumerate()
            .map(|(i, column)| format!("{} = ?{}", column.as_ref(), i + 1))
            .collect::<Vec<String>>()
            .join(", ");
        let id_placeholder = values.len() + 1;
        let query = format!(
            "UPDATE {} SET {} WHERE id = ?{}",
            Self::TABLE,
            assignments,
            id_placeholder
        );
        let mut bound: Vec<&dyn ToSql> = values.to_vec();
        bound.push(&id);
        conn.execute(&query, params_from_iter(bound))
            .map_err(|_| DatabaseError::UpdatingFailure)?;
        Ok(())
    }
}

impl Update for BookMetadata {
    type Column = BookDatabaseColumns;
    const TABLE: &'static str = "books";
}

impl Update for BookSeriesMetadata {
    type Column = SeriesDatabaseColumns;
    const TABLE: &'static str = "series";
}

impl Update for LibraryMetadata {
    type Column = LibraryDatabaseColumns;
    const TABLE: &'static str = "library";
}

impl Update for UserMetadata {
    type Column = UserDatabaseColumns;
    const TABLE: &'static str = "users";
}

impl Update for EmailVerification {
    type Column = EmailVerificationDatabaseColumns;
    const TABLE: &'static str = "email_verification";
}

impl Update for Sessions {
    type Column = SessionsDatabaseColumns;
    const TABLE: &'static str = "sessions";
}

impl Update for RecoveryCodes {
    type Column = RecoveryCodesColumns;
    const TABLE: &'static str = "recovery_codes";
}

impl Update for TOTP {
    type Column = TOTPDatabaseColumns;
    const TABLE: &'static str = "totp";
}
