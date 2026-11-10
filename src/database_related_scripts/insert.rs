use crate::data_models::models::{
    BookMetadata, BookSeriesMetadata, LibraryMetadata, RecoveryCodes, SeriesLibraryConnection,
    Sessions, UserMetadata, TOTP,
};
use crate::database_related_scripts::db::convert_system_time_to_unix_time;
use crate::error_types::DatabaseError;
use rusqlite::{params, Connection};

/// Write a model as a new row. Returns the new row's id (`last_insert_rowid`).
pub trait Insert {
    fn insert(&self, conn: &Connection) -> Result<i64, DatabaseError>;
}

impl Insert for BookMetadata {
    fn insert(&self, conn: &Connection) -> Result<i64, DatabaseError> {
        let query = "INSERT INTO books (title, author, format, language, cover_image, \
            description, tags, file_path, page_count, volume_number, chapter_number, \
            page_number, file_hash, last_modified, file_size, series) \
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
        let query = "INSERT INTO series (name, description, cover_image, \
            start_release_year, end_release_year) VALUES (?1, ?2, ?3, ?4, ?5)";

        let params = params![
            self.name.clone(),
            self.description.clone(),
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
        let query = "INSERT INTO library (library_name, library_type, cover_image, description) \
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
        let query = "INSERT INTO users (username, password_hash, email, email_verified, \
            is_admin, created_at, last_login) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)";

        let params = params![
            self.username,
            self.password_hash,
            self.email,
            self.email_verified,
            self.is_admin,
            convert_system_time_to_unix_time(self.created_at),
            self.last_login.map(convert_system_time_to_unix_time),
        ];

        conn.execute(query, params)
            .map_err(|_| DatabaseError::InsertionFailure)?;
        let id = conn.last_insert_rowid();
        Ok(id)
    }
}

impl Insert for SeriesLibraryConnection {
    fn insert(&self, conn: &Connection) -> Result<i64, DatabaseError> {
        let query = "INSERT INTO library_elements (library_id, series_id) VALUES (?1, ?2)";

        let params = params![self.library_id, self.series_id];

        conn.execute(query, params)
            .map_err(|_| DatabaseError::InsertionFailure)?;
        let id = conn.last_insert_rowid();
        Ok(id)
    }
}

impl Insert for Sessions {
    fn insert(&self, conn: &Connection) -> Result<i64, DatabaseError> {
        let query = "INSERT INTO sessions (user_id, device_id, device_name, platform, \
            refresh_token_hashed, refresh_token_valid_until, session_token_hashed, \
            session_token_valid_until, created_at, last_used_at) \
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)";
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
        ];
        conn.execute(query, params)
            .map_err(|_| DatabaseError::InsertionFailure)?;
        let id = conn.last_insert_rowid();
        Ok(id)
    }
}

