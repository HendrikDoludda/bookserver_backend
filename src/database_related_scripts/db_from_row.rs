//! `rusqlite::Row` → model conversions. Columns are read by name rather than by
//! position, so the mapping stays explicit and independent of column order.

use rusqlite::{
    types::{FromSql, FromSqlError, FromSqlResult, ValueRef},
    Row,
};

use crate::models::{
    BookFormat, BookLanguage, BookMetadata, BookSeriesMetadata, EmailCode, LibraryMetadata,
    LibraryType, LoginChallenge, RecoveryCodes, Sessions, UserMetadata, TOTP,
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

        let time = read_optional_time(row, "last_modified")?;

        // `"".split(',')` yields one empty piece, so empty pieces are dropped: no tags reads
        // back as none.
        let tags: Vec<String> = row
            .get::<_, String>("tags")?
            .split(',')
            .map(str::trim)
            .filter(|tag| !tag.is_empty())
            .map(String::from)
            .collect();

        Ok(Self {
            title: row.get("title")?,
            author: row.get("author")?,
            format: BookFormat::from_str(&format),
            language: BookLanguage::from_code(&language),
            cover_image: row.get("cover_image")?,
            description: row.get("description")?,
            tags,
            file_path: row.get("file_path")?,
            page_count: row.get("page_count")?,
            volume_number: row.get("volume_number")?,
            chapter_number: row.get("chapter_number")?,
            page_number: row.get("page_number")?,
            file_hash: row.get("file_hash")?,
            last_modified: time,
            file_size,
            series: row.get("series")?,
        })
    }
}

impl FromRow for BookSeriesMetadata {
    fn from_row(row: &Row) -> rusqlite::Result<Self> {
        Ok(Self {
            name: row.get("name")?,
            description: row.get("description")?,
            cover_image: row.get("cover_image")?,
            start_release_year: row.get("start_release_year")?,
            end_release_year: row.get("end_release_year")?,
        })
    }
}

impl FromRow for LibraryMetadata {
    fn from_row(row: &Row) -> rusqlite::Result<Self> {
        let library_type: String = row.get("library_type")?;
        Ok(Self {
            name: row.get("library_name")?,
            library_type: LibraryType::from_str(&library_type),
            cover_image: row.get("cover_image")?,
            description: row.get("description")?,
        })
    }
}

impl FromRow for UserMetadata {
    fn from_row(row: &Row) -> rusqlite::Result<Self> {
        let created_at = read_time(row, "created_at")?;
        let last_login = read_optional_time(row, "last_login")?;
        Ok(Self {
            user_id: row.get("id")?,
            username: row.get("username")?,
            password_hash: row.get("password_hash")?,
            email: row.get("email")?,
            email_verified: row.get("email_verified")?,
            role: row.get("role")?,
            created_at,
            last_login,
        })
    }
}

impl FromRow for TOTP {
    fn from_row(row: &Row) -> rusqlite::Result<Self> {
        Ok(Self {
            user_id: row.get("user_id")?,
            authentication_secret: row.get("authentication_secret")?,
            confirmed: row.get("confirmed")?,
            last_used_step: row.get("last_used_step")?,
            failed_attempts: row.get("failed_attempts")?,
            lockouts: row.get("lockouts")?,
            last_failed_at: read_optional_time(row, "last_failed_at")?,
            created_at: read_time(row, "created_at")?,
        })
    }
}

impl FromRow for RecoveryCodes {
    fn from_row(row: &Row) -> rusqlite::Result<Self> {
        Ok(Self {
            recovery_id: row.get("recovery_id")?,
            user_id: row.get("user_id")?,
            code_hashed: row.get("code_hash")?,
            used: row.get("used")?,
        })
    }
}

impl FromRow for Sessions {
    fn from_row(row: &Row) -> rusqlite::Result<Self> {
        let refresh_token_valid_until = read_time(row, "refresh_token_valid_until")?;
        let session_token_valid_until = read_time(row, "session_token_valid_until")?;
        let created_at = read_time(row, "created_at")?;
        let last_used_at = read_time(row, "last_used_at")?;

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
            created_at,
            last_used_at,
        })
    }
}

impl FromRow for LoginChallenge {
    fn from_row(row: &Row) -> rusqlite::Result<Self> {
        Ok(Self {
            challenge_id: row.get("challenge_id")?,
            user_id: row.get("user_id")?,
            token_hashed: row.get("token_hashed")?,
            expires_at: read_time(row, "expires_at")?,
            device_id: row.get("device_id")?,
            device_name: row.get("device_name")?,
            platform: row.get("platform")?,
        })
    }
}

