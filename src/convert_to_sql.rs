use rusqlite::ToSql;
use crate::models::{
    BookMetadata, BookSeriesMetadata, LibraryMetadata, SeriesLibraryConnection, UserMetadata,
};

pub trait ToSqlRow {
    fn convert(&self) -> Vec<Box<dyn ToSql>>;
}

impl ToSqlRow for BookMetadata {
    fn convert(&self) -> Vec<Box<dyn ToSql>> {
        vec![
            Box::new(self.title.clone()),
            Box::new(self.author.clone()),
            Box::new(self.format.as_str().to_string()),
            Box::new(self.language.to_code()),
            Box::new(self.cover_image.clone()),
            Box::new(self.folder_path.join("/")),
            Box::new(self.page_count),
            Box::new(self.series)
        ]
    }
}

impl ToSqlRow for BookSeriesMetadata {
    fn convert(&self) -> Vec<Box<dyn ToSql>> {
        vec![
            Box::new(self.name.clone()),
            Box::new(self.description.clone().unwrap_or_default()),
            Box::new(self.cover_image.clone()),
            Box::new(self.start_release_year),
            Box::new(self.end_release_year),
        ]
    }
}

impl ToSqlRow for LibraryMetadata {
    fn convert(&self) -> Vec<Box<dyn ToSql>> {
        vec![
            Box::new(self.name.clone()),
            Box::new(self.library_type.as_str().to_string()),
            Box::new(self.cover_image.clone()),
            Box::new(self.description.clone()),
        ]
    }
}

impl ToSqlRow for SeriesLibraryConnection {
    fn convert(&self) -> Vec<Box<dyn ToSql>> {
        vec![
            Box::new(self.series_id),
            Box::new(self.library_id),
        ]
    }
}

impl ToSqlRow for UserMetadata {
    fn convert(&self) -> Vec<Box<dyn ToSql>> {
        vec![
            Box::new(self.username.clone()),
            Box::new(self.passweord_hash.clone()),
            Box::new(self.email.clone()),
        ]
    }
}