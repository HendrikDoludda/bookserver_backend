use crate::models::DatabaseTypes;
use rusqlite::{Connection, Result};
use std::path::Path;
use crate::models::{
    BookDatabaseColumns, LibraryDatabaseColumns, SeriesDatabaseColumns, UserDatabaseColumns,
    ColumnSelector,
};
use crate::convert_to_sql::ToSqlRow;

struct Database {
    connection: Connection,
}

impl Database {
    pub fn new() -> Result<Self> {
        let path = Path::new("./data/databases/");
        std::fs::create_dir_all("./data/databases/")?;
        let conn = Connection::open(path.join("app_data.sqlite"))?;
        conn.execute_batch(
            "
        PRAGMA foreign_keys = ON;
        PRAGMA journal_mode = WAL;
        PRAGMA user_version = 1;
        ",
        )?;
        initialize_database(&conn)?;
        Ok(Self { connection: conn })
    }

    pub fn insert(&self, db_type: DatabaseTypes, data: &impl ToSqlRow) -> Result<()> {
        match db_type {
            DatabaseTypes::Books => {
                insert_book(&self.connection, data)?;
            }
            DatabaseTypes::Library => {
                insert_library(&self.connection, data)?;
            }
            DatabaseTypes::LibraryElements => {
                insert_library_elements(&self.connection, data)?;
            }
            DatabaseTypes::Series => {
                insert_series(&self.connection, data)?;
            }
            DatabaseTypes::Users => {
                insert_user(&self.connection, data)?;
            }
        }
    }

    pub fn update_value(
        &self,
        db_type: DatabaseTypes,
        column: impl Into<ColumnSelector>,
        new_value: &dyn rusqlite::ToSql,
        id: i64,
    ) -> Result<()> {
        match (db_type, column.into()) {
            (DatabaseTypes::Books, ColumnSelector::Book(col)) => {
                update_book(&self.connection, col, new_value, id)?;
            }
            (DatabaseTypes::Library, ColumnSelector::Library(col)) => {
                update_library(&self.connection, col, new_value, id)?;
            }
            (DatabaseTypes::Series, ColumnSelector::Series(col)) => {
                update_series(&self.connection, col, new_value, id)?;
            }
            (DatabaseTypes::Users, ColumnSelector::User(col)) => {
                update_user(&self.connection, col, new_value, id)?;
            }
            _ => {}
        }
        Ok(())
    }

    pub fn remove_entry(&self, db_type: DatabaseTypes, id: i64) -> Result<()> {
        if db_type == DatabaseTypes::LibraryElements {
            return Err(rusqlite::Error::InvalidQuery); // Prevent direct deletion from junction table
        } else {
            let table_name = db_type.get_table_name();
            let query = format!("DELETE FROM {} WHERE id = ?1", table_name);
            self.connection.execute(&query, rusqlite::params![id])?;
            Ok(())
        }
    }

    pub fn remove_entry_from_library_elements(
        &self,
        series_id: i64,
        library_id: i64,
    ) -> Result<()> {
        let query = "DELETE FROM library_elements WHERE series_id = ?1 AND library_id = ?2";
        self.connection
            .execute(&query, rusqlite::params![series_id, library_id])?;
        Ok(())
    }

    pub fn get_id_from_table(
        &self,
        db_type: DatabaseTypes,
        column: impl Into<ColumnSelector>,
        new_value: &dyn rusqlite::ToSql,
    ) -> Result<Option<i64>> {
        let table_name = db_type.get_table_name();
        let column_name= match column.into() {
            ColumnSelector::Book(col) => col.as_str(),
            ColumnSelector::Library(col) => col.as_str(),
            ColumnSelector::Series(col) => col.as_str(),
            ColumnSelector::User(col) => col.as_str(),
        };
        let query = format!("SELECT id FROM {} WHERE {} = ?1", table_name,column_name);
        let mut stmt = self.connection.prepare(&query)?;
        let mut rows = stmt.query(rusqlite::params![new_value])?;

        if let Some(row) = rows.next()? {
            let id: i64 = row.get(0)?;
            Ok(Some(id))
        } else {
            Ok(None)
        }
    }
}

