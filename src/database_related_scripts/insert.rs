use crate::data_models::models::{
    BookMetadata, BookSeriesMetadata, EmailVerification, LibraryMetadata, RecoveryCodes,
    SeriesLibraryConnection, Sessions, UserMetadata, TOTP,
};
use crate::error_types::DatabaseError;
use rusqlite::{params, Connection};
use std::time::SystemTime;

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
            convert_system_time_to_unix_time(self.last_modified.unwrap()),
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
            self.password_hash.clone(),
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

impl Insert for EmailVerification {
    fn insert(&self, conn: &Connection) -> Result<i64, DatabaseError> {
        let query = "INSERT INTO email_verification (user_id, email_verification_token, expires_at, invalidated) VALUES (?1, ?2, ?3, ?4)";
        let expired_time = convert_system_time_to_unix_time(self.expiration_date);
        let params = params![
            self.user_id,
            self.verification_token.clone(),
            expired_time,
            self.invalidated
        ];

        conn.execute(query, params)
            .map_err(|_| DatabaseError::InsertionFailure)?;
        let id = conn.last_insert_rowid();
        Ok(id)
    }
}

impl Insert for Sessions {
    fn insert(&self, conn: &Connection) -> Result<i64, DatabaseError> {
        let query = "INSERT INTO sessions (user_id, device_id, device_name, platform, refresh_token_hashed, refresh_token_valid_until, session_token_hashed, session_token_valid_until, created_at, last_used_at, authentication_completed) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8,?9,?10,?11)";
        let refresh_valid_until =
            convert_system_time_to_unix_time(self.refresh_token_expiration_date);
        let session_valid_until =
            convert_system_time_to_unix_time(self.session_token_expiration_date);
        let created_at = convert_system_time_to_unix_time(self.created_at);
        let last_used = convert_system_time_to_unix_time(self.last_used_at);
        let params = params![
            self.user_id,
            self.device_id,
            self.device_name,
            self.platform,
            self.refresh_token,
            refresh_valid_until,
            self.session_token,
            session_valid_until,
            created_at,
            last_used,
            self.authentication_completed,
        ];
        conn.execute(query, params)
            .map_err(|_| DatabaseError::InsertionFailure)?;
        let id = conn.last_insert_rowid();
        Ok(id)
    }
}

impl Insert for TOTP {
    fn insert(&self, conn: &Connection) -> Result<i64, DatabaseError> {
        let query =
            "INSERT INTO totp (user_id, authentication_secret, created_at) VALUES (?1, ?2, ?3)";

        let created_at = convert_system_time_to_unix_time(self.created_at);

        let params = params![self.user_id, self.authentication_secret, created_at];

        conn.execute(query, params)
            .map_err(|_| DatabaseError::InsertionFailure)?;
        let id = conn.last_insert_rowid();
        Ok(id)
    }
}

impl Insert for RecoveryCodes {
    fn insert(&self, conn: &Connection) -> Result<i64, DatabaseError> {
        let query = "INSERT INTO recovery_codes (user_id, code_hash, used) VALUES (?1, ?2, ?3)";

        let params = params![self.user_id, self.code_hashed, self.used];
        conn.execute(query, params)
            .map_err(|_| DatabaseError::InsertionFailure)?;
        let id = conn.last_insert_rowid();
        Ok(id)
    }
}

fn convert_system_time_to_unix_time(time: SystemTime) -> i64 {
    Some(time)
        .map(|t| t.duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64)
        .unwrap()
}
