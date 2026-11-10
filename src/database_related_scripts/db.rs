use std::path::Path;
use std::sync::Arc;
use std::time::SystemTime;

use r2d2::{ManageConnection, Pool};
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::{Connection, Result, ToSql, TransactionBehavior};
use serde::Serialize;

use crate::data_models::models::{ColumnSelector, DatabaseTypes, QuerySeparator, WithId};
use crate::database_related_scripts::{
    db_delete::Delete, db_search::Search, db_update::Update, extract::Extract, insert::Insert,
    migrations,
};
use crate::error_types::{AppErrors, DatabaseError};

/// Handle to the SQLite database: an r2d2 connection pool plus every query the
/// app runs. Most operations come in two flavours: a plain method that takes a
/// connection from the pool, and a `*_with_connection` variant that runs on a
/// caller-supplied connection so several operations can share one transaction
/// (see [`Database::setup_new_transaction`]).
// TODO: decide whether filtering happens here or in the frontend. Leaning towards
// the backend: pass filter options in with the request and apply them in the query.
pub struct Database {
    pool: r2d2::Pool<SqliteConnectionManager>,
}

/// Settings every connection needs. SQLite applies them per connection, not per file,
/// so a connection that skips this runs without foreign keys.
pub fn configure(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "
        PRAGMA foreign_keys = ON;
        PRAGMA journal_mode = WAL;
        PRAGMA synchronous = NORMAL;
        PRAGMA busy_timeout = 5000;
        ",
    )
}

impl Database {
    pub fn new() -> Result<Self, DatabaseError> {
        // Ensure the database folder exists
        std::fs::create_dir_all("./data/databases/")
            .map_err(|_| DatabaseError::DirectoryCreationFailure)?;

        Self::open(Path::new("./data/databases/app_data.sqlite"))
    }

    /// Opens (or creates) the database at `path` and applies any pending migrations.
    pub fn open(path: &Path) -> Result<Self, DatabaseError> {
        let manager = SqliteConnectionManager::file(path).with_init(|conn| configure(conn));

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
        let id: i64 = data.insert(conn)?;
        Ok(id)
    }

