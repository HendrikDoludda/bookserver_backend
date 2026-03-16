use crate::{models::{
    BookDatabaseColumns, BookMetadata, BookSeriesMetadata, LibraryDatabaseColumns, LibraryMetadata, SeriesDatabaseColumns, UserDatabaseColumns, UserMetadata
}};
use rusqlite::{Connection, ToSql, params};
use crate::error_types::DatabaseError;

pub trait Update{
    type Column: AsRef<str>;

    const TABLE: &'static str;

    fn update<T>(
        &self,
        conn: &Connection,
        column: Self::Column,
        new_value: T,
        id: i64,
    ) -> Result<(), DatabaseError> 
    where T: ToSql{
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