use crate::{
    data_models::models::{
        BookMetadata, BookSeriesMetadata, LibraryMetadata, RecoveryCodes, Sessions, UserMetadata,
        UserSettings, WithId, TOTP,
    },
    database_related_scripts::{db::logged, db_from_row::FromRow},
    error_types::DatabaseError,
};
use rusqlite::{params, params_from_iter, Connection, OptionalExtension};
use serde::Serialize;

/// Read rows by their `KEY` column into a model. Each type supplies `TABLE`, `KEY`
/// and `COLUMNS` (which must include `KEY`); the default methods assemble the queries.
pub trait Extract: FromRow {
    const TABLE: &'static str;
    const KEY: &'static str;
    const COLUMNS: &'static str;

    /// Fetch a single row by its key.
    fn extract(conn: &Connection, id: i64) -> Result<Option<Self>, DatabaseError> {
        let query = format!(
            "SELECT {} FROM {} WHERE {} = ?1",
            Self::COLUMNS,
            Self::TABLE,
            Self::KEY
        );

        conn.query_row(&query, params![id], Self::from_row)
            .optional()
            .map_err(logged("reading", Self::TABLE, DatabaseError::QueryFailure))
    }

    /// Fetch many rows by key in a single query. Returns each row paired with its
    /// key; keys that don't exist are simply absent from the result.
    fn extract_batch(conn: &Connection, ids: &[i64]) -> Result<Vec<WithId<Self>>, DatabaseError>
    where
        Self: Serialize,
    {
        if ids.is_empty() {
            return Ok(Vec::new()); // `IN ()` is invalid SQL, so bail early.
        }

        // One placeholder per id: "?1, ?2, ?3".
        let placeholders = (1..=ids.len())
            .map(|i| format!("?{i}"))
            .collect::<Vec<_>>()
            .join(", ");

        let query = format!(
            "SELECT {} FROM {} WHERE {} IN ({})",
            Self::COLUMNS,
            Self::TABLE,
            Self::KEY,
            placeholders
        );

        let mut stmt = conn.prepare(&query).map_err(logged(
            "reading",
            Self::TABLE,
            DatabaseError::TaskPreparationFailure,
        ))?;

        let rows = stmt
            .query_map(params_from_iter(ids), |row| {
                Ok(WithId {
                    id: row.get(Self::KEY)?,
                    data: Self::from_row(row)?,
                })
            })
            .map_err(logged("reading", Self::TABLE, DatabaseError::QueryFailure))?;

        rows.collect::<Result<Vec<_>, _>>().map_err(logged(
            "reading",
            Self::TABLE,
            DatabaseError::NextRowFailure,
        ))
    }

    /// Fetch every row of the table in key order, each paired with its key, in one query.
    fn extract_all(conn: &Connection) -> Result<Vec<WithId<Self>>, DatabaseError>
    where
        Self: Serialize,
    {
        let query = format!(
            "SELECT {} FROM {} ORDER BY {}",
            Self::COLUMNS,
            Self::TABLE,
            Self::KEY
        );

        let mut stmt = conn.prepare(&query).map_err(logged(
            "reading",
            Self::TABLE,
            DatabaseError::TaskPreparationFailure,
        ))?;

        let rows = stmt
            .query_map([], |row| {
                Ok(WithId {
                    id: row.get(Self::KEY)?,
                    data: Self::from_row(row)?,
                })
            })
            .map_err(logged("reading", Self::TABLE, DatabaseError::QueryFailure))?;

        rows.collect::<Result<Vec<_>, _>>().map_err(logged(
            "reading",
            Self::TABLE,
            DatabaseError::NextRowFailure,
        ))
    }
}

impl Extract for BookMetadata {
    const TABLE: &'static str = "books";
    const KEY: &'static str = "id";
    const COLUMNS: &'static str = "id, title, author, format, language, cover_image, \
        description, tags, file_path, page_count, volume_number, chapter_number, \
        page_number, file_hash, last_modified, file_size, series";
}

impl Extract for BookSeriesMetadata {
    const TABLE: &'static str = "series";
    const KEY: &'static str = "id";
    const COLUMNS: &'static str =
        "id, name, description, cover_image, start_release_year, end_release_year";
}

impl Extract for LibraryMetadata {
    const TABLE: &'static str = "library";
    const KEY: &'static str = "id";
    const COLUMNS: &'static str = "id, library_name, library_type, cover_image, description";
}

impl Extract for UserMetadata {
    const TABLE: &'static str = "users";
    const KEY: &'static str = "id";
    const COLUMNS: &'static str =
        "id, username, password_hash, email, email_verified, role, created_at, last_login";
}