impl Insert for TOTP {
    fn insert(&self, conn: &Connection) -> Result<i64, DatabaseError> {
        let query = "INSERT INTO totp (user_id, authentication_secret, confirmed, \
            last_used_step, failed_attempts, lockouts, last_failed_at, created_at) \
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)";

        let params = params![
            self.user_id,
            self.authentication_secret,
            self.confirmed,
            self.last_used_step,
            self.failed_attempts,
            self.lockouts,
            self.last_failed_at.map(convert_system_time_to_unix_time),
            convert_system_time_to_unix_time(self.created_at),
        ];

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

// Round trips: write a row through the model and read it back by its key — for the auth
// tables also update and delete it by its key, the path every auth handler takes.
#[cfg(test)]
mod tests {
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    use super::*;
    use crate::{
        database_related_scripts::db::Database,
        models::{
            DatabaseTypes, LibraryType, QuerySeparator, RecoveryCodesColumns,
            SessionsDatabaseColumns, TOTPDatabaseColumns, UserDatabaseColumns,
        },
        test_support::{insert_user, memory_connection},
    };

    #[test]
    fn library_round_trip() {
        let conn = memory_connection();
        // What the scanner writes: no cover and no description.
        let scanned = LibraryMetadata {
            name: "/books".to_string(),
            library_type: LibraryType::Books,
            cover_image: None,
            description: None,
        };
        let curated = LibraryMetadata {
            name: "Favourites".to_string(),
            library_type: LibraryType::Comics,
            cover_image: Some("./cover_images/favourites.jpg".to_string()),
            description: Some("The best ones".to_string()),
        };
        let scanned_id = Database::insert_with_connection(&conn, &scanned).unwrap();
        let curated_id = Database::insert_with_connection(&conn, &curated).unwrap();

        let stored =
            Database::get_entry_with_connection::<LibraryMetadata>(&conn, scanned_id).unwrap();
        assert_eq!(stored.name, "/books");
        assert_eq!(stored.library_type.as_str(), "books");
        assert_eq!(stored.cover_image, None);
        assert_eq!(stored.description, None);

        // The list endpoint's path: every row in one query, each with its id.
        let listed = Database::get_entries_batch_with_connection::<LibraryMetadata>(
            &conn,
            &[scanned_id, curated_id],
        )
        .unwrap();
        assert_eq!(listed.len(), 2);
        let stored = &listed
            .iter()
            .find(|row| row.id == curated_id)
            .expect("the curated library is listed")
            .data;
        assert_eq!(stored.name, "Favourites");
        assert_eq!(stored.library_type.as_str(), "comics");
        assert_eq!(
            stored.cover_image.as_deref(),
            Some("./cover_images/favourites.jpg")
        );
        assert_eq!(stored.description.as_deref(), Some("The best ones"));
    }

    #[test]
    fn series_round_trip() {
        let conn = memory_connection();
        // What the scanner writes: only the name.
        let scanned = BookSeriesMetadata {
            name: "the long road".to_string(),
            description: None,
            cover_image: None,
            start_release_year: None,
            end_release_year: None,
        };
        let described = BookSeriesMetadata {
            name: "a quiet sea".to_string(),
            description: Some("Twelve volumes".to_string()),
            cover_image: Some("./cover_images/sea.jpg".to_string()),
            start_release_year: Some(1999),
            end_release_year: Some(2004),
        };
        let scanned_id = Database::insert_with_connection(&conn, &scanned).unwrap();
        let described_id = Database::insert_with_connection(&conn, &described).unwrap();

        let stored =
            Database::get_entry_with_connection::<BookSeriesMetadata>(&conn, scanned_id).unwrap();
        assert_eq!(stored.name, "the long road");
        assert_eq!(
            stored.description, None,
            "no description must not come back as an empty one"
        );
        assert_eq!(stored.cover_image, None);
        assert_eq!(stored.start_release_year, None);
        assert_eq!(stored.end_release_year, None);

        let listed = Database::get_entries_batch_with_connection::<BookSeriesMetadata>(
            &conn,
            &[scanned_id, described_id],
        )
        .unwrap();
        assert_eq!(listed.len(), 2);
        let stored = &listed
            .iter()
            .find(|row| row.id == described_id)
            .expect("the described series is listed")
            .data;
        assert_eq!(stored.description.as_deref(), Some("Twelve volumes"));
        assert_eq!(stored.cover_image.as_deref(), Some("./cover_images/sea.jpg"));
        assert_eq!(stored.start_release_year, Some(1999));
        assert_eq!(stored.end_release_year, Some(2004));
    }

    // Whole seconds only: the columns store unix seconds, so anything finer is dropped.
    fn at(secs: u64) -> SystemTime {
        UNIX_EPOCH + Duration::from_secs(secs)
    }

    fn user(username: &str, email: &str) -> UserMetadata {
        UserMetadata {
            user_id: 0,
            username: username.to_string(),
            password_hash: "hash".to_string(),
            email: email.to_string(),
            email_verified: true,
            is_admin: true,
            created_at: at(1_700_000_000),
            last_login: None,
        }
    }

    #[test]
    fn user_round_trip() {
        let conn = memory_connection();
        let id =
            Database::insert_with_connection(&conn, &user("alice", "alice@example.com")).unwrap();

        let stored = Database::get_entry_with_connection::<UserMetadata>(&conn, id).unwrap();
        assert_eq!(stored.user_id, id);
        assert_eq!(stored.username, "alice");
        assert_eq!(stored.email, "alice@example.com");
        assert!(
            stored.email_verified,
            "the verified flag must reach the database"
        );
        assert!(stored.is_admin, "the admin flag must reach the database");
        assert_eq!(stored.created_at, at(1_700_000_000));
        assert_eq!(stored.last_login, None);

        let changed = Database::update_value_with_connection::<UserMetadata>(
            &conn,
            UserDatabaseColumns::LastLogin,
            &1_700_000_500_i64,
            id,
        )
        .unwrap();
        assert_eq!(changed, 1);
        let stored = Database::get_entry_with_connection::<UserMetadata>(&conn, id).unwrap();
        assert_eq!(stored.last_login, Some(at(1_700_000_500)));

        let removed =
            Database::remove_entry_with_connection(&conn, DatabaseTypes::Users, id).unwrap();
        assert_eq!(removed, 1);
        assert!(matches!(
            Database::get_entry_with_connection::<UserMetadata>(&conn, id),
            Err(DatabaseError::EntryNotFound)
        ));
    }

    #[test]
    fn a_user_written_with_the_column_defaults_reads_back() {
        let conn = memory_connection();
        let id = insert_user(&conn, "alice");

        let stored = Database::get_entry_with_connection::<UserMetadata>(&conn, id).unwrap();
        let now = SystemTime::now();
        assert!(stored.created_at <= now && stored.created_at > now - Duration::from_secs(60));
        assert_eq!(stored.last_login, None);
        assert!(!stored.is_admin);
        assert!(!stored.email_verified);
    }

    #[test]
    fn a_user_is_found_by_the_id_column() {
        let conn = memory_connection();
        let id = insert_user(&conn, "alice");

        let found = Database::search_for_single_row_with_connection::<UserMetadata>(
            &conn,
            &[UserDatabaseColumns::UserId],
            &[&id],
            QuerySeparator::And,
        )
        .unwrap()
        .expect("the user exists");
        assert_eq!(found.username, "alice");
    }

    #[test]
    fn email_and_username_ignore_case() {
        let conn = memory_connection();
        Database::insert_with_connection(&conn, &user("Alice", "Alice@Example.com")).unwrap();

        let found = Database::search_for_single_row_with_connection::<UserMetadata>(
            &conn,
            &[UserDatabaseColumns::Email],
            &[&"alice@example.com"],
            QuerySeparator::And,
        )
        .unwrap();
        assert!(found.is_some(), "an email lookup must ignore case");

        let same_email = Database::insert_with_connection(&conn, &user("bob", "ALICE@EXAMPLE.COM"));
        assert!(
            same_email.is_err(),
            "an email differing only in case is taken"
        );

        let same_name =
            Database::insert_with_connection(&conn, &user("ALICE", "other@example.com"));
        assert!(
            same_name.is_err(),
            "a username differing only in case is taken"
        );
    }

    #[test]
    fn session_round_trip() {
        let conn = memory_connection();
        let user_id = insert_user(&conn, "alice");
        let session = Sessions {
            session_id: 0,
            user_id,
            device_id: "device-1".to_string(),
            device_name: "phone".to_string(),
            platform: "ios".to_string(),
            refresh_token: "refresh-hash".to_string(),
            session_token: "session-hash".to_string(),
            refresh_token_expiration_date: at(1_702_592_000),
            session_token_expiration_date: at(1_700_000_900),
            created_at: at(1_700_000_000),
            last_used_at: at(1_700_000_000),
        };
        let id = Database::insert_with_connection(&conn, &session).unwrap();

        let stored = Database::get_entry_with_connection::<Sessions>(&conn, id).unwrap();
        assert_eq!(stored.session_id, id);
        assert_eq!(stored.user_id, user_id);
        assert_eq!(stored.device_id, "device-1");
        assert_eq!(stored.device_name, "phone");
        assert_eq!(stored.platform, "ios");
        assert_eq!(stored.refresh_token, "refresh-hash");
        assert_eq!(stored.session_token, "session-hash");
        assert_eq!(stored.refresh_token_expiration_date, at(1_702_592_000));
        assert_eq!(stored.session_token_expiration_date, at(1_700_000_900));
        assert_eq!(stored.created_at, at(1_700_000_000));
        assert_eq!(stored.last_used_at, at(1_700_000_000));

        let changed = Database::update_value_with_connection::<Sessions>(
            &conn,
            SessionsDatabaseColumns::LastUsedAt,
            &1_700_000_600_i64,
            id,
        )
        .unwrap();
        assert_eq!(changed, 1);
        let stored = Database::get_entry_with_connection::<Sessions>(&conn, id).unwrap();
        assert_eq!(stored.last_used_at, at(1_700_000_600));

        let removed =
            Database::remove_entry_with_connection(&conn, DatabaseTypes::Session, id).unwrap();
        assert_eq!(removed, 1);
        assert!(matches!(
            Database::get_entry_with_connection::<Sessions>(&conn, id),
            Err(DatabaseError::EntryNotFound)
        ));
    }

    #[test]
    fn totp_round_trip() {
        let conn = memory_connection();
        let user_id = insert_user(&conn, "alice");
        let totp = TOTP {
            user_id,
            authentication_secret: "secret".to_string(),
            confirmed: true,
            last_used_step: Some(56_666_666),
            failed_attempts: 3,
            lockouts: 2,
            last_failed_at: Some(at(1_700_000_100)),
            created_at: at(1_700_000_000),
        };
        Database::insert_with_connection(&conn, &totp).unwrap();

        let stored = Database::get_entry_with_connection::<TOTP>(&conn, user_id).unwrap();
        assert_eq!(stored.user_id, user_id);
        assert_eq!(stored.authentication_secret, "secret");
        assert!(
            stored.confirmed,
            "the confirmed flag must reach the database"
        );
        assert_eq!(stored.last_used_step, Some(56_666_666));
        assert_eq!(stored.failed_attempts, 3);
        assert_eq!(stored.lockouts, 2);
        assert_eq!(stored.last_failed_at, Some(at(1_700_000_100)));
        assert_eq!(stored.created_at, at(1_700_000_000));

        let changed = Database::update_value_with_connection::<TOTP>(
            &conn,
            TOTPDatabaseColumns::AuthenticationSecret,
            &"new-secret",
            user_id,
        )
        .unwrap();
        assert_eq!(changed, 1);
        let stored = Database::get_entry_with_connection::<TOTP>(&conn, user_id).unwrap();
        assert_eq!(stored.authentication_secret, "new-secret");

        let removed =
            Database::remove_entry_with_connection(&conn, DatabaseTypes::Totp, user_id).unwrap();
        assert_eq!(removed, 1);
    }

    #[test]
    fn recovery_code_round_trip() {
        let conn = memory_connection();
        let user_id = insert_user(&conn, "alice");
        let code = RecoveryCodes {
            recovery_id: 0,
            user_id,
            code_hashed: "code-hash".to_string(),
            used: false,
        };
        let id = Database::insert_with_connection(&conn, &code).unwrap();

        let stored = Database::get_entry_with_connection::<RecoveryCodes>(&conn, id).unwrap();
        assert_eq!(stored.recovery_id, id);
        assert_eq!(stored.user_id, user_id);
        assert_eq!(stored.code_hashed, "code-hash");
        assert!(!stored.used);

        let changed = Database::update_value_with_connection::<RecoveryCodes>(
            &conn,
            RecoveryCodesColumns::Used,
            &true,
            id,
        )
        .unwrap();
        assert_eq!(changed, 1);
        let stored = Database::get_entry_with_connection::<RecoveryCodes>(&conn, id).unwrap();
        assert!(stored.used);

        let removed =
            Database::remove_entry_with_connection(&conn, DatabaseTypes::RecoveryCodes, id)
                .unwrap();
        assert_eq!(removed, 1);
    }
}
