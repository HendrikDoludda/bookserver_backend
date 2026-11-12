-- ============= Setup Books Table =================
CREATE TABLE IF NOT EXISTS books (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    title TEXT NOT NULL,
    author TEXT NOT NULL,
    format TEXT NOT NULL,
    language TEXT NOT NULL,
    cover_image TEXT,
    description TEXT,
    tags TEXT NOT NULL,
    file_path TEXT NOT NULL UNIQUE,
    page_count INTEGER,
    volume_number INTEGER,
    chapter_number INTEGER,
    page_number INTEGER,
    file_hash TEXT,
    last_modified INTEGER,
    file_size INTEGER,
    series INTEGER
);

-- ============= Setup Library Table =================
CREATE TABLE IF NOT EXISTS library (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    library_name TEXT NOT NULL,
    library_type TEXT NOT NULL,
    cover_image TEXT,
    description TEXT
);

-- ============= Setup Library Elements Table =================
CREATE TABLE IF NOT EXISTS library_elements (
    library_id INTEGER NOT NULL,
    series_id INTEGER NOT NULL,
    PRIMARY KEY (library_id, series_id),
    FOREIGN KEY (library_id) REFERENCES library(id) ON DELETE CASCADE,
    FOREIGN KEY (series_id) REFERENCES series(id) ON DELETE CASCADE
);

-- ============= Setup Series Table =================
CREATE TABLE IF NOT EXISTS series (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    description TEXT,
    cover_image TEXT,
    start_release_year INTEGER,
    end_release_year INTEGER
);

-- ============= Setup Users Table =================
CREATE TABLE IF NOT EXISTS users (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    username TEXT NOT NULL UNIQUE COLLATE NOCASE,
    password_hash TEXT NOT NULL,
    email TEXT NOT NULL UNIQUE COLLATE NOCASE,
    email_verified BOOLEAN NOT NULL DEFAULT FALSE,
    role TEXT NOT NULL DEFAULT 'member' CHECK (role IN ('admin', 'member')),
    created_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
    last_login INTEGER
);

-- One live emailed code per user and purpose ('reset_password', 'verify_email').
-- The row outlives its code: it carries the counters for the resend limits.
CREATE TABLE IF NOT EXISTS email_codes (
    user_id INTEGER NOT NULL,
    purpose TEXT NOT NULL,
    code_hash TEXT NOT NULL,
    -- The address an email change waits to confirm; the code is mailed there.
    new_email TEXT,
    expires_at INTEGER NOT NULL,
    attempts INTEGER NOT NULL DEFAULT 0,
    last_sent_at INTEGER NOT NULL,
    sent_in_window INTEGER NOT NULL,
    window_started_at INTEGER NOT NULL,
    PRIMARY KEY (user_id, purpose),
    CHECK ((purpose = 'change_email') = (new_email IS NOT NULL)),
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);


CREATE TABLE IF NOT EXISTS sessions (
    session_id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id INTEGER NOT NULL,
    device_id TEXT NOT NULL,
    device_name TEXT NOT NULL,
    platform TEXT NOT NULL,
    refresh_token_hashed TEXT NOT NULL UNIQUE,
    refresh_token_valid_until INTEGER NOT NULL,
    session_token_hashed TEXT NOT NULL UNIQUE,
    session_token_valid_until INTEGER NOT NULL,
    created_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
    last_used_at INTEGER NOT NULL,
    UNIQUE (user_id, device_id),
    FOREIGN KEY(user_id) REFERENCES users(id) ON DELETE CASCADE
);

-- A login whose password was right but whose TOTP code is still pending.
CREATE TABLE IF NOT EXISTS login_challenges (
    challenge_id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id INTEGER NOT NULL,
    token_hashed TEXT NOT NULL UNIQUE,
    expires_at INTEGER NOT NULL,
    device_id TEXT NOT NULL,
    device_name TEXT NOT NULL,
    platform TEXT NOT NULL,
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);

-- Login tries per typed name, whether or not the name belongs to an account.
CREATE TABLE IF NOT EXISTS login_failures (
    name TEXT PRIMARY KEY,
    attempts INTEGER NOT NULL,
    window_started_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS recovery_codes (
    recovery_id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id INTEGER NOT NULL,
    code_hash TEXT UNIQUE NOT NULL,
    used BOOLEAN NOT NULL DEFAULT FALSE,
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);

-- A user's authenticator app: pending until a code confirms the setup. The secret is stored
-- encrypted, and the try counters are the wrong-code lock.
CREATE TABLE IF NOT EXISTS totp (
    user_id INTEGER PRIMARY KEY,
    authentication_secret TEXT NOT NULL,
    confirmed BOOLEAN NOT NULL DEFAULT FALSE,
    last_used_step INTEGER,
    failed_attempts INTEGER NOT NULL DEFAULT 0,
    lockouts INTEGER NOT NULL DEFAULT 0,
    last_failed_at INTEGER,
    created_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS locales (
    code TEXT PRIMARY KEY,
    display_name TEXT NOT NULL,
    icon TEXT,
    is_default BOOLEAN NOT NULL DEFAULT FALSE,
    enabled BOOLEAN NOT NULL DEFAULT TRUE,
    sort_order INTEGER NOT NULL,
    CHECK (enabled OR NOT is_default)
);

-- A CHECK sees one row at a time; a unique index over only the default rows allows at most one.
CREATE UNIQUE INDEX IF NOT EXISTS one_default_locale ON locales (is_default) WHERE is_default;

CREATE TABLE IF NOT EXISTS translation_keys (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    audience TEXT NOT NULL CHECK (audience IN ('frontend', 'backend')),
    namespace TEXT NOT NULL,
    name TEXT NOT NULL,
    hint TEXT,
    UNIQUE (namespace, name)
);

CREATE TABLE IF NOT EXISTS translations (
    locale_code TEXT NOT NULL,
    key_id INTEGER NOT NULL,
    value TEXT NOT NULL,
    PRIMARY KEY (locale_code, key_id),
    FOREIGN KEY (locale_code) REFERENCES locales(code) ON DELETE CASCADE,
    FOREIGN KEY (key_id) REFERENCES translation_keys(id) ON DELETE CASCADE
);

-- Created by the user's first change; until then, and for a NULL column, the defaults apply.
CREATE TABLE IF NOT EXISTS user_settings (
    user_id INTEGER PRIMARY KEY,
    locale TEXT,
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE,
    FOREIGN KEY (locale) REFERENCES locales(code) ON DELETE SET NULL
);
