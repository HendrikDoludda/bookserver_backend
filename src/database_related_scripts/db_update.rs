use crate::error_types::DatabaseError;
use crate::models::{
    BookDatabaseColumns, BookMetadata, BookSeriesMetadata, EmailVerification,
    EmailVerificationDatabaseColumns, LibraryDatabaseColumns, LibraryMetadata, RecoveryCodes,
    RecoveryCodesColumns, SeriesDatabaseColumns, Sessions, SessionsDatabaseColumns,
    TOTPDatabaseColumns, UserDatabaseColumns, UserMetadata, TOTP,
};
use rusqlite::{params, Connection, ToSql};

pub trait Update {
    type Column: AsRef<str>;

    const TABLE: &'static str;

    fn update<T>(
        &self,
        conn: &Connection,
        column: Self::Column,
        new_value: T,
        id: i64,
    ) -> Result<(), DatabaseError>
    where
        T: ToSql,
    {
        let query = format!(
            "UPDATE {} SET {} = ?1 WHERE id = ?2",
            Self::TABLE,
            column.as_ref()
        );

        conn.execute(&query, params![new_value, id])
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