    /// Returns the number of rows changed: 0 when no row has that key.
    pub fn update_value<U>(
        &self,
        column: U::Column,
        new_value: &dyn ToSql,
        id: i64,
    ) -> Result<usize, DatabaseError>
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
    ) -> Result<usize, DatabaseError>
    where
        U: Update,
    {
        U::update(conn, column, new_value, id)
    }

    pub fn update_multiple_values<U>(
        &self,
        columns: &[U::Column],
        new_values: &[&dyn ToSql],
        id: i64,
    ) -> Result<usize, DatabaseError>
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
    ) -> Result<usize, DatabaseError>
    where
        U: Update,
    {
        U::update_multiple(conn, columns, new_values, id)
    }

    /// Deletes the row whose key column equals `id`; returns the number of rows deleted.
    pub fn remove_entry(&self, db_type: DatabaseTypes, id: i64) -> Result<usize, DatabaseError> {
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
    ) -> Result<usize, DatabaseError> {
        // The junction table has no single key; use `remove_entry_from_library_elements`.
        let key = db_type
            .get_key_column()
            .ok_or(DatabaseError::LibrarySeriesException)?;
        let query = format!(
            "DELETE FROM {} WHERE {} = ?1",
            db_type.get_table_name(),
            key
        );
        conn.execute(&query, rusqlite::params![id])
            .map_err(|_| DatabaseError::ConnectionExecutableFailure)
    }

    pub fn remove_entry_with_connection_based_on_columns<T>(
        conn: &Connection,
        columns: &[T::Column],
        values: &[&dyn ToSql],
        query_separator: QuerySeparator,
    ) -> Result<usize, DatabaseError>
    where
        T: Delete,
    {
        T::delete(conn, columns, values, query_separator)
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
        T::extract(conn, id)?.ok_or(DatabaseError::EntryNotFound)
    }

    /// Fetch many rows by id in a single query, each paired with its id.
    /// Returns an empty Vec for an empty id list (no query is run).
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
    ) -> Result<Option<T>, DatabaseError>
    where
        T: Search,
    {
        let conn = self
            .pool
            .get()
            .map_err(|_| DatabaseError::PoolConnectionRetrievalFailure)?;
        Self::search_for_single_row_with_connection::<T>(&conn, columns, values, query_separator)
    }

    pub fn search_for_single_row_with_connection<T>(
        conn: &Connection,
        columns: &[T::Column],
        values: &[&dyn ToSql],
        query_separator: QuerySeparator,
    ) -> Result<Option<T>, DatabaseError>
    where
        T: Search,
    {
        T::search_for_row(conn, columns, values, query_separator)
    }

    pub fn search_for_multiple_rows<T>(
        &self,
        columns: &[T::Column],
        values: &[&dyn ToSql],
        query_separator: QuerySeparator,
    ) -> Result<Vec<T>, DatabaseError>
    where
        T: Search,
    {
        let conn = self
            .pool
            .get()
            .map_err(|_| DatabaseError::PoolConnectionRetrievalFailure)?;
        Self::search_for_multiple_rows_with_connection::<T>(&conn, columns, values, query_separator)
    }

    pub fn search_for_multiple_rows_with_connection<T>(
        conn: &Connection,
        columns: &[T::Column],
        values: &[&dyn ToSql],
        query_separator: QuerySeparator,
    ) -> Result<Vec<T>, DatabaseError>
    where
        T: Search,
    {
        T::search_for_multiple_rows(conn, columns, values, query_separator)
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
        conn.execute(query, rusqlite::params![series_id, library_id])
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
        rows.collect::<Result<Vec<i64>, _>>()
            .map_err(|_| DatabaseError::OperationFailure)
    }

    /// Looks up a row id by a single column value.
    /// Legacy: new code should not use this; it remains for the folder scanner.
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

    /// Runs `operation` inside a single SQLite transaction. It commits if the closure
    /// returns `Ok`; on `Err` the transaction is dropped, which rolls it back, and the
    /// closure's own error is returned unchanged.
    pub fn setup_new_transaction<F, T>(&self, operation: F) -> Result<T, AppErrors>
    where
        F: FnOnce(&Connection) -> Result<T, AppErrors>,
    {
        let mut conn = self.pool.get().map_err(|err| {
            log::error!("Got an error when trying to set up a new connection: {err}");
            DatabaseError::PoolConnectionRetrievalFailure
        })?;
        // IMMEDIATE takes the write lock up front. A deferred transaction that reads and
        // then writes fails at once with SQLITE_BUSY_SNAPSHOT if another connection wrote
        // in between, and the busy timeout can't help it.
        let tx = conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|err| {
                log::error!("Failed to set up a transaction: {err}");
                DatabaseError::TransactionFailure
            })?;
        let result = operation(&tx)?;
        tx.commit().map_err(|err| {
            log::error!("Failure while commiting the transaction: {err}");
            DatabaseError::OperationCommitFailure
        })?;
        Ok(result)
    }

    /// [`Database::setup_new_transaction`] for async handlers. SQLite calls block the
    /// thread, so they run on Tokio's blocking pool instead of an async worker thread.
    pub async fn transact<F, T>(self: Arc<Self>, operation: F) -> Result<T, AppErrors>
    where
        F: FnOnce(&Connection) -> Result<T, AppErrors> + Send + 'static,
        T: Send + 'static,
    {
        tokio::task::spawn_blocking(move || self.setup_new_transaction(operation))
            .await
            .map_err(|err| {
                log::error!("The transaction task did not finish: {err}");
                DatabaseError::TransactionFailure
            })?
    }

    /// Runs read-only work on a pooled connection on Tokio's blocking pool. No transaction:
    /// an `IMMEDIATE` one takes the write lock, and every request's session lookup would
    /// queue behind every write.
    pub async fn read<F, T>(self: Arc<Self>, operation: F) -> Result<T, AppErrors>
    where
        F: FnOnce(&Connection) -> Result<T, AppErrors> + Send + 'static,
        T: Send + 'static,
    {
        tokio::task::spawn_blocking(move || {
            let conn = self.pool.get().map_err(|err| {
                log::error!("Got an error when trying to set up a new connection: {err}");
                DatabaseError::PoolConnectionRetrievalFailure
            })?;
            operation(&conn)
        })
        .await
        .map_err(|err| {
            log::error!("The read task did not finish: {err}");
            DatabaseError::TransactionFailure
        })?
    }
}

/// Converts a `SystemTime` to whole seconds since the Unix epoch, the format every
/// timestamp column uses. Panics for times before 1970.
pub fn convert_system_time_to_unix_time(time: SystemTime) -> i64 {
    checked_unix_time(time).expect("the server's own times lie after 1970")
}