impl Extract for TOTP {
    const TABLE: &'static str = "totp";
    const KEY: &'static str = "user_id";
    const COLUMNS: &'static str = "user_id, authentication_secret, confirmed, last_used_step, \
        failed_attempts, lockouts, last_failed_at, created_at";
}

impl Extract for UserSettings {
    const TABLE: &'static str = "user_settings";
    const KEY: &'static str = "user_id";
    const COLUMNS: &'static str = "user_id, locale";
}

impl Extract for RecoveryCodes {
    const TABLE: &'static str = "recovery_codes";
    const KEY: &'static str = "recovery_id";
    const COLUMNS: &'static str = "recovery_id, user_id, code_hash, used";
}

impl Extract for Sessions {
    const TABLE: &'static str = "sessions";
    const KEY: &'static str = "session_id";
    const COLUMNS: &'static str = "session_id, user_id, device_id, device_name, platform, \
        refresh_token_hashed, refresh_token_valid_until, session_token_hashed, \
        session_token_valid_until, created_at, last_used_at";
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{capture_logs, insert_user, memory_connection};

    #[test]
    fn an_unreadable_row_is_logged_once_per_read_with_its_table_and_cause() {
        let conn = memory_connection();
        let user_id = insert_user(&conn, "alice");
        conn.execute(
            "INSERT INTO totp (user_id, authentication_secret, created_at) \
             VALUES (?1, 'secret', 'not a time')",
            [user_id],
        )
        .unwrap();

        let mut results = Vec::new();
        let logs = capture_logs(|| {
            results.push(TOTP::extract(&conn, user_id).map(|_| ()));
            results.push(TOTP::extract_batch(&conn, &[user_id]).map(|_| ()));
            results.push(TOTP::extract_all(&conn).map(|_| ()));
        });

        assert!(matches!(results[0], Err(DatabaseError::QueryFailure)));
        assert!(matches!(results[1], Err(DatabaseError::NextRowFailure)));
        assert!(matches!(results[2], Err(DatabaseError::NextRowFailure)));
        assert_eq!(logs.matches("reading totp failed:").count(), 3, "{logs}");
        assert!(
            logs.contains("created_at"),
            "the cause names the column: {logs}"
        );
    }

    fn insert_totp(conn: &Connection, user_id: i64, secret: &str) {
        conn.execute(
            "INSERT INTO totp (user_id, authentication_secret, created_at) \
             VALUES (?1, ?2, 1700000000)",
            params![user_id, secret],
        )
        .unwrap();
    }

    #[test]
    fn extract_reads_by_the_tables_key_column() {
        let conn = memory_connection();
        let user_id = insert_user(&conn, "alice");
        insert_totp(&conn, user_id, "secret");

        let found = TOTP::extract(&conn, user_id)
            .unwrap()
            .expect("the row exists");
        assert_eq!(found.authentication_secret, "secret");

        assert!(TOTP::extract(&conn, user_id + 1).unwrap().is_none());
    }

    #[test]
    fn extract_batch_pairs_each_row_with_its_key() {
        let conn = memory_connection();
        let alice = insert_user(&conn, "alice");
        let bob = insert_user(&conn, "bob");
        insert_totp(&conn, alice, "alice-secret");
        insert_totp(&conn, bob, "bob-secret");

        let mut rows = TOTP::extract_batch(&conn, &[bob, alice, 999]).unwrap();
        rows.sort_by_key(|row| row.id);

        let found: Vec<(i64, &str)> = rows
            .iter()
            .map(|row| (row.id, row.data.authentication_secret.as_str()))
            .collect();
        assert_eq!(found, vec![(alice, "alice-secret"), (bob, "bob-secret")]);
    }

    #[test]
    fn extract_batch_of_nothing_runs_no_query() {
        let conn = memory_connection();
        assert!(TOTP::extract_batch(&conn, &[]).unwrap().is_empty());
    }

    #[test]
    fn extract_all_reads_every_row_in_key_order() {
        let conn = memory_connection();
        let alice = insert_user(&conn, "alice");
        let bob = insert_user(&conn, "bob");
        let carol = insert_user(&conn, "carol");
        // Written out of key order, so the order has to come from the query.
        for (user_id, secret) in [(carol, "c"), (alice, "a"), (bob, "b")] {
            insert_totp(&conn, user_id, secret);
        }

        let rows = TOTP::extract_all(&conn).unwrap();

        let found: Vec<(i64, &str)> = rows
            .iter()
            .map(|row| (row.id, row.data.authentication_secret.as_str()))
            .collect();
        assert_eq!(found, vec![(alice, "a"), (bob, "b"), (carol, "c")]);
    }

    #[test]
    fn extract_all_of_an_empty_table_is_empty() {
        let conn = memory_connection();
        assert!(TOTP::extract_all(&conn).unwrap().is_empty());
    }
}
