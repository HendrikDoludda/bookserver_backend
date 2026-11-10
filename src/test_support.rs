//! Shared fixtures for the unit tests.

use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use rusqlite::{params, Connection};
use uuid::Uuid;

use crate::{
    app_state::AppState,
    database_related_scripts::{
        db::{configure, Database},
        migrations,
    },
    models::{EmailConfig, SmtpTls},
    routes::{
        email_helper::{EmailInformation, Mailer},
        registration::SetupCode,
        totp_cipher::TotpCipher,
    },
};

/// The setup code of every test router; first sign-ups in tests send it.
pub const TEST_SETUP_CODE: &str = "SETUP234";

/// The TOTP cipher of every test router.
pub fn test_cipher() -> TotpCipher {
    TotpCipher::from_signing_key(&[7; 32])
}

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

/// The code in a mail from `email_codes::code_mail`: the line indented by four spaces.
pub fn code_in(mail: &EmailInformation) -> String {
    mail.body
        .lines()
        .find_map(|line| line.strip_prefix("    "))
        .expect("the mail carries a code")
        .to_string()
}

/// Runs `f` and answers everything it logged through `tracing` on this thread, as plain text.
pub fn capture_logs(f: impl FnOnce()) -> String {
    let buffer = Arc::new(Mutex::new(Vec::new()));
    let writer = {
        let buffer = Arc::clone(&buffer);
        move || LogBuffer(Arc::clone(&buffer))
    };
    let subscriber = tracing_subscriber::fmt()
        .with_writer(writer)
        .with_ansi(false)
        .finish();
    tracing::subscriber::with_default(subscriber, f);
    let bytes = buffer.lock().unwrap().clone();
    String::from_utf8(bytes).expect("log lines are UTF-8")
}

/// The writer behind `capture_logs`: every line lands in the shared buffer.
struct LogBuffer(Arc<Mutex<Vec<u8>>>);

impl Write for LogBuffer {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// A mailer whose SMTP server refuses every connection, for the paths where sending fails.
pub fn unreachable_mailer() -> Mailer {
    // Bind and release a port, so nothing is listening on it.
    let port = std::net::TcpListener::bind("127.0.0.1:0")
        .expect("bind a free port")
        .local_addr()
        .expect("read the port")
        .port();
    Mailer::from_config(Some(EmailConfig {
        host: "127.0.0.1".to_string(),
        port,
        tls: SmtpTls::Off,
        credentials: None,
        from: "server@example.com".to_string(),
        from_name: "Book Server".to_string(),
    }))
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

    /// Router state on this database, with the given mailer, `TEST_SETUP_CODE` and
    /// `test_cipher()`.
    pub fn state(&self, mailer: Mailer) -> AppState {
        AppState {
            db: Arc::clone(&self.db),
            mailer,
            setup_code: SetupCode::of(TEST_SETUP_CODE),
            totp_cipher: test_cipher(),
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
