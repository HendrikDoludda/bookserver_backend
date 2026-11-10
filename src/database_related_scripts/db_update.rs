use crate::data_models::models::{
    BookDatabaseColumns, BookMetadata, BookSeriesMetadata, LibraryDatabaseColumns, LibraryMetadata,
    RecoveryCodes, RecoveryCodesColumns, SeriesDatabaseColumns, Sessions, SessionsDatabaseColumns,
    TOTPDatabaseColumns, UserDatabaseColumns, UserMetadata, TOTP,
};
use crate::database_related_scripts::db::logged;
use crate::error_types::DatabaseError;
use rusqlite::{params, params_from_iter, Connection, ToSql};

/// Update one or more columns of the row whose `KEY` column equals `id`.
/// Both methods return the number of rows changed: 0 when no row has that key.
pub trait Update {
    type Column: AsRef<str>;
    const TABLE: &'static str;
    const KEY: &'static str;

    fn update(
        conn: &Connection,
        column: Self::Column,
        new_value: &dyn ToSql,
        id: i64,
    ) -> Result<usize, DatabaseError> {
        let query = format!(
            "UPDATE {} SET {} = ?1 WHERE {} = ?2",
            Self::TABLE,
            column.as_ref(),
            Self::KEY
        );

        conn.execute(&query, params![new_value, id])
            .map_err(logged("updating", Self::TABLE, DatabaseError::UpdatingFailure))
    }

    fn update_multiple(
        conn: &Connection,
        columns: &[Self::Column],
        values: &[&dyn ToSql],
        id: i64,
    ) -> Result<usize, DatabaseError> {
        if columns.len() != values.len() {
            return Err(DatabaseError::InvalidParameters);
        }

        let assignments = columns
            .iter()
            .enumerate()
            .map(|(i, column)| format!("{} = ?{}", column.as_ref(), i + 1))
            .collect::<Vec<String>>()
            .join(", ");
        let id_placeholder = values.len() + 1;
        let query = format!(
            "UPDATE {} SET {} WHERE {} = ?{}",
            Self::TABLE,
            assignments,
            Self::KEY,
            id_placeholder
        );
        let mut bound: Vec<&dyn ToSql> = values.to_vec();
        bound.push(&id);
        conn.execute(&query, params_from_iter(bound))
            .map_err(logged("updating", Self::TABLE, DatabaseError::UpdatingFailure))
    }
}

impl Update for BookMetadata {
    type Column = BookDatabaseColumns;
    const TABLE: &'static str = "books";
    const KEY: &'static str = "id";
}

impl Update for BookSeriesMetadata {
    type Column = SeriesDatabaseColumns;
    const TABLE: &'static str = "series";
    const KEY: &'static str = "id";
}

impl Update for LibraryMetadata {
    type Column = LibraryDatabaseColumns;
    const TABLE: &'static str = "library";
    const KEY: &'static str = "id";
}

impl Update for UserMetadata {
    type Column = UserDatabaseColumns;
    const TABLE: &'static str = "users";
    const KEY: &'static str = "id";
}

impl Update for Sessions {
    type Column = SessionsDatabaseColumns;
    const TABLE: &'static str = "sessions";
    const KEY: &'static str = "session_id";
}

impl Update for RecoveryCodes {
    type Column = RecoveryCodesColumns;
    const TABLE: &'static str = "recovery_codes";
    const KEY: &'static str = "recovery_id";
}

impl Update for TOTP {
    type Column = TOTPDatabaseColumns;
    const TABLE: &'static str = "totp";
    const KEY: &'static str = "user_id";
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{capture_logs, insert_user, memory_connection};

    #[test]
    fn a_failed_update_is_logged_with_its_table_and_cause() {
        let conn = memory_connection();
        insert_user(&conn, "alice");
        let bob = insert_user(&conn, "bob");

        let mut result = None;
        let logs = capture_logs(|| {
            result = Some(UserMetadata::update(
                &conn,
                UserDatabaseColumns::Username,
                &"alice",
                bob,
            ));
        });

        assert!(matches!(result, Some(Err(DatabaseError::UpdatingFailure))));
        assert!(
            logs.contains("updating users failed: UNIQUE constraint failed: users.username"),
            "{logs}"
        );
    }

    #[test]
    fn update_reports_how_many_rows_changed() {
        let conn = memory_connection();
        let user_id = insert_user(&conn, "alice");

        let changed =
            UserMetadata::update(&conn, UserDatabaseColumns::Username, &"bob", user_id).unwrap();
        assert_eq!(changed, 1);

        let missing =
            UserMetadata::update(&conn, UserDatabaseColumns::Username, &"carol", user_id + 1)
                .unwrap();
        assert_eq!(missing, 0);
    }

    #[test]
    fn update_finds_rows_by_the_tables_key_column() {
        let conn = memory_connection();
        let user_id = insert_user(&conn, "alice");
        conn.execute(
            "INSERT INTO totp (user_id, authentication_secret) VALUES (?1, 'old')",
            [user_id],
        )
        .unwrap();

        let changed = TOTP::update(
            &conn,
            TOTPDatabaseColumns::AuthenticationSecret,
            &"new",
            user_id,
        )
        .unwrap();
        assert_eq!(changed, 1);

        let secret: String = conn
            .query_row(
                "SELECT authentication_secret FROM totp WHERE user_id = ?1",
                [user_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(secret, "new");
    }

    #[test]
    fn update_multiple_reports_changed_rows() {
        let conn = memory_connection();
        let user_id = insert_user(&conn, "alice");

        let changed = UserMetadata::update_multiple(
            &conn,
            &[UserDatabaseColumns::Username, UserDatabaseColumns::IsAdmin],
            &[&"bob", &true],
            user_id,
        )
        .unwrap();
        assert_eq!(changed, 1);

        let (username, is_admin): (String, bool) = conn
            .query_row(
                "SELECT username, is_admin FROM users WHERE id = ?1",
                [user_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(username, "bob");
        assert!(is_admin);
    }

    #[test]
    fn update_multiple_refuses_mismatched_columns_and_values() {
        let conn = memory_connection();
        let result = UserMetadata::update_multiple(
            &conn,
            &[UserDatabaseColumns::Username, UserDatabaseColumns::IsAdmin],
            &[&"bob"],
            1,
        );
        assert!(matches!(result, Err(DatabaseError::InvalidParameters)));
    }
}