impl FromRow for EmailCode {
    fn from_row(row: &Row) -> rusqlite::Result<Self> {
        Ok(Self {
            user_id: row.get("user_id")?,
            purpose: row.get("purpose")?,
            code_hash: row.get("code_hash")?,
            expires_at: read_time(row, "expires_at")?,
            attempts: row.get("attempts")?,
            last_sent_at: read_time(row, "last_sent_at")?,
            sent_in_window: row.get("sent_in_window")?,
            window_started_at: read_time(row, "window_started_at")?,
        })
    }
}

/// Reads a unix-seconds column that must have a value. A NULL becomes rusqlite's own
/// `InvalidColumnType` error, with the column's real position and name.
pub(crate) fn read_time(row: &Row, column: &str) -> rusqlite::Result<SystemTime> {
    Ok(row.get::<_, UnixSeconds>(column)?.0)
}

/// Reads a unix-seconds column that may be NULL.
fn read_optional_time(row: &Row, column: &str) -> rusqlite::Result<Option<SystemTime>> {
    Ok(row
        .get::<_, Option<UnixSeconds>>(column)?
        .map(|time| time.0))
}

/// A unix-seconds column as a `SystemTime`. A wrapper, because Rust only allows a
/// trait impl where the crate owns the trait or the type, and rusqlite owns `FromSql`
/// while std owns `SystemTime`.
struct UnixSeconds(SystemTime);

impl FromSql for UnixSeconds {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        let secs = i64::column_result(value)?;
        // Negative or past `SystemTime`'s range: rusqlite turns `OutOfRange` into an
        // error naming the column, where plain `+` would panic.
        u64::try_from(secs)
            .ok()
            .and_then(|secs| UNIX_EPOCH.checked_add(Duration::from_secs(secs)))
            .map(UnixSeconds)
            .ok_or(FromSqlError::OutOfRange(secs))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::{types::Type, Connection};

    fn read_one<T>(
        sql: &str,
        read: impl FnOnce(&Row) -> rusqlite::Result<T>,
    ) -> rusqlite::Result<T> {
        let conn = Connection::open_in_memory().unwrap();
        conn.query_row(sql, [], read)
    }

    #[test]
    fn a_time_is_read_as_unix_seconds() {
        let time = read_one("SELECT 1700000000 AS created_at", |row| {
            read_time(row, "created_at")
        })
        .unwrap();
        assert_eq!(time, UNIX_EPOCH + Duration::from_secs(1_700_000_000));
    }

    #[test]
    fn a_missing_required_time_reports_its_real_column() {
        let err = read_one("SELECT 1 AS id, NULL AS created_at", |row| {
            read_time(row, "created_at")
        })
        .unwrap_err();

        assert!(
            matches!(
                err,
                rusqlite::Error::InvalidColumnType(1, ref name, Type::Null) if name == "created_at"
            ),
            "got {err:?}"
        );
    }

    #[test]
    fn a_missing_optional_time_is_none() {
        let time = read_one("SELECT NULL AS last_login", |row| {
            read_optional_time(row, "last_login")
        })
        .unwrap();
        assert_eq!(time, None);
    }

    #[test]
    fn a_present_optional_time_is_some() {
        let time = read_one("SELECT 1700000000 AS last_login", |row| {
            read_optional_time(row, "last_login")
        })
        .unwrap();
        assert_eq!(time, Some(UNIX_EPOCH + Duration::from_secs(1_700_000_000)));
    }

    #[test]
    fn a_negative_time_is_an_error_not_a_panic() {
        let err = read_one("SELECT 1 AS id, -5 AS created_at", |row| {
            read_time(row, "created_at")
        })
        .unwrap_err();
        assert!(
            matches!(err, rusqlite::Error::IntegralValueOutOfRange(1, -5)),
            "got {err:?}"
        );
    }

    #[test]
    fn a_negative_optional_time_is_an_error_too() {
        let err = read_one("SELECT -5 AS last_login", |row| {
            read_optional_time(row, "last_login")
        })
        .unwrap_err();
        assert!(
            matches!(err, rusqlite::Error::IntegralValueOutOfRange(0, -5)),
            "got {err:?}"
        );
    }
}
