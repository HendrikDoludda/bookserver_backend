use crate::{
    error_types::DatabaseError,
    models::{
        BookFormat, BookLanguage, BookMetadata, BookSeriesMetadata, LibraryMetadata, LibraryType, SeriesLibraryConnection, UserMetadata
    },
};
use rusqlite::{params, Connection, OptionalExtension};

use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub trait Extract: Sized {
    const QUERY: &'static str;

    fn extract(conn: &Connection, id: i64) -> Result<Option<Self>, DatabaseError>;
}

impl Extract for BookMetadata {
    const QUERY: &'static str = "
    SELECT id, title, author, format, language, cover_image, 
           description, tags, file_path, page_count, 
           volume_number, chapter_number, page_number, 
           file_hash, last_modified, file_size, series 
    FROM books 
    WHERE id = ?1
";

    fn extract(conn: &Connection, id: i64) -> Result<Option<Self>, DatabaseError> {
        let query = Self::QUERY;

        conn.query_row(&query, params![id], |row| {
            let format: String = row.get(3)?;
            let language: String = row.get(4)?;

            let file_size: Option<u64> = row.get::<_, Option<i64>>(15)?.map(|size| size as u64);

            let time: Option<SystemTime> = row.get::<_, Option<i64>>(14)?
    .map(|secs| UNIX_EPOCH + Duration::from_secs(secs as u64));

            let tags: Vec<String> = row
                .get::<_, String>(7)?
                .split(',')
                .map(|tag| tag.trim().to_string())
                .collect();

            Ok(Self {
                title: row.get(1)?,
                author: row.get(2)?,
                format: BookFormat::from_str(&format),
                language: BookLanguage::from_code(&language),
                cover_image: row.get(5)?,
                description: row.get(6)?,
                tags: tags, //the tags get joined with , before added them to the table so I assume I have to split them here somehow
                file_path: row.get(8)?,
                page_count: row.get(9)?,
                volume_number: row.get(10)?,
                chapter_number: row.get(11)?,
                page_number: row.get(12)?,
                file_hash: row.get(13)?,
                last_modified: time,  // needs to be converted to
                file_size: file_size, //is an i64 and needs to be u64
                series: row.get(16)?,
            })
        })
        .optional()
        .map_err(|_| DatabaseError::QueryFailure)
    }
}


impl Extract for BookSeriesMetadata {
    const QUERY: &'static str = "SELECT 
    name, description, cover_image, 
    start_release_year, end_release_year
    FROM series WHERE id = ?1";

    fn extract(conn: &Connection, id: i64) -> Result<Option<Self>, DatabaseError> {
        let query = Self::QUERY;

        conn.query_row(&query, params![id], |row| {
            Ok(Self {
                name: row.get(1)?,
                description: Some(row.get(2)?),
                cover_image: Some(row.get(3)?),
                start_release_year: Some(row.get(4)?),
                end_release_year: Some(row.get(5)?)
            })
        })
        .optional()
        .map_err(|_| DatabaseError::QueryFailure)
    }
}

impl Extract for LibraryMetadata{
    const QUERY: &'static str = "SELECT
    library_name, library_type, cover_image,
    description
    FROM library WHERE id = ?1";

    fn extract(conn: &Connection, id: i64) -> Result<Option<Self>, DatabaseError> {
        let query = Self::QUERY;

        conn.query_row(&query, params![id], |row|{
            let library_type_string: String = row.get(2)?;
            Ok(Self {
                name: row.get(1)?,
                library_type: LibraryType::from_str(library_type_string.as_str()),
                cover_image: Some(row.get(3)?),
                description: Some(row.get(4)?)
            })
        })
        .optional()
        .map_err(|_|DatabaseError::QueryFailure)
    }
}

impl Extract for UserMetadata {
    const QUERY: &'static str = "
    SELECT 
    username, password_hash, email
    FROM users WHERE id = ?1";

    fn extract(conn: &Connection, id: i64) -> Result<Option<Self>, DatabaseError> {
        let query = Self::QUERY;

        conn.query_row(&query, params![id], |row|{
            Ok(Self { 
                username: row.get(1)?,
                passweord_hash: row.get(2)?,
                email: row.get(3)? })
        })
        .optional()
        .map_err(|_|DatabaseError::QueryFailure)
    }
}