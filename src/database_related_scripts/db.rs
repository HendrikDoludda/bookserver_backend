use crate::database_related_scripts::db_update::Update;
use crate::database_related_scripts::extract::Extract;
use crate::database_related_scripts::insert::Insert;
use crate::database_related_scripts::migrations;
use crate::error_types::DatabaseError;
use crate::models::ColumnSelector;
use crate::models::DatabaseTypes;
use crate::models::WithId;
use serde::Serialize;
use r2d2::{ManageConnection, Pool};
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::{Connection, Result, ToSql};
use std::path::Path;
//TODO: Check if the filtering should happen in the backend or the frontend. I think both are valid but I imagine that backend is better
//So probably pass in filtering options with the request and then filter somewhere here
pub struct Database {
    pool: r2d2::Pool<SqliteConnectionManager>,
}

impl Database {
    pub fn new() -> Result<Self, DatabaseError> {
        let path = Path::new("./data/databases/app_data.sqlite");

        // Ensure the database folder exists
        std::fs::create_dir_all("./data/databases/")
            .map_err(|_| DatabaseError::DirectoryCreationFailure)?;

        let manager = SqliteConnectionManager::file(path).with_init(|c| {
            c.execute_batch(
                "
            PRAGMA foreign_keys = ON;
            PRAGMA journal_mode = WAL;
            PRAGMA synchronous = NORMAL;
            PRAGMA busy_timeout = 5000;
            ",
            )?;
            Ok(())
        });

        {
            let mut conn = manager
                .connect()
                .map_err(|_| DatabaseError::FailedToConnectToDatabase)?;

            migrations::run(&mut conn)?;
        }

        let pool = Pool::builder()
            .max_size(4)
            .build(manager)
            .map_err(|_| DatabaseError::ConnectionPoolFailure)?;

        let _conn = pool
            .get()
            .map_err(|_| DatabaseError::PoolConnectionRetrievalFailure)?;

        Ok(Self { pool })
    }

    pub fn insert(&self, data: &impl Insert) -> Result<i64, DatabaseError> {
        let conn = self
            .pool
            .get()
            .map_err(|_| DatabaseError::PoolConnectionRetrievalFailure)?;
        let id: i64 = data.insert(&conn)?;
        Ok(id)
    }

    pub fn update_value<U, T>(
        &self,
        column: U::Column,
        new_value: T,
        id: i64,
        updater: U,
    ) -> Result<(), DatabaseError>
    where
        U: Update,
        T: ToSql,
    {
        let conn = self
            .pool
            .get()
            .map_err(|_| DatabaseError::PoolConnectionRetrievalFailure)?;
        updater.update(&conn, column, new_value, id)
    }

    pub fn remove_entry(&self, db_type: DatabaseTypes, id: i64) -> Result<(), DatabaseError> {
        if db_type == DatabaseTypes::LibraryElements {
            return Err(DatabaseError::LibrarySeriesException); // Prevent direct deletion from junction table
        } else {
            let conn = self
                .pool
                .get()
                .map_err(|_| DatabaseError::PoolConnectionRetrievalFailure)?;
            let table_name = db_type.get_table_name();
            let query = format!("DELETE FROM {} WHERE id = ?1", table_name);
            conn.execute(&query, rusqlite::params![id])
                .map_err(|_| DatabaseError::ConnectionExecutableFailure)?;
            Ok(())
        }
    }

    pub fn get_entry<T>(&self, id: i64) -> Result<T, DatabaseError>
    where
        T: Extract,
    {
        let conn = self
            .pool
            .get()
            .map_err(|_| DatabaseError::PoolConnectionRetrievalFailure)?;
        T::extract(&conn, id)?.ok_or(DatabaseError::EntryNotFound)
    }

    // Fetch many rows by id in a single query, each paired with its id.
    // Returns an empty Vec for an empty id list (no query is run).
    pub fn get_entries_batch<T>(&self, ids: &[i64]) -> Result<Vec<WithId<T>>, DatabaseError>
    where
        T: Extract + Serialize,
    {
        let conn = self
            .pool
            .get()
            .map_err(|_| DatabaseError::PoolConnectionRetrievalFailure)?;
        T::extract_batch(&conn, ids)
    }

