use crate::{
    data_models::models::{
        BookMetadata, BookSeriesMetadata, EmailVerification, LibraryMetadata, RecoveryCodes,
        Sessions, UserMetadata, WithId, TOTP,
    },
    database_related_scripts::db_from_row::FromRow,
    error_types::DatabaseError,
    models::ResetPasswordRequest,
};
use rusqlite::{params, params_from_iter, Connection, OptionalExtension};
use serde::Serialize;

pub trait Extract: FromRow {
    // The building blocks each type supplies; queries are assembled from these.
    const TABLE: &'static str;
    const COLUMNS: &'static str;

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
}

impl Extract for BookSeriesMetadata {
    const TABLE: &'static str = "series";
    const COLUMNS: &'static str =
        "id, name, description, cover_image, start_release_year, end_release_year";
}

impl Extract for LibraryMetadata {
    const TABLE: &'static str = "library";
    const COLUMNS: &'static str = "id, library_name, library_type, cover_image, description";
}

impl Extract for UserMetadata {
    const TABLE: &'static str = "users";
    const COLUMNS: &'static str =
        "id, username, password_hash, email, email_verified, is_admin, created_at, last_login";
}

impl Extract for TOTP {
    const TABLE: &'static str = "totp";
    const COLUMNS: &'static str = "user_id, authentication_secret, created_at";
}

impl Extract for RecoveryCodes {
    const TABLE: &'static str = "recovery_codes";
    const COLUMNS: &'static str = "recovery_id, user_id, code_hash, used";
}

impl Extract for Sessions {
    const TABLE: &'static str = "sessions";
    const COLUMNS: &'static str =
        "session_id, user_id, device_id, device_name, platform, refresh_token_hashed, refresh_token_valid_until, session_token_hashed, session_token_valid_until, created_at, last_used_at, authentication_completed";
}

impl Extract for EmailVerification {
    const TABLE: &'static str = "email_verification";
    const COLUMNS: &'static str = "user_id, email_verification_token, expires_at, invalidated";
}

impl Extract for ResetPasswordRequest {
    const TABLE: &'static str = "reset_password_requests";
    const COLUMNS: &'static str = "user_id, reset_token, expires_at, invalidated";
}
