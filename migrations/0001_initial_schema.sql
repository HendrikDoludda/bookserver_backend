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
    email TEXT
);
