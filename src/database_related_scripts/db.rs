use crate::data_models::models::ColumnSelector;
use crate::data_models::models::DatabaseTypes;
use crate::data_models::models::WithId;
use crate::database_related_scripts::db_search::Search;
use crate::database_related_scripts::db_update::Update;
use crate::database_related_scripts::extract::Extract;
use crate::database_related_scripts::insert::Insert;
use crate::database_related_scripts::migrations;
use crate::error_types::AppErrors;
use crate::error_types::DatabaseError;
use crate::models::QuerySeparator;
use crate::models::SelectionMethod;
use r2d2::{ManageConnection, Pool};
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::{Connection, Result, ToSql};
use serde::Serialize;
use std::path::Path;
use std::time::SystemTime;
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
        Self::insert_with_connection(&conn, data)
    }

    pub fn insert_with_connection(
        conn: &Connection,
        data: &impl Insert,
    ) -> Result<i64, DatabaseError> {
        let id: i64 = data.insert(&conn)?;
        Ok(id)
    }

    pub fn update_value<U>(
        &self,
        column: U::Column,
        new_value: &dyn ToSql,
        id: i64,
    ) -> Result<(), DatabaseError>
    where
        U: Update,
    {
        let conn = self
            .pool
            .get()
            .map_err(|_| DatabaseError::PoolConnectionRetrievalFailure)?;
        Self::update_value_with_connection::<U>(&conn, column, new_value, id)
    }

    pub fn update_value_with_connection<U>(
        conn: &Connection,
        column: U::Column,
        new_value: &dyn ToSql,
        id: i64,
    ) -> Result<(), DatabaseError>
    where
        U: Update,
    {
        U::update(&conn, column, new_value, id)
    }

    pub fn update_multiple_values<U>(
        &self,
        columns: &[U::Column],
        new_values: &[&dyn ToSql],
        id: i64,
    ) -> Result<(), DatabaseError>
    where
        U: Update,
    {
        let conn = self
            .pool
            .get()
            .map_err(|_| DatabaseError::PoolConnectionRetrievalFailure)?;
        Self::update_multiple_values_with_connection::<U>(&conn, columns, new_values, id)
    }

    pub fn update_multiple_values_with_connection<U>(
        conn: &Connection,
        columns: &[U::Column],
        new_values: &[&dyn ToSql],
        id: i64,
    ) -> Result<(), DatabaseError>
    where
        U: Update,
    {
        U::update_multiple(&conn, columns, new_values, id)
    }

    pub fn remove_entry(&self, db_type: DatabaseTypes, id: i64) -> Result<(), DatabaseError> {
        let conn = self
            .pool
            .get()
            .map_err(|_| DatabaseError::PoolConnectionRetrievalFailure)?;
        Self::remove_entry_with_connection(&conn, db_type, id)
    }

    pub fn remove_entry_with_connection(
        conn: &Connection,
        db_type: DatabaseTypes,
        id: i64,
    ) -> Result<(), DatabaseError> {
        if db_type == DatabaseTypes::LibraryElements {
            return Err(DatabaseError::LibrarySeriesException); // Prevent direct deletion from junction table
        } else {
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
        Self::get_entry_with_connection::<T>(&conn, id)
    }

    pub fn get_entry_with_connection<T>(conn: &Connection, id: i64) -> Result<T, DatabaseError>
    where
        T: Extract,
    {
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
        Self::get_entries_batch_with_connection::<T>(&conn, ids)
    }

    pub fn get_entries_batch_with_connection<T>(
        conn: &Connection,
        ids: &[i64],
    ) -> Result<Vec<WithId<T>>, DatabaseError>
    where
        T: Extract + Serialize,
    {
        T::extract_batch(conn, ids)
    }

    pub fn search_for_single_row<T>(
        &self,
        columns: &[T::Column],
        values: &[&dyn ToSql],
        query_separator: QuerySeparator,
        selection_method: SelectionMethod,
    ) -> Result<Option<T>, DatabaseError>
    where
        T: Search,
    {
        let conn = self
            .pool
            .get()
            .map_err(|_| DatabaseError::PoolConnectionRetrievalFailure)?;
        Self::search_for_single_row_with_connection::<T>(
            &conn,
            columns,
            values,
            query_separator,
            selection_method,
        )
    }

    pub fn search_for_single_row_with_connection<T>(
        conn: &Connection,
        columns: &[T::Column],
        values: &[&dyn ToSql],
        query_separator: QuerySeparator,
        selection_method: SelectionMethod,
    ) -> Result<Option<T>, DatabaseError>
    where
        T: Search,
    {
        T::search_for_row(&conn, columns, values, query_separator, selection_method)
    }

    pub fn search_for_multiple_rows<T>(
        &self,
        columns: &[T::Column],
        values: &[&dyn ToSql],
        query_separator: QuerySeparator,
        selection_method: SelectionMethod,
    ) -> Result<Vec<T>, DatabaseError>
    where
        T: Search,
    {
        let conn = self
            .pool
            .get()
            .map_err(|_| DatabaseError::PoolConnectionRetrievalFailure)?;
        Self::search_for_multiple_rows_with_connection::<T>(
            &conn,
            columns,
            values,
            query_separator,
            selection_method,
        )
    }

    pub fn search_for_multiple_rows_with_connection<T>(
        conn: &Connection,
        columns: &[T::Column],
        values: &[&dyn ToSql],
        query_separator: QuerySeparator,
        selection_method: SelectionMethod,
    ) -> Result<Vec<T>, DatabaseError>
    where
        T: Search,
    {
        T::search_for_multiple_rows(&conn, columns, values, query_separator, selection_method)
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
        Self::remove_entry_from_library_elements_with_connection(&conn, series_id, library_id)
    }

    pub fn remove_entry_from_library_elements_with_connection(
        conn: &Connection,
        series_id: i64,
        library_id: i64,
    ) -> Result<(), DatabaseError> {
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
        Self::get_series_entries_in_library_with_connection(&conn, library_id)
    }

    pub fn get_series_entries_in_library_with_connection(
        conn: &Connection,
        library_id: i64,
    ) -> Result<Vec<i64>, DatabaseError> {
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
        //should not be used
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

    pub fn setup_new_transaction<F, T>(&self, operation: F) -> Result<T, AppErrors>
    where
        F: FnOnce(&Connection) -> Result<T, AppErrors>,
    {
        let mut conn = self.pool.get().map_err(|err| {
            log::error!("Got an error when trying to set up a new connection: {err}");
            DatabaseError::PoolConnectionRetrievalFailure
        })?;
        let tx = conn.transaction().map_err(|err| {
            log::error!("Failed to set up a transaction: {err}");
            DatabaseError::TransactionFailure
        })?;
        let result = operation(&tx).map_err(|err| {
            log::error!("Failure while executing the transaction: {err}");
            DatabaseError::OperationFailure
        })?;
        tx.commit().map_err(|err| {
            log::error!("Failure while commiting the transaction: {err}");
            DatabaseError::OperationCommitFailure
        })?;
        Ok(result)
    }
}

pub fn convert_system_time_to_unix_time(time: SystemTime) -> i64 {
    Some(time)
        .map(|t| t.duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64)
        .unwrap()
}