    pub fn get_all_libraries(&self) -> Result<Vec<i64>, DatabaseError> {
        let conn = self
            .pool
            .get()
            .map_err(|_| DatabaseError::PoolConnectionRetrievalFailure)?;
        let mut stmt = conn
            .prepare("SELECT id FROM library")
            .map_err(|_| DatabaseError::TaskPreparationFailure)?;
        let rows = stmt
            .query_map([], |row| row.get::<_, i64>("id"))
            .map_err(|_| DatabaseError::QueryFailure)?;
        rows.collect::<Result<Vec<i64>, _>>()
            .map_err(|_| DatabaseError::NextRowFailure)
    }

    pub fn get_books_in_series(&self, series_id: i64) -> Result<Vec<i64>, DatabaseError> {
        let conn = self
            .pool
            .get()
            .map_err(|_| DatabaseError::PoolConnectionRetrievalFailure)?;
        let mut stmt = conn
            .prepare("SELECT id FROM books WHERE series = ?1")
            .map_err(|_| DatabaseError::TaskPreparationFailure)?;
        let rows = stmt
            .query_map(rusqlite::params![series_id], |row| row.get::<_, i64>("id"))
            .map_err(|_| DatabaseError::QueryFailure)?;
        rows.collect::<Result<Vec<i64>, _>>()
            .map_err(|_| DatabaseError::NextRowFailure)
    }

    pub fn remove_entry_from_library_elements(
        &self,
        series_id: i64,
        library_id: i64,
    ) -> Result<(), DatabaseError> {
        let conn = self
            .pool
            .get()
            .map_err(|_| DatabaseError::PoolConnectionRetrievalFailure)?;
        let query = "DELETE FROM library_elements WHERE series_id = ?1 AND library_id = ?2";
        conn.execute(&query, rusqlite::params![series_id, library_id])
            .map_err(|_| DatabaseError::ConnectionExecutableFailure)?;
        Ok(())
    }

    pub fn get_series_entries_in_library(
        &self,
        library_id: i64,
    ) -> Result<Vec<i64>, DatabaseError> {
        let conn = self
            .pool
            .get()
            .map_err(|_| DatabaseError::PoolConnectionRetrievalFailure)?;
        let mut stmt = conn
            .prepare("SELECT series_id FROM library_elements WHERE library_id = ?1")
            .map_err(|_| DatabaseError::OperationFailure)?;
        let rows = stmt
            .query_map(rusqlite::params![library_id], |row| {
                row.get::<_, i64>("series_id")
            })
            .map_err(|_| DatabaseError::OperationFailure)?;
        let series_ids: Result<Vec<i64>, _> = rows.collect();
        Ok(series_ids.map_err(|_| DatabaseError::OperationFailure)?)
    }

    pub fn get_id_from_table(
        &self,
        db_type: DatabaseTypes,
        column: impl Into<ColumnSelector>,
        value_to_search_for: &dyn rusqlite::ToSql,
    ) -> Result<Option<i64>, DatabaseError> {
        let conn = self
            .pool
            .get()
            .map_err(|_| DatabaseError::PoolConnectionRetrievalFailure)?;
        let table_name = db_type.get_table_name();
        let column_name = match column.into() {
            ColumnSelector::Book(col) => col.as_ref().to_string(),
            ColumnSelector::Library(col) => col.as_ref().to_string(),
            ColumnSelector::Series(col) => col.as_ref().to_string(),
            ColumnSelector::User(col) => col.as_ref().to_string(),
        };
        let query = format!("SELECT id FROM {} WHERE {} = ?1", table_name, column_name);
        let mut stmt = conn
            .prepare(&query)
            .map_err(|_| DatabaseError::TaskPreparationFailure)?;
        let mut rows = stmt
            .query(rusqlite::params![value_to_search_for])
            .map_err(|_| DatabaseError::QueryFailure)?;

        if let Some(row) = rows.next().map_err(|_| DatabaseError::NextRowFailure)? {
            let id: i64 = row.get(0).map_err(|_| DatabaseError::IdRetrievalFailure)?;
            Ok(Some(id))
        } else {
            Ok(None)
        }
    }

    pub fn setup_new_transaction<F, T>(&self, operation: F) -> Result<T, DatabaseError>
    where
        F: FnOnce(&Connection) -> Result<T>,
    {
        let mut conn = self
            .pool
            .get()
            .map_err(|_| DatabaseError::PoolConnectionRetrievalFailure)?;
        let tx = conn
            .transaction()
            .map_err(|_| DatabaseError::TransactionFailure)?;
        let result = operation(&tx).map_err(|_| DatabaseError::OperationFailure)?;
        tx.commit()
            .map_err(|_| DatabaseError::OperationCommitFailure)?;
        Ok(result)
    }
}
