//add the conversion from row to model into here.
// we can use the column names to make it more explicit when converting between the two

use rusqlite::Row;

use crate::models::{
    BookFormat, BookLanguage, BookMetadata, BookSeriesMetadata, EmailVerification, LibraryMetadata,
    LibraryType, RecoveryCodes, Sessions, UserMetadata, TOTP,
};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub trait FromRow: Sized {
    fn from_row(row: &Row) -> rusqlite::Result<Self>;
}

impl FromRow for BookMetadata {
    fn from_row(row: &Row) -> rusqlite::Result<Self> {
        let format: String = row.get("format")?;
        let language: String = row.get("language")?;

        let file_size: Option<u64> = row
            .get::<_, Option<i64>>("file_size")?
            .map(|size| size as u64);

        let time = Some(convert_to_system_time(row, "last_modified", 14)?);

        let tags: Vec<String> = row
            .get::<_, String>("tags")?
            .split(',')
            .map(|tag| tag.trim().to_string())
            .collect();

        Ok(Self {
            title: row.get("title")?,
            author: row.get("author")?,
            format: BookFormat::from_str(&format),
            language: BookLanguage::from_code(&language),
            cover_image: row.get("cover_image")?,
            description: row.get("description")?,
            tags: tags,
            file_path: row.get("file_path")?,
            page_count: row.get("page_count")?,
            volume_number: row.get("volume_number")?,
            chapter_number: row.get("chapter_number")?,
            page_number: row.get("page_number")?,
            file_hash: row.get("file_hash")?,
            last_modified: time,
            file_size: file_size,
            series: row.get("series")?,
        })
    }
}

impl FromRow for BookSeriesMetadata {
    fn from_row(row: &Row) -> rusqlite::Result<Self> {
        Ok(Self {
            name: row.get("name")?,
            description: Some(row.get("description")?),
            cover_image: Some(row.get("cover_image")?),
            start_release_year: Some(row.get("start_release_year")?),
            end_release_year: Some(row.get("end_release_year")?),
        })
    }
}

impl FromRow for LibraryMetadata {
    fn from_row(row: &Row) -> rusqlite::Result<Self> {
        let library_type_string: String = row.get("library_type")?;
        let library_type = LibraryType::from_str(library_type_string.as_str());
        Ok(Self {
            name: row.get("name")?,
            library_type,
            cover_image: Some(row.get("cover_image")?),
            description: Some(row.get("description")?),
        })
    }
}

impl FromRow for UserMetadata {
    fn from_row(row: &Row) -> rusqlite::Result<Self> {
        let created_at = convert_to_system_time(row, "created_at", 6)?;
        let last_login = convert_to_system_time(row, "last_login", 7)?;
        Ok(Self {
            username: row.get("username")?,
            password_hash: row.get("password_hash")?,
            email: row.get("email")?,
            email_verified: row.get("email_verified")?,
            is_admin: row.get("is_admin")?,
            created_at,
            last_login,
        })
    }
}

impl FromRow for TOTP {
    fn from_row(row: &Row) -> rusqlite::Result<Self> {
        let created_at = convert_to_system_time(row, "created_at", 2)?;

        Ok(Self {
            user_id: row.get("user_id")?,
            authentication_secret: row.get("authentication_secret")?,
            created_at,
        })
    }
}

impl FromRow for RecoveryCodes {
    fn from_row(row: &Row) -> rusqlite::Result<Self> {
        Ok(Self {
            recovery_id: row.get("rocovery_id")?,
            user_id: row.get("user_id")?,
            code_hashed: row.get("code_hash")?,
            used: row.get("used")?,
        })
    }
}

impl FromRow for Sessions {
    fn from_row(row: &Row) -> rusqlite::Result<Self> {
        let refresh_token_valid_until =
            convert_to_system_time(row, "refresh_token_valid_until", 6)?;
        let session_token_valid_until =
            convert_to_system_time(row, "session_token_valid_until", 8)?;
        let created_at = convert_to_system_time(row, "created_at", 9)?;
        let last_used_at = convert_to_system_time(row, "last§", 10)?;

        Ok(Self {
            session_id: row.get("session_id")?,
            user_id: row.get("user_id")?,
            device_id: row.get("device_id")?,
            device_name: row.get("device_name")?,
            platform: row.get("platform")?,
            refresh_token: row.get("refresh_token_hashed")?,
            refresh_token_expiration_date: refresh_token_valid_until,
            session_token: row.get("session_token_hashed")?,
            session_token_expiration_date: session_token_valid_until,
            created_at: created_at,
            last_used_at: last_used_at,
            authentication_completed: row.get("authentication_completed")?,
        })
    }
}

impl FromRow for EmailVerification {
    fn from_row(row: &Row) -> rusqlite::Result<Self> {
        let expires_at = convert_to_system_time(row, "expires_at", 2)?;

        Ok(Self {
            user_id: row.get("user_id")?,
            verification_token: row.get("email_verification_token")?,
            expiration_date: expires_at,
            invalidated: row.get("invalidated")?,
        })
    }
}

fn convert_to_system_time(
    row: &Row,
    column: &str,
    index: usize,
) -> Result<SystemTime, rusqlite::Error> {
    let secs: Option<i64> = row.get(column)?;
    secs.map(|s| UNIX_EPOCH + Duration::from_secs(s as u64))
        .ok_or(rusqlite::Error::InvalidColumnType(
            //requires a usize instead of str
            index,
            "timestamp".into(),
            rusqlite::types::Type::Null,
        ))
}