/// Whole seconds since the Unix epoch, or `None` for a time before it — for times the
/// server doesn't make itself, such as a file's modification time.
pub fn checked_unix_time(time: SystemTime) -> Option<i64> {
    time.duration_since(std::time::UNIX_EPOCH)
        .ok()
        .map(|age| age.as_secs() as i64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error_types::AuthenticationError;
    use crate::test_support::{insert_user, memory_connection, TempDatabase};

    fn count_users(db: &Database) -> i64 {
        let conn = db.pool.get().unwrap();
        conn.query_row("SELECT COUNT(*) FROM users", [], |row| row.get(0))
            .unwrap()
    }

    fn foreign_keys_setting(conn: &Connection) -> i64 {
        conn.query_row("PRAGMA foreign_keys", [], |row| row.get(0))
            .unwrap()
    }

    #[test]
    fn test_connections_enforce_foreign_keys() {
        let conn = memory_connection();
        assert_eq!(foreign_keys_setting(&conn), 1);
        let orphan = conn.execute(
            "INSERT INTO totp (user_id, authentication_secret) VALUES (999, 'secret')",
            [],
        );
        assert!(orphan.is_err(), "a row for a missing user must be refused");
    }

    #[test]
    fn pooled_connections_enforce_foreign_keys() {
        let temp = TempDatabase::open();
        let conn = temp.db.pool.get().unwrap();
        assert_eq!(foreign_keys_setting(&conn), 1);
    }

    #[test]
    fn failed_transaction_rolls_back_and_returns_its_own_error() {
        let temp = TempDatabase::open();
        let result: Result<(), AppErrors> = temp.db.setup_new_transaction(|conn| {
            insert_user(conn, "alice");
            Err(AuthenticationError::IncorrectCredentials.into())
        });

        assert!(matches!(
            result,
            Err(AppErrors::Authentication(
                AuthenticationError::IncorrectCredentials
            ))
        ));
        assert_eq!(count_users(&temp.db), 0);
    }

    #[test]
    fn transactions_take_the_write_lock_before_their_first_statement() {
        let temp = TempDatabase::open();
        temp.db
            .setup_new_transaction(|_conn| {
                let other = Connection::open(temp.path()).unwrap();
                other.busy_timeout(std::time::Duration::ZERO).unwrap();

                let blocked = other.execute_batch("BEGIN IMMEDIATE; COMMIT;");
                assert!(
                    matches!(
                        blocked,
                        Err(rusqlite::Error::SqliteFailure(ref e, _))
                            if e.code == rusqlite::ErrorCode::DatabaseBusy
                    ),
                    "another writer must already be locked out, got {blocked:?}"
                );
                Ok(())
            })
            .unwrap();
    }

    #[test]
    fn successful_transaction_commits() {
        let temp = TempDatabase::open();
        let id = temp
            .db
            .setup_new_transaction(|conn| Ok(insert_user(conn, "alice")))
            .unwrap();

        assert!(id > 0);
        assert_eq!(count_users(&temp.db), 1);
    }

    #[tokio::test]
    async fn transact_commits_on_ok_and_returns_errors_unchanged() {
        let temp = TempDatabase::open();

        let id = temp
            .db
            .clone()
            .transact(|conn| Ok(insert_user(conn, "alice")))
            .await
            .unwrap();
        assert!(id > 0);

        let failed: Result<(), AppErrors> = temp
            .db
            .clone()
            .transact(|conn| {
                insert_user(conn, "bob");
                Err(AuthenticationError::IncorrectCredentials.into())
            })
            .await;
        assert!(matches!(
            failed,
            Err(AppErrors::Authentication(
                AuthenticationError::IncorrectCredentials
            ))
        ));

        assert_eq!(count_users(&temp.db), 1);
    }

    #[test]
    fn remove_entry_deletes_by_the_tables_key_column() {
        let conn = memory_connection();
        let user_id = insert_user(&conn, "alice");
        conn.execute(
            "INSERT INTO totp (user_id, authentication_secret) VALUES (?1, 'secret')",
            [user_id],
        )
        .unwrap();

        let removed =
            Database::remove_entry_with_connection(&conn, DatabaseTypes::Totp, user_id).unwrap();
        assert_eq!(removed, 1);

        let removed_again =
            Database::remove_entry_with_connection(&conn, DatabaseTypes::Totp, user_id).unwrap();
        assert_eq!(removed_again, 0);
    }

    #[test]
    fn a_time_before_1970_has_no_unix_time() {
        let epoch = std::time::UNIX_EPOCH;
        assert_eq!(checked_unix_time(epoch), Some(0));
        assert_eq!(
            checked_unix_time(epoch + std::time::Duration::from_secs(5)),
            Some(5)
        );
        assert_eq!(
            checked_unix_time(epoch - std::time::Duration::from_secs(1)),
            None
        );
    }

    #[test]
    fn remove_entry_refuses_the_junction_table() {
        let conn = memory_connection();
        let result =
            Database::remove_entry_with_connection(&conn, DatabaseTypes::LibraryElements, 1);
        assert!(matches!(result, Err(DatabaseError::LibrarySeriesException)));
    }
}
