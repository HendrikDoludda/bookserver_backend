use crate::{
    error_types::DatabaseError,
    models::{
        BookFormat, BookLanguage, BookMetadata, BookSeriesMetadata, LibraryMetadata, LibraryType,
        UserMetadata, WithId,
    },
};
use rusqlite::{params, params_from_iter, Connection, OptionalExtension, Row};
use serde::Serialize;

use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub trait Extract: Sized {
    // The building blocks each type supplies; queries are assembled from these.
    const TABLE: &'static str;
    const COLUMNS: &'static str;

    // Parse one row into the metadata struct. Written once per type, reused by
    // both the single and batch lookups below. Column indices match COLUMNS,
    // so column 0 is always the row id.
    fn from_row(row: &Row) -> rusqlite::Result<Self>;

    // Fetch a single row by id.
    fn extract(conn: &Connection, id: i64) -> Result<Option<Self>, DatabaseError> {
        let query = format!(
            "SELECT {} FROM {} WHERE id = ?1",
            Self::COLUMNS,
            Self::TABLE
        );

        conn.query_row(&query, params![id], Self::from_row)
            .optional()
            .map_err(|_| DatabaseError::QueryFailure)
    }

    // Fetch many rows by id in a single query. Returns each row paired with its
    // id; rows whose id doesn't exist are simply absent from the result.
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
            "SELECT {} FROM {} WHERE id IN ({})",
            Self::COLUMNS,
            Self::TABLE,
            placeholders
        );

        let mut stmt = conn
            .prepare(&query)
            .map_err(|_| DatabaseError::TaskPreparationFailure)?;

        let rows = stmt
            .query_map(params_from_iter(ids), |row| {
                Ok(WithId {
                    id: row.get(0)?,
                    data: Self::from_row(row)?,
                })
            })
            .map_err(|_| DatabaseError::QueryFailure)?;

        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|_| DatabaseError::NextRowFailure)
    }
}

impl Extract for BookMetadata {
    const TABLE: &'static str = "books";
    const COLUMNS: &'static str = "id, title, author, format, language, cover_image, \
        description, tags, file_path, page_count, volume_number, chapter_number, \
        page_number, file_hash, last_modified, file_size, series";

    fn from_row(row: &Row) -> rusqlite::Result<Self> {
        let format: String = row.get(3)?;
        let language: String = row.get(4)?;

        let file_size: Option<u64> = row.get::<_, Option<i64>>(15)?.map(|size| size as u64);

        let time: Option<SystemTime> = row
            .get::<_, Option<i64>>(14)?
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
            tags,
            file_path: row.get(8)?,
            page_count: row.get(9)?,
            volume_number: row.get(10)?,
            chapter_number: row.get(11)?,
            page_number: row.get(12)?,
            file_hash: row.get(13)?,
            last_modified: time,
            file_size,
            series: row.get(16)?,
        })
    }
}

impl Extract for BookSeriesMetadata {
    const TABLE: &'static str = "series";
    const COLUMNS: &'static str =
        "id, name, description, cover_image, start_release_year, end_release_year";

    fn from_row(row: &Row) -> rusqlite::Result<Self> {
        Ok(Self {
            name: row.get(1)?,
            description: Some(row.get(2)?),
            cover_image: Some(row.get(3)?),
            start_release_year: Some(row.get(4)?),
            end_release_year: Some(row.get(5)?),
        })
    }
}

impl Extract for LibraryMetadata {
    const TABLE: &'static str = "library";
    const COLUMNS: &'static str = "id, library_name, library_type, cover_image, description";

    fn from_row(row: &Row) -> rusqlite::Result<Self> {
        let library_type_string: String = row.get(2)?;
        Ok(Self {
            name: row.get(1)?,
            library_type: LibraryType::from_str(library_type_string.as_str()),
            cover_image: Some(row.get(3)?),
            description: Some(row.get(4)?),
        })
    }
}

impl Extract for UserMetadata {
    const TABLE: &'static str = "users";
    const COLUMNS: &'static str = "id, username, password_hash, email";

    fn from_row(row: &Row) -> rusqlite::Result<Self> {
        Ok(Self {
            username: row.get(1)?,
            passweord_hash: row.get(2)?,
            email: row.get(3)?,
        })
    }
}
