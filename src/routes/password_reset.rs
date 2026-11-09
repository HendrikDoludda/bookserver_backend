//! Password reset by an emailed code: ask for a code, then redeem it with a new password.

use std::{sync::Arc, time::SystemTime};

use crate::{
    data_models::authentication_model::ConfirmPasswordResetRequest,
    database_related_scripts::db_email_codes,
    db::Database,
    error_types::{AppErrors, AuthenticationError},
    models::{CodePurpose, UserDatabaseColumns, UserMetadata},
    routes::{
        auth::{check_new_password, end_all_sessions, hash_secret},
        email_codes::{check_code, find_account, send_code},
        email_helper::Mailer,
    },
};

const PURPOSE: CodePurpose = CodePurpose::ResetPassword;

/// Issues a reset code for `email` and mails it, within the limits; see [`send_code`].
pub async fn send_reset_code(db: Arc<Database>, mailer: Mailer, email: String, now: SystemTime) {
    send_code(db, mailer, PURPOSE, email, now).await;
}

/// Redeems a reset code. On success the account has the new password and no sessions;
/// the user logs in again, so a second factor still applies.
pub async fn confirm_reset(
    db: Arc<Database>,
    request: ConfirmPasswordResetRequest,
    now: SystemTime,
) -> Result<(), AppErrors> {
    let ConfirmPasswordResetRequest {
        email,
        code,
        new_password,
        new_password_repeat,
    } = request;
    check_new_password(&new_password, &new_password_repeat)?;

    let user = Arc::clone(&db)
        .read(move |conn| Ok(find_account(conn, PURPOSE, &email)?))
        .await?;
    let Some((user, code_hash)) = check_code(Arc::clone(&db), PURPOSE, user, code, now).await?
    else {
        return Err(AuthenticationError::InvalidValidationCode.into());
    };

    let new_hash = hash_secret(new_password).await?;
    let user_id = user.user_id;
    db.transact(move |conn| {
        // Exactly one row: a parallel request that used the same code finds it gone.
        if !db_email_codes::consume(conn, user_id, PURPOSE, &code_hash)? {
            return Err(AuthenticationError::InvalidValidationCode.into());
        }
        Database::update_value_with_connection::<UserMetadata>(
            conn,
            UserDatabaseColumns::PasswordHash,
            &new_hash,
            user_id,
        )?;
        end_all_sessions(conn, user_id)?;
        Ok(())
    })
    .await
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use crate::{
        routes::{auth::secret_matches, email_codes::CODE_LIFETIME},
        test_support::{code_in, TempDatabase},
    };

    const OLD: &str = "old password 123";
    const NEW: &str = "new password 456";

    /// Alice, stored as `Alice@Example.com`, with one session.
    async fn alice(temp: &TempDatabase) -> i64 {
        let password_hash = hash_secret(OLD.to_string()).await.unwrap();
        temp.db
            .setup_new_transaction(move |conn| {
                conn.execute(
                    "INSERT INTO users (username, password_hash, email) \
                     VALUES ('alice', ?1, 'Alice@Example.com')",
                    [&password_hash],
                )
                .unwrap();
                let user_id = conn.last_insert_rowid();
                conn.execute(
                    "INSERT INTO sessions (user_id, device_id, device_name, platform, \
                     refresh_token_hashed, refresh_token_valid_until, session_token_hashed, \
                     session_token_valid_until, last_used_at) \
                     VALUES (?1, 'device', 'phone', 'ios', 'refresh', 0, 'session', 0, 0)",
                    [user_id],
                )
                .unwrap();
                Ok(user_id)
            })
            .unwrap()
    }

    fn count(temp: &TempDatabase, sql: &'static str, user_id: i64) -> i64 {
        temp.db
            .setup_new_transaction(move |conn| {
                Ok(conn.query_row(sql, [user_id], |row| row.get(0)).unwrap())
            })
            .unwrap()
    }

    fn attempts(temp: &TempDatabase, user_id: i64) -> i64 {
        count(
            temp,
            "SELECT attempts FROM email_codes WHERE user_id = ?1",
            user_id,
        )
    }

    async fn password_is(temp: &TempDatabase, user_id: i64, password: &str) -> bool {
        let hash: String = temp
            .db
            .setup_new_transaction(move |conn| {
                Ok(conn
                    .query_row(
                        "SELECT password_hash FROM users WHERE id = ?1",
                        [user_id],
                        |row| row.get(0),
                    )
                    .unwrap())
            })
            .unwrap();
        secret_matches(password.to_string(), Some(hash))
            .await
            .unwrap()
    }

    async fn request(temp: &TempDatabase, mailer: &Mailer, email: &str, now: SystemTime) {
        send_reset_code(Arc::clone(&temp.db), mailer.clone(), email.to_string(), now).await;
    }

    fn confirmation(
        email: &str,
        code: &str,
        new: &str,
        repeat: &str,
    ) -> ConfirmPasswordResetRequest {
        ConfirmPasswordResetRequest {
            email: email.to_string(),
            code: code.to_string(),
            new_password: new.to_string(),
            new_password_repeat: repeat.to_string(),
        }
    }

    async fn confirm(temp: &TempDatabase, code: &str, now: SystemTime) -> Result<(), AppErrors> {
        confirm_reset(
            Arc::clone(&temp.db),
            confirmation("alice@example.com", code, NEW, NEW),
            now,
        )
        .await
    }

    fn is_invalid_code(result: &Result<(), AppErrors>) -> bool {
        matches!(
            result,
            Err(AppErrors::Authentication(
                AuthenticationError::InvalidValidationCode
            ))
        )
    }

    #[tokio::test]
    async fn a_known_address_gets_one_mail_at_its_stored_address() {
        let temp = TempDatabase::open();
        let user_id = alice(&temp).await;
        let (mailer, sent) = Mailer::capture();

        request(&temp, &mailer, "alice@example.com", SystemTime::now()).await;

        let sent = sent.lock().unwrap();
        assert_eq!(sent.len(), 1);
        assert_eq!(sent[0].email, "Alice@Example.com");
        assert_eq!(sent[0].username, "alice");
        assert_eq!(code_in(&sent[0]).len(), 8);
        assert_eq!(
            count(
                &temp,
                "SELECT COUNT(*) FROM email_codes WHERE user_id = ?1",
                user_id
            ),
            1
        );
    }

    #[tokio::test]
    async fn an_unknown_address_or_a_username_gets_nothing() {
        let temp = TempDatabase::open();
        let user_id = alice(&temp).await;
        let (mailer, sent) = Mailer::capture();

        request(&temp, &mailer, "nobody@example.com", SystemTime::now()).await;
        request(&temp, &mailer, "alice", SystemTime::now()).await;

        assert!(sent.lock().unwrap().is_empty());
        assert_eq!(
            count(
                &temp,
                "SELECT COUNT(*) FROM email_codes WHERE user_id = ?1",
                user_id
            ),
            0
        );
    }

    #[tokio::test]
    async fn the_cooldown_and_the_daily_cap_limit_the_mails() {
        let temp = TempDatabase::open();
        alice(&temp).await;
        let (mailer, sent) = Mailer::capture();
        let start = SystemTime::now();
        let after = |secs: u64| start + Duration::from_secs(secs);
        let mails = || sent.lock().unwrap().len();

        request(&temp, &mailer, "alice@example.com", start).await;
        request(&temp, &mailer, "alice@example.com", after(30)).await;
        assert_eq!(mails(), 1, "a second mail within the cooldown");

        for secs in [61, 122, 183, 244] {
            request(&temp, &mailer, "alice@example.com", after(secs)).await;
        }
        assert_eq!(mails(), 5);

        request(&temp, &mailer, "alice@example.com", after(305)).await;
        assert_eq!(mails(), 5, "a sixth mail on the same day");

        request(&temp, &mailer, "alice@example.com", after(24 * 60 * 60)).await;
        assert_eq!(mails(), 6, "a new day allows a new mail");
    }

    #[tokio::test]
    async fn the_emailed_code_sets_the_new_password_and_ends_every_session() {
        let temp = TempDatabase::open();
        let user_id = alice(&temp).await;
        let (mailer, sent) = Mailer::capture();
        let start = SystemTime::now();
        request(&temp, &mailer, "alice@example.com", start).await;
        let code = code_in(&sent.lock().unwrap()[0]);

        let typed_casually = code.to_lowercase();
        confirm(&temp, &typed_casually, start + Duration::from_secs(60))
            .await
            .unwrap();

        assert!(password_is(&temp, user_id, NEW).await);
        assert!(!password_is(&temp, user_id, OLD).await);
        assert_eq!(
            count(
                &temp,
                "SELECT COUNT(*) FROM sessions WHERE user_id = ?1",
                user_id
            ),
            0
        );

        let again = confirm(&temp, &code, start + Duration::from_secs(90)).await;
        assert!(is_invalid_code(&again), "a code works only once");
    }

    #[tokio::test]
    async fn a_wrong_code_is_counted_and_five_tries_end_the_code() {
        let temp = TempDatabase::open();
        let user_id = alice(&temp).await;
        let (mailer, sent) = Mailer::capture();
        let start = SystemTime::now();
        request(&temp, &mailer, "alice@example.com", start).await;
        let code = code_in(&sent.lock().unwrap()[0]);

        // `0` is not in the code alphabet, so this can never be the real code.
        let wrong = confirm(&temp, "00000000", start).await;
        assert!(is_invalid_code(&wrong));
        assert_eq!(attempts(&temp, user_id), 1, "the failed try stays counted");

        for _ in 0..4 {
            assert!(is_invalid_code(&confirm(&temp, "00000000", start).await));
        }
        let right_but_too_late = confirm(&temp, &code, start).await;
        assert!(is_invalid_code(&right_but_too_late));
        assert!(password_is(&temp, user_id, OLD).await);
    }

    #[tokio::test]
    async fn an_expired_code_is_refused() {
        let temp = TempDatabase::open();
        let user_id = alice(&temp).await;
        let (mailer, sent) = Mailer::capture();
        let start = SystemTime::now();
        request(&temp, &mailer, "alice@example.com", start).await;
        let code = code_in(&sent.lock().unwrap()[0]);

        let late = confirm(&temp, &code, start + CODE_LIFETIME + Duration::from_secs(1)).await;
        assert!(is_invalid_code(&late));
        assert!(password_is(&temp, user_id, OLD).await);
    }

    #[tokio::test]
    async fn an_unknown_address_fails_like_a_wrong_code() {
        let temp = TempDatabase::open();
        alice(&temp).await;

        let result = confirm_reset(
            Arc::clone(&temp.db),
            confirmation("nobody@example.com", "ABCD2345", NEW, NEW),
            SystemTime::now(),
        )
        .await;
        assert!(is_invalid_code(&result));
    }

    #[tokio::test]
    async fn bad_new_passwords_are_refused_before_a_try_is_counted() {
        let temp = TempDatabase::open();
        let user_id = alice(&temp).await;
        let (mailer, sent) = Mailer::capture();
        let start = SystemTime::now();
        request(&temp, &mailer, "alice@example.com", start).await;
        let code = code_in(&sent.lock().unwrap()[0]);

        let mismatch = confirm_reset(
            Arc::clone(&temp.db),
            confirmation("alice@example.com", &code, NEW, "something else 1"),
            start,
        )
        .await;
        assert!(matches!(
            mismatch,
            Err(AppErrors::Authentication(
                AuthenticationError::MismatchingPasswords
            ))
        ));

        let short = confirm_reset(
            Arc::clone(&temp.db),
            confirmation("alice@example.com", &code, "too short", "too short"),
            start,
        )
        .await;
        assert!(matches!(
            short,
            Err(AppErrors::Authentication(
                AuthenticationError::PasswordTooShort
            ))
        ));

        assert_eq!(attempts(&temp, user_id), 0);
    }
}
