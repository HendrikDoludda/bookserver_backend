use crate::database_related_scripts::db::logged;
use crate::error_types::DatabaseError;
use rusqlite::{Connection, OptionalExtension};

// A single migration: a version number, a human-readable label, and the SQL
// to run. Versions must be unique and applied in ascending order.
struct Migration {
    version: i64,
    name: &'static str,
    sql: &'static str,
}

// `include_str!` reads the file at COMPILE time and bakes its contents into
// the binary as a &'static str. That means the migrations/ folder does not
// need to ship next to the executable — the SQL travels inside it.
// The path is resolved relative to THIS source file.
const MIGRATIONS: &[Migration] = &[Migration {
    version: 1,
    name: "initial_schema",
    sql: include_str!("../../migrations/0001_initial_schema.sql"),
}];

// Apply every migration that has not yet been recorded in `schema_migrations`.
// Each migration runs inside its own transaction: if anything inside the SQL
// fails, SQLite rolls the whole thing back so the DB never ends up in a
// half-migrated state.
pub fn run(conn: &mut Connection) -> Result<(), DatabaseError> {
    // 1. Bookkeeping table. Created with the same idempotent style as the
    //    real tables, so this function is safe to call on every startup.
    conn.execute(
        "CREATE TABLE IF NOT EXISTS schema_migrations (
            version    INTEGER PRIMARY KEY,
            applied_at INTEGER NOT NULL
        )",
        [],
    )
    .map_err(logged(
        "creating",
        "schema_migrations",
        DatabaseError::DatabaseInitializationFailure,
    ))?;

    // 2. Walk the migrations in order and apply any that are missing.
    for m in MIGRATIONS {
        let already_applied = conn
            .query_row(
                "SELECT 1 FROM schema_migrations WHERE version = ?1",
                [m.version],
                |_| Ok(()),
            )
            .optional()
            .map_err(logged(
                "reading",
                "schema_migrations",
                DatabaseError::OperationFailure,
            ))?
            .is_some();

        if already_applied {
            continue;
        }

        let tx = conn.transaction().map_err(logged(
            "starting a transaction for migration",
            m.name,
            DatabaseError::TransactionFailure,
        ))?;

        tx.execute_batch(m.sql).map_err(logged(
            "applying migration",
            m.name,
            DatabaseError::DatabaseInitializationFailure,
        ))?;

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);

        tx.execute(
            "INSERT INTO schema_migrations (version, applied_at) VALUES (?1, ?2)",
            rusqlite::params![m.version, now],
        )
        .map_err(logged(
            "recording migration",
            m.name,
            DatabaseError::ConnectionExecutableFailure,
        ))?;

        tx.commit().map_err(logged(
            "committing migration",
            m.name,
            DatabaseError::OperationCommitFailure,
        ))?;

        tracing::info!("Applied migration {:04} {}", m.version, m.name);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{insert_user, memory_connection};
    use rusqlite::params;

    #[test]
    fn deleting_a_user_deletes_only_their_auth_rows() {
        let conn = memory_connection();
        let alice = insert_user(&conn, "alice");
        let bob = insert_user(&conn, "bob");

        for user_id in [alice, bob] {
            let unique = format!("for-{user_id}");
            conn.execute(
                "INSERT INTO email_codes (user_id, purpose, code_hash, expires_at, \
                 last_sent_at, sent_in_window, window_started_at) \
                 VALUES (?1, 'reset_password', ?2, 0, 0, 1, 0)",
                params![user_id, unique],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO sessions (user_id, device_id, device_name, platform, \
                 refresh_token_hashed, refresh_token_valid_until, session_token_hashed, \
                 session_token_valid_until, last_used_at) \
                 VALUES (?1, ?2, 'phone', 'ios', ?2, 0, ?2, 0, 0)",
                params![user_id, unique],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO recovery_codes (user_id, code_hash) VALUES (?1, ?2)",
                params![user_id, unique],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO totp (user_id, authentication_secret) VALUES (?1, 'secret')",
                params![user_id],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO login_challenges (user_id, token_hashed, expires_at, device_id, \
                 device_name, platform) VALUES (?1, ?2, 0, 'device', 'phone', 'ios')",
                params![user_id, unique],
            )
            .unwrap();
        }

        conn.execute("DELETE FROM users WHERE id = ?1", [alice])
            .unwrap();

        for table in [
            "email_codes",
            "sessions",
            "recovery_codes",
            "totp",
            "login_challenges",
        ] {
            let count = |user_id: i64| -> i64 {
                conn.query_row(
                    &format!("SELECT COUNT(*) FROM {table} WHERE user_id = ?1"),
                    [user_id],
                    |row| row.get(0),
                )
                .unwrap()
            };
            assert_eq!(
                count(alice),
                0,
                "{table} still has rows of the deleted user"
            );
            assert_eq!(count(bob), 1, "{table} lost rows of another user");
        }
    }

    /// Whether a column refuses NULL, read from the schema itself.
    fn refuses_null(conn: &Connection, table: &str, column: &str) -> bool {
        conn.query_row(
            "SELECT \"notnull\" FROM pragma_table_info(?1) WHERE name = ?2",
            [table, column],
            |row| row.get(0),
        )
        .unwrap_or_else(|err| panic!("{table}.{column}: {err}"))
    }

    #[test]
    fn a_book_table_column_allows_null_exactly_when_its_rust_field_is_an_option() {
        let conn = memory_connection();
        let required = [
            ("books", "title"),
            ("books", "author"),
            ("books", "format"),
            ("books", "language"),
            ("books", "tags"),
            ("books", "file_path"),
            ("library", "library_name"),
            ("library", "library_type"),
            ("series", "name"),
        ];
        let optional = [
            ("books", "cover_image"),
            ("books", "description"),
            ("books", "page_count"),
            ("books", "volume_number"),
            ("books", "chapter_number"),
            ("books", "page_number"),
            ("books", "file_hash"),
            ("books", "last_modified"),
            ("books", "file_size"),
            ("books", "series"),
            ("library", "cover_image"),
            ("library", "description"),
            ("series", "description"),
            ("series", "cover_image"),
            ("series", "start_release_year"),
            ("series", "end_release_year"),
        ];
        for (table, column) in required {
            assert!(
                refuses_null(&conn, table, column),
                "{table}.{column} must refuse NULL"
            );
        }
        for (table, column) in optional {
            assert!(
                !refuses_null(&conn, table, column),
                "{table}.{column} must allow NULL"
            );
        }
        // A new column has to be sorted into one of the lists above.
        for table in ["books", "library", "series"] {
            let columns: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM pragma_table_info(?1) WHERE name != 'id'",
                    [table],
                    |row| row.get(0),
                )
                .unwrap();
            let listed = required
                .iter()
                .chain(&optional)
                .filter(|(listed_table, _)| *listed_table == table)
                .count();
            assert_eq!(columns, listed as i64, "every column of {table} is listed");
        }
    }

    #[test]
    fn running_the_migrations_again_changes_nothing() {
        let mut conn = memory_connection();
        run(&mut conn).unwrap();

        let applied: i64 = conn
            .query_row("SELECT COUNT(*) FROM schema_migrations", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(applied, MIGRATIONS.len() as i64);
    }
}
