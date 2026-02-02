use rusqlite::ToSql;
use rusqlite::types::ToSqlOutput;
use rusqlite::types::Value;
use crate::utils::convert_file_path_to_blob;

pub trait ToSqlRow {
    fn convert(&self) -> Vec<&dyn ToSql>;
}

impl ToSqlRow for BookMetadata {
    fn convert(&self) -> Vec<&dyn ToSql> {
        vec![
            &self.title,
            &self.author,
            &self.format,
            &self.language,
            &self.cover_image_path,
            &self.file_path,
            &self.page_count,
            &self.series,
        ]
    }
}

impl ToSqlRow for BookSeriesMetadata {
    fn convert(&self) -> Vec<&dyn ToSql> {
        vec![
            &self.name,
            &self.description,
            &self.cover_image_path,
            &self.start_release_year,
            &self.end_release_year,
        ]
    }
}

impl ToSqlRow for LibraryMetadata {
    fn convert(&self) -> Vec<&dyn ToSql> {
        vec![
            &self.library_name,
            &self.library_type,
            &self.cover_image_path,
            &self.description,
        ]
    }
}

impl ToSqlRow for SeriesLibraryConnection {
    fn convert(&self) -> Vec<&dyn ToSql> {
        vec![
            &self.series_id,
            &self.library_id,
        ]
    }
}

impl ToSqlRow for UserMetadata {
    fn convert(&self) -> Vec<&dyn ToSql> {
        vec![
            &self.username,
            &self.password_hash,
            &self.email,
        ]
    }
}