use crate::data_models::models::{
    BookDatabaseColumns, BookMetadata, BookSeriesMetadata, EmailVerification,
    EmailVerificationDatabaseColumns, LibraryDatabaseColumns, LibraryMetadata, QuerySeparator,
    RecoveryCodes, RecoveryCodesColumns, SelectionMethod, SeriesDatabaseColumns, Sessions,
    SessionsDatabaseColumns, TOTPDatabaseColumns, UserDatabaseColumns, UserMetadata, TOTP,
};
use crate::database_related_scripts::db_from_row::FromRow;
use crate::error_types::DatabaseError;
use rusqlite::{Connection, Result, ToSql};

pub trait Search: FromRow {
    type Column: AsRef<str>;
    const TABLE: &'static str;

    fn search_for_row(
        conn: &Connection,
        columns: &[Self::Column],
        values: &[&dyn ToSql],
        query_separator: QuerySeparator,
        selection_method: SelectionMethod,
    ) -> Result<Option<Self>, DatabaseError> {
        if columns.len() != values.len() {
            return Err(DatabaseError::InvalidParameters);
        }
        let values_to_check = columns
            .iter()
            .map(|column| format!("{} = ?", column.as_ref()))
            .collect::<Vec<String>>()
            .join(query_separator.as_str());
        let selection = match selection_method {
            SelectionMethod::Everything => " * ".to_string(),
            SelectionMethod::PassedInColumns => columns
                .iter()
                .map(|column| column.as_ref())
                .collect::<Vec<_>>()
                .join(", "),
        };
        let query = format!(
            "SELECT {} FROM {} WHERE {} LIMIT 1",
            selection,
            Self::TABLE,
            values_to_check
        );
        let result = conn.query_row(&query, values, |row| Self::from_row(row)); //will create the from row in another script
        match result {
            Ok(entry) => Ok(Some(entry)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(_) => Err(DatabaseError::EntryNotFound),
        }
    }

    fn search_for_multiple_rows(
        conn: &Connection,
        columns: &[Self::Column],
        values: &[&dyn ToSql],
        query_separator: QuerySeparator,
        selection_method: SelectionMethod,
    ) -> Result<Vec<Self>, DatabaseError> {
        if columns.len() != values.len() {
            return Err(DatabaseError::InvalidParameters);
        }

        let values_to_check = columns
            .iter()
            .map(|column| format!("{} = ?", column.as_ref()))
            .collect::<Vec<String>>()
            .join(query_separator.as_str());
        let selection = match selection_method {
            SelectionMethod::Everything => " * ".to_string(),
            SelectionMethod::PassedInColumns => columns
                .iter()
                .map(|column| column.as_ref())
                .collect::<Vec<_>>()
                .join(", "),
        };
        let query = format!(
            "SELECT {} FROM {} WHERE {}",
            selection,
            Self::TABLE,
            values_to_check
        );

        let mut stmt = conn
            .prepare(&query)
            .map_err(|_| DatabaseError::TaskPreparationFailure)?;

        let result = stmt
            .query_map(values, |row| Self::from_row(row))
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
