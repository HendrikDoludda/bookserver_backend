//! The `email_codes` table. Its key is two columns (user and purpose), so it gets these
//! dedicated functions instead of the generic `Extract` / `Update` traits.

use std::time::SystemTime;

use rusqlite::{
    params,
    types::{FromSql, FromSqlError, FromSqlResult, ToSqlOutput, ValueRef},
    Connection, OptionalExtension, ToSql,
};

use crate::{
    database_related_scripts::{
        db::{convert_system_time_to_unix_time, logged},
        db_from_row::FromRow,
    },
    error_types::DatabaseError,
    models::{CodePurpose, EmailCode},
};

impl ToSql for CodePurpose {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        Ok(self.as_str().into())
    }
}

impl FromSql for CodePurpose {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        match value.as_str()? {
            "reset_password" => Ok(CodePurpose::ResetPassword),
            "verify_email" => Ok(CodePurpose::VerifyEmail),
            _ => Err(FromSqlError::InvalidType),
        }
    }
}

pub fn find(
    conn: &Connection,
    user_id: i64,
    purpose: CodePurpose,
) -> Result<Option<EmailCode>, DatabaseError> {
    conn.query_row(
        "SELECT * FROM email_codes WHERE user_id = ?1 AND purpose = ?2",
        params![user_id, purpose],
        EmailCode::from_row,
    )
    .optional()
    .map_err(logged("reading", "email_codes", DatabaseError::QueryFailure))
}

/// Writes the row for this user and purpose, replacing an earlier one.
pub fn save(conn: &Connection, code: &EmailCode) -> Result<(), DatabaseError> {
    conn.execute(
        "INSERT INTO email_codes (user_id, purpose, code_hash, expires_at, attempts, \
             last_sent_at, sent_in_window, window_started_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8) \
         ON CONFLICT (user_id, purpose) DO UPDATE SET \
             code_hash = excluded.code_hash, \
             expires_at = excluded.expires_at, \
             attempts = excluded.attempts, \
             last_sent_at = excluded.last_sent_at, \
             sent_in_window = excluded.sent_in_window, \
             window_started_at = excluded.window_started_at",
        params![
            code.user_id,
            code.purpose,
            code.code_hash,
            convert_system_time_to_unix_time(code.expires_at),
            code.attempts,
            convert_system_time_to_unix_time(code.last_sent_at),
            code.sent_in_window,
            convert_system_time_to_unix_time(code.window_started_at),
        ],
    )
    .map_err(logged(
        "inserting into",
        "email_codes",
        DatabaseError::InsertionFailure,
    ))?;
    Ok(())
}

/// Counts one try on a live code that has tries left. `false` when there is nothing to
/// try: no code, an expired one, or one that is out of tries.
pub fn reserve_attempt(
    conn: &Connection,
    user_id: i64,
    purpose: CodePurpose,
    now: SystemTime,
    max_attempts: i64,
) -> Result<bool, DatabaseError> {
    let changed = conn
        .execute(
            "UPDATE email_codes SET attempts = attempts + 1 \
             WHERE user_id = ?1 AND purpose = ?2 AND attempts < ?3 AND expires_at > ?4",
            params![
                user_id,
                purpose,
                max_attempts,
                convert_system_time_to_unix_time(now)
            ],
        )
        .map_err(logged(
            "updating",
            "email_codes",
            DatabaseError::UpdatingFailure,
        ))?;
    Ok(changed == 1)
}

