use crate::models::{
    BookMetadata, BookSeriesMetadata, LibraryMetadata, SeriesLibraryConnection, UserMetadata,
};
use rusqlite::ToSql;

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
            Box::new(self.description.clone()),
            Box::new(self.tags.join(",")),
            Box::new(self.file_path.clone()),
            Box::new(self.page_count),
            Box::new(self.volume_number),
            Box::new(self.chapter_number),
            Box::new(self.page_number),
            Box::new(self.file_hash.clone()),
            Box::new(
                self.last_modified
                    .map(|t| t.duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64),
            ),
            Box::new(self.file_size.map(|size| size as i64)),
            Box::new(self.series),
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
        vec![Box::new(self.series_id), Box::new(self.library_id)]
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
