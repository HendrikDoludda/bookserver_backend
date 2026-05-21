use crate::error_types::DatabaseError;
use rusqlite::Connection;

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
    .map_err(|_| DatabaseError::DatabaseInitializationFailure)?;

    // 2. Walk the migrations in order and apply any that are missing.
    for m in MIGRATIONS {
        let already_applied = match conn.query_row(
            "SELECT 1 FROM schema_migrations WHERE version = ?1",
            [m.version],
            |_| Ok(()),
        ) {
            Ok(()) => true,
            Err(rusqlite::Error::QueryReturnedNoRows) => false,
            Err(_) => return Err(DatabaseError::OperationFailure),
        };

        if already_applied {
            continue;
        }

        let tx = conn
            .transaction()
            .map_err(|_| DatabaseError::TransactionFailure)?;

        tx.execute_batch(m.sql)
            .map_err(|_| DatabaseError::DatabaseInitializationFailure)?;

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);

        tx.execute(
            "INSERT INTO schema_migrations (version, applied_at) VALUES (?1, ?2)",
            rusqlite::params![m.version, now],
        )
        .map_err(|_| DatabaseError::ConnectionExecutableFailure)?;

        tx.commit()
            .map_err(|_| DatabaseError::OperationCommitFailure)?;

        tracing::info!("Applied migration {:04} {}", m.version, m.name);
    }

    Ok(())
}