fn initialize_database(conn: &Connection) -> Result<(), rusqlite::Error> {
    conn.execute_batch(
        "
        -- ============= Setup Books Table =================
        CREATE TABLE IF NOT EXISTS books (
            id INTEGER PRIMARY KEY,
            title TEXT NOT NULL,
            author TEXT,
            format TEXT,
            language TEXT,
            cover_image TEXT,
            file_path TEXT NOT NULL UNIQUE,
            page_count INTEGER,
            series INTEGER
        );

        -- ============= Setup Library Table =================
        CREATE TABLE IF NOT EXISTS library (
            id INTEGER PRIMARY KEY,
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
            id INTEGER PRIMARY KEY,
            name TEXT NOT NULL,
            description TEXT NOT NULL,
            cover_image TEXT,
            start_release_year INTEGER,
            end_release_year INTEGER
        );

        -- ============= Setup Users Table =================
        CREATE TABLE IF NOT EXISTS users (
            id INTEGER PRIMARY KEY,
            username TEXT NOT NULL UNIQUE,
            password_hash TEXT NOT NULL,
            email TEXT
        );
        ",
    )?;
    Ok(())
}

fn insert_book(conn: &Connection, book: &impl ToSqlRow) -> Result<()> {
    conn.execute(
        "INSERT INTO books (title, author, format, language, cover_image, file_path, page_count, series) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        book.convert(),
    )?;
    Ok(())
}

fn insert_series(conn: &Connection, series: &impl ToSqlRow) -> Result<()> {
    conn.execute(
        "INSERT INTO series (name, description, cover_image, start_release_year, end_release_year) VALUES (?1, ?2, ?3, ?4, ?5)",
        series.convert(),
    )?;
    Ok(())
}

fn insert_library(conn: &Connection, library: &impl ToSqlRow) -> Result<()> {
    conn.execute(
        "INSERT INTO library (library_name, library_type, cover_image, description) VALUES (?1, ?2, ?3, ?4)",
        library.convert(),
    )?;
    Ok(())
}

fn insert_library_elements(conn: &Connection, elements: &impl ToSqlRow) -> Result<()> {
    conn.execute(
        "INSERT INTO library_elements (library_id, series_id) VALUES (?1, ?2)",
        elements.convert(),
    )?;
    Ok(())
}

fn insert_user(conn: &Connection, user: &impl ToSqlRow) -> Result<()> {
    conn.execute(
        "INSERT INTO users (username, password_hash, email) VALUES (?1, ?2, ?3)",
        user.convert(),
    )?;
    Ok(())
}

fn update_book(
    conn: &Connection,
    valueToUpdate: BookDatabaseColumns,
    newValue: &dyn ToSql,
    bookId: i64,
) -> Result<()> {
    let query = format!("UPDATE books SET {} = ?1 WHERE id = ?2", valueToUpdate);
    let rows = conn.execute(&query, params![newValue, bookId])?;
    println!("Updated {} rows in books table.", rows);
    Ok(())
}

fn update_series(
    conn: &Connection,
    valueToUpdate: SeriesDatabaseColumns,
    newValue: &dyn ToSql,
    seriesId: i64,
) -> Result<()> {
    let query = format!(
        "UPDATE series SET {} = ?1 WHERE id = ?2",
        valueToUpdate.as_str()
    );
    let rows = conn.execute(&query, params![newValue, seriesId])?;
    println!("Updated {} rows in series table.", rows);
    Ok(())
}

fn update_library(
    conn: &Connection,
    valueToUpdate: LibraryDatabaseColumns,
    newValue: &dyn ToSql,
    libraryId: i64,
) -> Result<()> {
    let query = format!(
        "UPDATE library SET {} = ?1 WHERE id = ?2",
        valueToUpdate.as_str()
    );
    let rows = conn.execute(&query, params![newValue, libraryId])?;
    println!("Updated {} rows in library table.", rows);
    Ok(())
}

fn update_user(
    conn: &Connection,
    valueToUpdate: UserDatabaseColumns,
    newValue: &dyn ToSql,
    userId: i64,
) -> Result<()> {
    let query = format!("UPDATE users SET {} = ?1 WHERE id = ?2", valueToUpdate.as_str());
    let rows = conn.execute(&query, params![newValue, userId])?;
    println!("Updated {} rows in users table.", rows);
    Ok(())
}
