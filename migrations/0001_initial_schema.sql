-- ============= Setup Books Table =================
CREATE TABLE IF NOT EXISTS books (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    title TEXT NOT NULL,
    author TEXT,
    format TEXT,
    language TEXT,
    cover_image TEXT,
    description TEXT,
    tags TEXT,
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
    library_type TEXT,
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
    description TEXT NOT NULL,
    cover_image TEXT,
    start_release_year INTEGER,
    end_release_year INTEGER
);

-- ============= Setup Users Table =================
CREATE TABLE IF NOT EXISTS users (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    username TEXT NOT NULL UNIQUE,
    password_hash TEXT NOT NULL,
    email TEXT NOT NULL UNIQUE,
    email_verified BOOLEAN NOT NULL DEFAULT FALSE,
    is_admin BOOLEAN NOT NULL DEFAULT FALSE,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    last_login DATETIME
);

CREATE TABLE IF NOT EXISTS email_verification (
    user_id INTEGER PRIMARY KEY,
    email_verification_token TEXT NOT NULL UNIQUE,
    expires_at DATETIME NOT NULL,
    invalidated BOOLEAN NOT NULL DEFAULT FALSE,
    FOREIGN KEY(user_id) REFERENCES users(id) ON DELETE CASCADE
);


CREATE TABLE IF NOT EXISTS sessions (
    session_id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id INTEGER NOT NULL,
    device_id BINARY(16) NOT NULL UNIQUE,
    device_name TEXT NOT NULL,
    platform TEXT,
    refresh_token_hashed TEXT NOT NULL UNIQUE,
    refresh_token_valid_until DATETIME NOT NULL,
    session_token_hashed TEXT NOT NULL,
    session_token_valid_until DATETIME NOT NULL,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    last_used_at DATETIME,
    authentication_completed BOOLEAN NOT NULL DEFAULT FALSE,
    FOREIGN KEY(user_id) REFERENCES users(id) ON DELETE CASCADE
); --automatically refresh the refresh token when it get's used

CREATE TABLE IF NOT EXISTS recovery_codes (
    recovery_id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id INTEGER NOT NULL,
    code_hash TEXT UNIQUE NOT NULL,
    used BOOLEAN DEFAULT FALSE,
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS totp (
    user_id INTEGER PRIMARY KEY,
    authentication_secret TEXT NOT NULL,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);
