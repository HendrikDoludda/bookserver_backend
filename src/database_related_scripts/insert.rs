use crate::error_types::DatabaseError;
use crate::models::{
    BookMetadata, BookSeriesMetadata, LibraryMetadata, SeriesLibraryConnection, UserMetadata,
};
use rusqlite::{params, Connection};

pub trait Insert {
    fn insert(&self, conn: &Connection) -> Result<i64, DatabaseError>;
}

impl Insert for BookMetadata {
    fn insert(&self, conn: &Connection) -> Result<i64, DatabaseError> {
        let query = "INSERT INTO books 
        (title, 
    author, 
    format, 
    language, 
    cover_image, 
    description, 
    tags, 
    file_path, 
    page_count, 
    volume_number, 
    chapter_number, 
    page_number, 
    file_hash, 
    last_modified, 
    file_size, 
    series) 
    VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)";

        let params = params![
            self.title.clone(),
            self.author.clone(),
            self.format.as_str().to_string(),
            self.language.to_code(),
            self.cover_image.clone(),
            self.description.clone(),
            self.tags.join(","),
            self.file_path.clone(),
            self.page_count,
            self.volume_number,
            self.chapter_number,
            self.page_number,
            self.file_hash.clone(),
            self.last_modified
                .map(|t| t.duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64),
            self.file_size.map(|size| size as i64),
            self.series
        ];

        conn.execute(query, params)
            .map_err(|_| DatabaseError::InsertionFailure)?;
        let id = conn.last_insert_rowid();
        Ok(id)
    }
}

impl Insert for BookSeriesMetadata {
    fn insert(&self, conn: &Connection) -> Result<i64, DatabaseError> {
        let query = "INSERT INTO series 
        (name,
         description,
          cover_image,
           start_release_year,
            end_release_year)
             VALUES (?1, ?2, ?3, ?4, ?5)";

        let params = params![
            self.name.clone(),
            self.description.clone().unwrap_or_default(),
            self.cover_image.clone(),
            self.start_release_year,
            self.end_release_year
        ];

        conn.execute(query, params)
            .map_err(|_| DatabaseError::InsertionFailure)?;
        let id = conn.last_insert_rowid();
        Ok(id)
    }
}

impl Insert for LibraryMetadata {
    fn insert(&self, conn: &Connection) -> Result<i64, DatabaseError> {
        let query = "INSERT INTO library 
        (library_name,
         library_type,
          cover_image,
           description)
            VALUES (?1, ?2, ?3, ?4)";

        let params = params![
            self.name.clone(),
            self.library_type.as_str(),
            self.cover_image.clone(),
            self.description.clone()
        ];

        conn.execute(query, params)
            .map_err(|_| DatabaseError::InsertionFailure)?;
        let id = conn.last_insert_rowid();
        Ok(id)
    }
}

impl Insert for UserMetadata {
    fn insert(&self, conn: &Connection) -> Result<i64, DatabaseError> {
        let query = "INSERT INTO users 
        (username,
         password_hash,
          email)
           VALUES (?1, ?2, ?3)";

        let params = params![
            self.username.clone(),
            self.passweord_hash.clone(),
            self.email.clone()
        ];

        conn.execute(query, params)
            .map_err(|_| DatabaseError::InsertionFailure)?;
        let id = conn.last_insert_rowid();
        Ok(id)
    }
}

impl Insert for SeriesLibraryConnection {
    fn insert(&self, conn: &Connection) -> Result<i64, DatabaseError> {
        let query = "INSERT INTO library_elements 
        (library_id,
         series_id)
          VALUES (?1, ?2)";

        let params = params![self.library_id, self.series_id];

        conn.execute(query, params)
            .map_err(|_| DatabaseError::InsertionFailure)?;
        let id = conn.last_insert_rowid();
        Ok(id)
    }
}