/// Deletes the code if it is still the one that was checked. `false` when another request
/// already used or replaced it.
pub fn consume(
    conn: &Connection,
    user_id: i64,
    purpose: CodePurpose,
    code_hash: &str,
) -> Result<bool, DatabaseError> {
    let changed = conn
        .execute(
            "DELETE FROM email_codes WHERE user_id = ?1 AND purpose = ?2 AND code_hash = ?3",
            params![user_id, purpose, code_hash],
        )
        .map_err(logged(
            "deleting from",
            "email_codes",
            DatabaseError::QueryFailure,
        ))?;
    Ok(changed == 1)
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, UNIX_EPOCH};

    use super::*;
    use crate::test_support::{insert_user, memory_connection};

    fn at(secs: u64) -> SystemTime {
        UNIX_EPOCH + Duration::from_secs(secs)
    }

    fn code(user_id: i64, purpose: CodePurpose, hash: &str) -> EmailCode {
        EmailCode {
            user_id,
            purpose,
            code_hash: hash.to_string(),
            expires_at: at(1_700_000_900),
            attempts: 0,
            last_sent_at: at(1_700_000_000),
            sent_in_window: 1,
            window_started_at: at(1_700_000_000),
        }
    }

    fn rows(conn: &Connection) -> i64 {
        conn.query_row("SELECT COUNT(*) FROM email_codes", [], |row| row.get(0))
            .unwrap()
    }

    #[test]
    fn a_saved_code_reads_back() {
        let conn = memory_connection();
        let user_id = insert_user(&conn, "alice");
        save(&conn, &code(user_id, CodePurpose::ResetPassword, "hash-1")).unwrap();

        let stored = find(&conn, user_id, CodePurpose::ResetPassword)
            .unwrap()
            .expect("the code exists");
        assert_eq!(stored.purpose, CodePurpose::ResetPassword);
        assert_eq!(stored.code_hash, "hash-1");
        assert_eq!(stored.expires_at, at(1_700_000_900));
        assert_eq!(stored.attempts, 0);
        assert_eq!(stored.last_sent_at, at(1_700_000_000));
        assert_eq!(stored.sent_in_window, 1);
        assert_eq!(stored.window_started_at, at(1_700_000_000));
    }

    #[test]
    fn saving_again_replaces_the_row() {
        let conn = memory_connection();
        let user_id = insert_user(&conn, "alice");
        save(&conn, &code(user_id, CodePurpose::ResetPassword, "hash-1")).unwrap();
        save(&conn, &code(user_id, CodePurpose::ResetPassword, "hash-2")).unwrap();

        assert_eq!(rows(&conn), 1);
        let stored = find(&conn, user_id, CodePurpose::ResetPassword)
            .unwrap()
            .unwrap();
        assert_eq!(stored.code_hash, "hash-2");
    }

    #[test]
    fn each_purpose_has_its_own_row() {
        let conn = memory_connection();
        let user_id = insert_user(&conn, "alice");
        save(&conn, &code(user_id, CodePurpose::ResetPassword, "reset")).unwrap();
        save(&conn, &code(user_id, CodePurpose::VerifyEmail, "verify")).unwrap();

        assert_eq!(rows(&conn), 2);
        let verify = find(&conn, user_id, CodePurpose::VerifyEmail)
            .unwrap()
            .unwrap();
        assert_eq!(verify.code_hash, "verify");
    }

    #[test]
    fn tries_are_counted_until_the_limit() {
        let conn = memory_connection();
        let user_id = insert_user(&conn, "alice");
        save(&conn, &code(user_id, CodePurpose::ResetPassword, "hash")).unwrap();
        let now = at(1_700_000_100);

        for _ in 0..3 {
            assert!(reserve_attempt(&conn, user_id, CodePurpose::ResetPassword, now, 3).unwrap());
        }
        assert!(!reserve_attempt(&conn, user_id, CodePurpose::ResetPassword, now, 3).unwrap());

        let stored = find(&conn, user_id, CodePurpose::ResetPassword)
            .unwrap()
            .unwrap();
        assert_eq!(stored.attempts, 3);
    }

    #[test]
    fn an_expired_or_missing_code_has_no_tries() {
        let conn = memory_connection();
        let user_id = insert_user(&conn, "alice");
        assert!(!reserve_attempt(
            &conn,
            user_id,
            CodePurpose::ResetPassword,
            at(1_700_000_100),
            5
        )
        .unwrap());

        save(&conn, &code(user_id, CodePurpose::ResetPassword, "hash")).unwrap();
        let after_expiry = at(1_700_000_900);
        assert!(
            !reserve_attempt(&conn, user_id, CodePurpose::ResetPassword, after_expiry, 5).unwrap()
        );
    }

    #[test]
    fn a_code_is_consumed_once_and_only_by_its_own_hash() {
        let conn = memory_connection();
        let user_id = insert_user(&conn, "alice");
        save(&conn, &code(user_id, CodePurpose::ResetPassword, "hash")).unwrap();

        assert!(!consume(&conn, user_id, CodePurpose::ResetPassword, "other").unwrap());
        assert_eq!(rows(&conn), 1);

        assert!(consume(&conn, user_id, CodePurpose::ResetPassword, "hash").unwrap());
        assert_eq!(rows(&conn), 0);
        assert!(!consume(&conn, user_id, CodePurpose::ResetPassword, "hash").unwrap());
    }

    #[test]
    fn an_unknown_purpose_in_the_table_is_a_query_failure() {
        let conn = memory_connection();
        let user_id = insert_user(&conn, "alice");
        conn.execute(
            "INSERT INTO email_codes (user_id, purpose, code_hash, expires_at, last_sent_at, \
             sent_in_window, window_started_at) VALUES (?1, 'something_else', 'h', 0, 0, 1, 0)",
            [user_id],
        )
        .unwrap();

        let result = conn.query_row(
            "SELECT * FROM email_codes WHERE user_id = ?1",
            [user_id],
            EmailCode::from_row,
        );
        assert!(result.is_err());
    }
}
