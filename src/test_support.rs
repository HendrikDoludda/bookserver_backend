//! Shared fixtures for the unit tests.

use std::{
    fs,
    path::{Path, PathBuf},
    sync::Arc,
};

use rusqlite::{params, Connection};
use uuid::Uuid;

use crate::{
    app_state::AppState,
    database_related_scripts::{
        db::{configure, Database},
        migrations,
    },
    routes::email_helper::Mailer,
};

/// A fresh in-memory database with every migration applied.
pub fn memory_connection() -> Connection {
    let mut conn = Connection::open_in_memory().expect("open an in-memory database");
    configure(&conn).expect("configure the connection");
    migrations::run(&mut conn).expect("run the migrations");
    conn
}

/// Inserts a user with plain SQL, so fixtures don't depend on the model code under test.
pub fn insert_user(conn: &Connection, username: &str) -> i64 {
    conn.execute(
        "INSERT INTO users (username, password_hash, email) VALUES (?1, 'not-a-hash', ?2)",
        params![username, format!("{username}@example.com")],
    )
    .expect("insert a user");
    conn.last_insert_rowid()
}

/// A pooled `Database` in a temp file, deleted when the value is dropped. A plain
/// `:memory:` path won't do: every pooled connection would get its own empty database.
pub struct TempDatabase {
    pub db: Arc<Database>,
    // Fields drop in declaration order, so the pool closes before the file is deleted.
    _file: TempFile,
}

impl TempDatabase {
    pub fn open() -> Self {
        let path = std::env::temp_dir().join(format!("bookserver-test-{}.sqlite", Uuid::new_v4()));
        let db = Database::open(&path).expect("open a temp-file database");
        Self {
            db: Arc::new(db),
            _file: TempFile(path),
        }
    }

    pub fn path(&self) -> &Path {
        &self._file.0
    }

    /// Router state on this database, with the given mailer.
    pub fn state(&self, mailer: Mailer) -> AppState {
        AppState {
            db: Arc::clone(&self.db),
            mailer,
        }
    }
}

struct TempFile(PathBuf);

impl Drop for TempFile {
    fn drop(&mut self) {
        for suffix in ["", "-wal", "-shm"] {
            let mut path = self.0.clone().into_os_string();
            path.push(suffix);
            let _ = fs::remove_file(path);
        }
    }
}
