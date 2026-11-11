//! Changes users make to their own account.

use std::{sync::Arc, time::SystemTime};

use rusqlite::Connection;

use crate::{
    data_models::authentication_model::{
        ChangeEmailRequest, ChangePasswordRequest, ChangeUserNameRequest, ConfirmEmailChangeRequest,
    },
    database_related_scripts::db_email_codes,
    db::Database,
    error_types::{AppErrors, AuthenticationError, DatabaseError},
    models::{CodePurpose, QuerySeparator, UserDatabaseColumns, UserMetadata},
    routes::{
        auth::{check_email, check_new_password, check_username, end_other_sessions, hash_secret},
        email_codes::{
            check_code, code_mail, counters_for_next_mail, generate_code, save_fresh_code,
            MailCounters,
        },
        email_helper::{EmailInformation, Mailer},
        login::check_current_password,
    },
};

const CHANGE_EMAIL: CodePurpose = CodePurpose::ChangeEmail;

/// Sets a new password once the current one is re-checked, and ends the user's other sessions
/// and pending logins; the session that asked stays.
pub async fn change_password(
    db: Arc<Database>,
    user_id: i64,
    current_session_id: i64,
    request: ChangePasswordRequest,
    now: SystemTime,
) -> Result<(), AppErrors> {
    let ChangePasswordRequest {
        current_password,
        new_password,
        new_password_repeat,
    } = request;
    check_new_password(&new_password, &new_password_repeat)?;
    // A plain compare is enough: the current password is verified next, so equal strings can
    // only mean a change to the same password.
    if new_password == current_password {
        return Err(AuthenticationError::PasswordUnchanged.into());
    }
    let checked = check_current_password(Arc::clone(&db), user_id, current_password, now).await?;
    let new_hash = hash_secret(new_password).await?;

    db.transact(move |conn| {
        save_new_password(
            conn,
            user_id,
            current_session_id,
            &checked.password_hash,
            &new_hash,
        )
    })
    .await
}

/// The final write of a password change. The stored hash must still be the one the current
/// password was checked against: a reset or a parallel change since then made it wrong.
fn save_new_password(
    conn: &Connection,
    user_id: i64,
    current_session_id: i64,
    checked_hash: &str,
    new_hash: &str,
) -> Result<(), AppErrors> {
    let stored = Database::get_entry_with_connection::<UserMetadata>(conn, user_id)?;
    if stored.password_hash != checked_hash {
        return Err(AuthenticationError::IncorrectPassword.into());
    }
    Database::update_value_with_connection::<UserMetadata>(
        conn,
        UserDatabaseColumns::PasswordHash,
        &new_hash,
        user_id,
    )?;
    end_other_sessions(conn, user_id, current_session_id)?;
    Ok(())
}

/// Renames the account. A name another account has, in any case, answers 409; a new case of the
/// user's own name is a rename like any other.
pub async fn change_username(
    db: Arc<Database>,
    user_id: i64,
    request: ChangeUserNameRequest,
) -> Result<(), AppErrors> {
    let ChangeUserNameRequest { new_username } = request;
    check_username(&new_username)?;
    db.transact(move |conn| {
        // Inside the write lock, so of two renames to one name the second sees the first: a 409,
        // never the `UNIQUE` constraint's 500.
        let holder = Database::search_for_single_row_with_connection::<UserMetadata>(
            conn,
            &[UserDatabaseColumns::Username],
            &[&new_username],
            QuerySeparator::And,
        )?;
        if holder.is_some_and(|holder| holder.user_id != user_id) {
            return Err(AuthenticationError::UsernameTaken.into());
        }
        Database::update_value_with_connection::<UserMetadata>(
            conn,
            UserDatabaseColumns::Username,
            &new_username,
            user_id,
        )?;
        Ok(())
    })
    .await
}

/// What a requested email change leads to.
pub enum EmailChangeRequested {
    /// Email is off, so the address changed at once — as a sign-up is verified on the spot then.
    Applied,
    /// The new address still has to prove itself: `send_email_change_code` mails it a code.
    AwaitsCode { new_email: String },
}

/// Checks a requested email change: the address, that it is new, the password, and that no
/// other account has it. With email on, the address then waits for its code; with email off, it
/// is written at once.
pub async fn request_email_change(
    db: Arc<Database>,
    mailer: &Mailer,
    user_id: i64,
    request: ChangeEmailRequest,
    now: SystemTime,
) -> Result<EmailChangeRequested, AppErrors> {
    let ChangeEmailRequest {
        new_email,
        password,
    } = request;
    check_email(&new_email)?;
    let current = Arc::clone(&db)
        .read(move |conn| {
            Ok(Database::get_entry_with_connection::<UserMetadata>(conn, user_id)?.email)
        })
        .await?;
    // ASCII-only, like the email column's NOCASE: the database holds the two as one address.
    if current.eq_ignore_ascii_case(&new_email) {
        return Err(AuthenticationError::EmailUnchanged.into());
    }
    check_current_password(Arc::clone(&db), user_id, password, now).await?;
    let lookup = new_email.clone();
    let taken = Arc::clone(&db)
        .read(move |conn| Ok(address_taken(conn, user_id, &lookup)?))
        .await?;
    if taken {
        return Err(AuthenticationError::EmailTaken.into());
    }
    if mailer.is_enabled() {
        return Ok(EmailChangeRequested::AwaitsCode { new_email });
    }

    db.transact(move |conn| {
        // Checked again inside the write lock, where no other account can take it meanwhile.
        if address_taken(conn, user_id, &new_email)? {
            return Err(AuthenticationError::EmailTaken.into());
        }
        write_new_email(conn, user_id, &new_email)?;
        Ok(EmailChangeRequested::Applied)
    })
    .await
}

/// Stores a code for the new address and mails it there, within `email_codes`' limits. The
/// request was answered already, so every outcome ends here: over the limits, nothing is sent.
pub async fn send_email_change_code(
    db: Arc<Database>,
    mailer: Mailer,
    user_id: i64,
    new_email: String,
    now: SystemTime,
) {
    match issue_email_change_code(db, user_id, new_email, now).await {
        Ok(Some(mail)) => {
            if let Err(err) = mailer.send(mail).await {
                tracing::error!("sending a {} email failed: {err}", CHANGE_EMAIL.as_str());
            }
        }
        Ok(None) => {}
        Err(err) => tracing::error!("issuing a {} code failed: {err}", CHANGE_EMAIL.as_str()),
    }
}

/// Stores a fresh code with the new address and returns the mail carrying it, or `None` while
/// the limits forbid one.
async fn issue_email_change_code(
    db: Arc<Database>,
    user_id: i64,
    new_email: String,
    now: SystemTime,
) -> Result<Option<EmailInformation>, AppErrors> {
    // Checked before hashing, so a request over the limits costs no Argon2 work.
    let may_send = Arc::clone(&db)
        .transact(move |conn| Ok(counters_for_change_mail(conn, user_id, now)?.is_some()))
        .await?;
    if !may_send {
        return Ok(None);
    }
    let code = generate_code();
    let code_hash = hash_secret(code.clone()).await?;

    // Checked again inside the write: a parallel request may have sent a mail meanwhile.
    db.transact(move |conn| {
        let Some(counters) = counters_for_change_mail(conn, user_id, now)? else {
            return Ok(None);
        };
        let user = Database::get_entry_with_connection::<UserMetadata>(conn, user_id)?;
        let stored = save_fresh_code(
            conn,
            user_id,
            CHANGE_EMAIL,
            Some(new_email),
            code_hash,
            counters,
            now,
        )?;
        Ok(Some(code_mail(&stored, &user, &code)))
    })
    .await
}

fn counters_for_change_mail(
    conn: &Connection,
    user_id: i64,
    now: SystemTime,
) -> Result<Option<MailCounters>, DatabaseError> {
    let existing = db_email_codes::find(conn, user_id, CHANGE_EMAIL)?;
    Ok(counters_for_next_mail(existing.as_ref(), now))
}

/// Confirms an email change with the code mailed to the new address. The try is counted first,
/// against the code's limit of tries.
pub async fn confirm_email_change(
    db: Arc<Database>,
    user_id: i64,
    request: ConfirmEmailChangeRequest,
    now: SystemTime,
) -> Result<(), AppErrors> {
    let user = Arc::clone(&db)
        .read(move |conn| {
            Ok(Database::get_entry_with_connection::<UserMetadata>(
                conn, user_id,
            )?)
        })
        .await?;
    let Some((_, code_hash)) =
        check_code(Arc::clone(&db), CHANGE_EMAIL, Some(user), request.code, now).await?
    else {
        return Err(AuthenticationError::InvalidValidationCode.into());
    };

    db.transact(move |conn| {
        // Read again inside the write lock: a parallel confirm may have used the code, or a newer
        // request replaced it.
        let new_email = db_email_codes::find(conn, user_id, CHANGE_EMAIL)?
            .filter(|code| code.code_hash == code_hash)
            .and_then(|code| code.new_email)
            .ok_or(AuthenticationError::InvalidValidationCode)?;
        if address_taken(conn, user_id, &new_email)? {
            return Err(AuthenticationError::EmailTaken.into());
        }
        write_new_email(conn, user_id, &new_email)
    })
    .await
}

/// Whether an account other than this one has the address, in any case.
fn address_taken(conn: &Connection, user_id: i64, email: &str) -> Result<bool, DatabaseError> {
    let holder = Database::search_for_single_row_with_connection::<UserMetadata>(
        conn,
        &[UserDatabaseColumns::Email],
        &[&email],
        QuerySeparator::And,
    )?;
    Ok(holder.is_some_and(|holder| holder.user_id != user_id))
}

/// Writes the new address as verified and deletes the account's emailed codes, the change's own
/// among them: the others went to the old address, and a reset code is stored per account, so it
/// would still work through the new one.
fn write_new_email(conn: &Connection, user_id: i64, new_email: &str) -> Result<(), AppErrors> {
    Database::update_multiple_values_with_connection::<UserMetadata>(
        conn,
        &[
            UserDatabaseColumns::Email,
            UserDatabaseColumns::EmailVerified,
        ],
        &[&new_email, &true],
        user_id,
    )?;
    db_email_codes::delete_for_user(conn, user_id)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use rusqlite::params;

    use std::{sync::Mutex, time::Duration};

    use super::*;
    use crate::{
        data_models::authentication_model::{
            ConfirmPasswordResetRequest, LoginRequest, LoginResponse,
        },
        db::convert_system_time_to_unix_time,
        routes::{
            email_codes::CODE_LIFETIME,
            login::log_in,
            password_reset::{confirm_reset, send_reset_code},
        },
        test_support::{code_in, insert_user, memory_connection, TempDatabase},
    };

    const OLD: &str = "correct horse battery";
    const NEW: &str = "battery staple horse";
    const OTHER: &str = "staple horse battery";

    async fn add_user(temp: &TempDatabase, name: &str) -> i64 {
        let hash = hash_secret(OLD.to_string()).await.unwrap();
        let name = name.to_string();
        temp.db
            .setup_new_transaction(move |conn| {
                conn.execute(
                    "INSERT INTO users (username, password_hash, email, email_verified) \
                     VALUES (?1, ?2, ?1 || '@example.com', 1)",
                    params![name, hash],
                )
                .unwrap();
                Ok(conn.last_insert_rowid())
            })
            .unwrap()
    }

    /// A live session on `device`, which names it and its token hashes; answers its id.
    fn add_session(temp: &TempDatabase, user_id: i64, device: &str, now: SystemTime) -> i64 {
        let device = device.to_string();
        let valid_until = convert_system_time_to_unix_time(now) + 30 * 24 * 60 * 60;
        temp.db
            .setup_new_transaction(move |conn| {
                conn.execute(
                    "INSERT INTO sessions (user_id, device_id, device_name, platform, \
                     refresh_token_hashed, refresh_token_valid_until, session_token_hashed, \
                     session_token_valid_until, last_used_at) \
                     VALUES (?1, ?2, ?2, 'ios', ?2 || '-refresh', ?3, ?2 || '-session', ?3, 0)",
                    params![user_id, device, valid_until],
                )
                .unwrap();
                Ok(conn.last_insert_rowid())
            })
            .unwrap()
    }

    /// A login that passed the password and waits for its TOTP code.
    fn add_pending_login(temp: &TempDatabase, user_id: i64, now: SystemTime) {
        let expires_at = convert_system_time_to_unix_time(now) + 5 * 60;
        temp.db
            .setup_new_transaction(move |conn| {
                conn.execute(
                    "INSERT INTO login_challenges (user_id, token_hashed, expires_at, device_id, \
                     device_name, platform) VALUES (?1, 'challenge', ?2, 'watch', 'watch', 'ios')",
                    params![user_id, expires_at],
                )
                .unwrap();
                Ok(())
            })
            .unwrap();
    }

    fn count(temp: &TempDatabase, sql: &'static str) -> i64 {
        temp.db
            .setup_new_transaction(move |conn| {
                Ok(conn.query_row(sql, [], |row| row.get(0)).unwrap())
            })
            .unwrap()
    }

    fn session_ids(temp: &TempDatabase) -> Vec<i64> {
        temp.db
            .setup_new_transaction(|conn| {
                let ids = conn
                    .prepare("SELECT session_id FROM sessions ORDER BY session_id")
                    .unwrap()
                    .query_map([], |row| row.get(0))
                    .unwrap()
                    .collect::<Result<Vec<i64>, _>>()
                    .unwrap();
                Ok(ids)
            })
            .unwrap()
    }

    async fn change(
        temp: &TempDatabase,
        user_id: i64,
        session_id: i64,
        current: &str,
        new: &str,
        repeat: &str,
        now: SystemTime,
    ) -> Result<(), AppErrors> {
        let request = ChangePasswordRequest {
            current_password: current.to_string(),
            new_password: new.to_string(),
            new_password_repeat: repeat.to_string(),
        };
        change_password(Arc::clone(&temp.db), user_id, session_id, request, now).await
    }

    async fn login(temp: &TempDatabase, password: &str, now: SystemTime) -> &'static str {
        login_as(temp, "alice", password, now).await
    }

    async fn login_as(
        temp: &TempDatabase,
        name: &str,
        password: &str,
        now: SystemTime,
    ) -> &'static str {
        let request = LoginRequest {
            username_or_email: name.to_string(),
            password: password.to_string(),
            device_name: "laptop".to_string(),
            device_id: None,
            platform: "macos".to_string(),
        };
        match log_in(Arc::clone(&temp.db), request, now).await {
            Ok(LoginResponse::Ok(_)) => "accepted",
            Err(AppErrors::Authentication(AuthenticationError::IncorrectCredentials)) => {
                "incorrect"
            }
            _ => "other",
        }
    }

    fn outcome<T>(result: &Result<T, AppErrors>) -> &'static str {
        match result {
            Ok(_) => "accepted",
            Err(AppErrors::Authentication(err)) => match err {
                AuthenticationError::IncorrectPassword => "incorrect password",
                AuthenticationError::PasswordUnchanged => "unchanged",
                AuthenticationError::MismatchingPasswords => "mismatch",
                AuthenticationError::PasswordTooShort => "too short",
                AuthenticationError::UsernameTaken => "username taken",
                AuthenticationError::InvalidUsername => "invalid username",
                AuthenticationError::EmailTaken => "email taken",
                AuthenticationError::EmailUnchanged => "unchanged email",
                AuthenticationError::InvalidEmail => "invalid email",
                AuthenticationError::InvalidValidationCode => "invalid code",
                _ => "other auth error",
            },
            Err(_) => "other error",
        }
    }

    #[tokio::test]
    async fn the_new_password_replaces_the_old_one_and_only_the_asking_session_stays() {
        let temp = TempDatabase::open();
        let now = SystemTime::now();
        let alice = add_user(&temp, "alice").await;
        let bob = add_user(&temp, "bob").await;
        let asking = add_session(&temp, alice, "alice-phone", now);
        add_session(&temp, alice, "alice-tablet", now);
        let bobs = add_session(&temp, bob, "bob-laptop", now);
        add_pending_login(&temp, alice, now);

        change(&temp, alice, asking, OLD, NEW, NEW, now)
            .await
            .unwrap();

        assert_eq!(session_ids(&temp), vec![asking, bobs]);
        assert_eq!(
            count(&temp, "SELECT COUNT(*) FROM login_challenges"),
            0,
            "a login waiting for its TOTP code ends too"
        );
        assert_eq!(login(&temp, OLD, now).await, "incorrect");
        assert_eq!(login(&temp, NEW, now).await, "accepted");
    }

    fn alices_failed_tries(temp: &TempDatabase) -> i64 {
        count(
            temp,
            "SELECT COALESCE(SUM(attempts), 0) FROM login_failures WHERE name = 'alice'",
        )
    }

    #[tokio::test]
    async fn a_wrong_current_password_is_refused_and_counts_on_logins_throttle() {
        let temp = TempDatabase::open();
        let now = SystemTime::now();
        let alice = add_user(&temp, "alice").await;
        let session = add_session(&temp, alice, "alice-phone", now);

        let result = change(&temp, alice, session, "wrong horse battery", NEW, NEW, now).await;

        assert_eq!(outcome(&result), "incorrect password");
        assert_eq!(alices_failed_tries(&temp), 1);
        assert_eq!(login(&temp, OLD, now).await, "accepted", "nothing changed");
    }

    #[tokio::test]
    async fn an_unchanged_mismatched_or_short_password_is_refused_before_any_check() {
        let temp = TempDatabase::open();
        let now = SystemTime::now();
        let alice = add_user(&temp, "alice").await;
        let session = add_session(&temp, alice, "alice-phone", now);
        // A check of the right password would clear these; untouched, they prove none ran.
        for _ in 0..3 {
            assert_eq!(login(&temp, "wrong horse battery", now).await, "incorrect");
        }

        for (new, repeat, expected) in [
            (OLD, OLD, "unchanged"),
            (NEW, OTHER, "mismatch"),
            ("too short", "too short", "too short"),
        ] {
            let result = change(&temp, alice, session, OLD, new, repeat, now).await;
            assert_eq!(outcome(&result), expected, "{new:?}");
        }
        assert_eq!(alices_failed_tries(&temp), 3, "no password was checked");
        assert_eq!(login(&temp, OLD, now).await, "accepted");
    }

    #[test]
    fn a_hash_changed_since_the_check_is_refused_at_the_final_write() {
        let conn = memory_connection();
        let alice = insert_user(&conn, "alice");
        let stored_hash = || -> String {
            conn.query_row(
                "SELECT password_hash FROM users WHERE id = ?1",
                [alice],
                |row| row.get(0),
            )
            .unwrap()
        };

        let stale = save_new_password(&conn, alice, 1, "the hash that was checked", "new hash");
        assert_eq!(outcome(&stale), "incorrect password");
        assert_eq!(stored_hash(), "not-a-hash");

        save_new_password(&conn, alice, 1, "not-a-hash", "new hash").unwrap();
        assert_eq!(stored_hash(), "new hash");
    }

    #[tokio::test]
    async fn of_two_parallel_changes_one_wins_and_the_other_password_never_works() {
        let temp = TempDatabase::open();
        let now = SystemTime::now();
        let alice = add_user(&temp, "alice").await;
        let session = add_session(&temp, alice, "alice-phone", now);

        let (a, b) = tokio::join!(
            change(&temp, alice, session, OLD, NEW, NEW, now),
            change(&temp, alice, session, OLD, OTHER, OTHER, now),
        );

        let mut outcomes = [outcome(&a), outcome(&b)];
        outcomes.sort_unstable();
        assert_eq!(outcomes, ["accepted", "incorrect password"]);
        let (winner, loser) = if a.is_ok() {
            (NEW, OTHER)
        } else {
            (OTHER, NEW)
        };
        assert_eq!(login(&temp, loser, now).await, "incorrect");
        assert_eq!(login(&temp, winner, now).await, "accepted");
    }

    async fn rename(
        temp: &TempDatabase,
        user_id: i64,
        new_username: &str,
    ) -> Result<(), AppErrors> {
        let request = ChangeUserNameRequest {
            new_username: new_username.to_string(),
        };
        change_username(Arc::clone(&temp.db), user_id, request).await
    }

    fn username_of(temp: &TempDatabase, user_id: i64) -> String {
        temp.db
            .setup_new_transaction(move |conn| {
                Ok(conn
                    .query_row(
                        "SELECT username FROM users WHERE id = ?1",
                        [user_id],
                        |row| row.get(0),
                    )
                    .unwrap())
            })
            .unwrap()
    }

    #[tokio::test]
    async fn a_renamed_account_logs_in_with_the_new_name_only() {
        let temp = TempDatabase::open();
        let now = SystemTime::now();
        let alice = add_user(&temp, "alice").await;

        rename(&temp, alice, "alicia").await.unwrap();

        assert_eq!(username_of(&temp, alice), "alicia");
        assert_eq!(login_as(&temp, "alicia", OLD, now).await, "accepted");
        assert_eq!(login_as(&temp, "alice", OLD, now).await, "incorrect");
    }

    #[tokio::test]
    async fn another_accounts_name_in_any_case_is_taken_but_a_new_case_of_your_own_is_not() {
        let temp = TempDatabase::open();
        let alice = add_user(&temp, "alice").await;
        add_user(&temp, "bob").await;

        for taken in ["bob", "BOB", "Bob"] {
            let result = rename(&temp, alice, taken).await;
            assert_eq!(outcome(&result), "username taken", "{taken}");
        }
        assert_eq!(username_of(&temp, alice), "alice");

        rename(&temp, alice, "Alice").await.unwrap();
        assert_eq!(username_of(&temp, alice), "Alice");
    }

    #[tokio::test]
    async fn an_invalid_name_is_refused() {
        let temp = TempDatabase::open();
        let alice = add_user(&temp, "alice").await;

        for invalid in ["al", "al ice", "al@ice"] {
            let result = rename(&temp, alice, invalid).await;
            assert_eq!(outcome(&result), "invalid username", "{invalid:?}");
        }
        assert_eq!(username_of(&temp, alice), "alice");
    }

    #[tokio::test]
    async fn of_two_accounts_racing_for_one_name_one_gets_it_and_the_other_a_409() {
        let temp = TempDatabase::open();
        let alice = add_user(&temp, "alice").await;
        let bob = add_user(&temp, "bob").await;

        let (a, b) = tokio::join!(rename(&temp, alice, "carol"), rename(&temp, bob, "carol"));

        let mut outcomes = [outcome(&a), outcome(&b)];
        outcomes.sort_unstable();
        assert_eq!(outcomes, ["accepted", "username taken"], "never a 500");
        let winner = if a.is_ok() { alice } else { bob };
        assert_eq!(username_of(&temp, winner), "carol");
    }

    const NEW_ADDRESS: &str = "alice.new@example.com";

    async fn request_change(
        temp: &TempDatabase,
        mailer: &Mailer,
        user_id: i64,
        new_email: &str,
        password: &str,
        now: SystemTime,
    ) -> Result<EmailChangeRequested, AppErrors> {
        let request = ChangeEmailRequest {
            new_email: new_email.to_string(),
            password: password.to_string(),
        };
        request_email_change(Arc::clone(&temp.db), mailer, user_id, request, now).await
    }

    fn requested(result: &Result<EmailChangeRequested, AppErrors>) -> &'static str {
        match result {
            Ok(EmailChangeRequested::Applied) => "applied",
            Ok(EmailChangeRequested::AwaitsCode { .. }) => "awaits code",
            Err(_) => outcome(result),
        }
    }

    /// Asks for an email change the way the handler does, and mails its code before returning.
    async fn change_email(
        temp: &TempDatabase,
        mailer: &Mailer,
        user_id: i64,
        new_email: &str,
        now: SystemTime,
    ) -> &'static str {
        let result = request_change(temp, mailer, user_id, new_email, OLD, now).await;
        if let Ok(EmailChangeRequested::AwaitsCode { new_email }) = &result {
            let db = Arc::clone(&temp.db);
            send_email_change_code(db, mailer.clone(), user_id, new_email.clone(), now).await;
        }
        requested(&result)
    }

    async fn confirm(
        temp: &TempDatabase,
        user_id: i64,
        code: &str,
        now: SystemTime,
    ) -> Result<(), AppErrors> {
        let request = ConfirmEmailChangeRequest {
            code: code.to_string(),
        };
        confirm_email_change(Arc::clone(&temp.db), user_id, request, now).await
    }

    /// The `(address, code)` of every mail sent so far.
    fn mails(sent: &Mutex<Vec<EmailInformation>>) -> Vec<(String, String)> {
        sent.lock()
            .unwrap()
            .iter()
            .map(|mail| (mail.email.clone(), code_in(mail)))
            .collect()
    }

    /// The stored address and whether it is verified.
    fn email_of(temp: &TempDatabase, user_id: i64) -> (String, bool) {
        temp.db
            .setup_new_transaction(move |conn| {
                Ok(conn
                    .query_row(
                        "SELECT email, email_verified FROM users WHERE id = ?1",
                        [user_id],
                        |row| Ok((row.get(0)?, row.get(1)?)),
                    )
                    .unwrap())
            })
            .unwrap()
    }

    fn code_rows(temp: &TempDatabase) -> i64 {
        count(temp, "SELECT COUNT(*) FROM email_codes")
    }

    #[tokio::test]
    async fn the_code_goes_to_the_new_address_and_its_confirm_moves_the_login_there() {
        let temp = TempDatabase::open();
        let now = SystemTime::now();
        let alice = add_user(&temp, "alice").await;
        let (mailer, sent) = Mailer::capture();

        let result = change_email(&temp, &mailer, alice, NEW_ADDRESS, now).await;

        assert_eq!(result, "awaits code");
        let mail = mails(&sent);
        assert_eq!(mail.len(), 1);
        let (to, code) = &mail[0];
        assert_eq!(to, NEW_ADDRESS, "only the new address gets the code");
        assert_eq!(
            login_as(&temp, "alice@example.com", OLD, now).await,
            "accepted"
        );
        assert_eq!(login_as(&temp, NEW_ADDRESS, OLD, now).await, "incorrect");

        confirm(&temp, alice, code, now).await.unwrap();

        assert_eq!(email_of(&temp, alice), (NEW_ADDRESS.to_string(), true));
        assert_eq!(login_as(&temp, NEW_ADDRESS, OLD, now).await, "accepted");
        assert_eq!(
            login_as(&temp, "alice@example.com", OLD, now).await,
            "incorrect"
        );
        let again = confirm(&temp, alice, code, now).await;
        assert_eq!(outcome(&again), "invalid code", "a code works once");
    }

    #[tokio::test]
    async fn five_wrong_codes_end_the_code() {
        let temp = TempDatabase::open();
        let now = SystemTime::now();
        let alice = add_user(&temp, "alice").await;
        let (mailer, sent) = Mailer::capture();
        change_email(&temp, &mailer, alice, NEW_ADDRESS, now).await;
        let (_, code) = mails(&sent).remove(0);

        // `0` is not in the code alphabet, so this can never be the real code.
        for _ in 0..5 {
            assert_eq!(
                outcome(&confirm(&temp, alice, "00000000", now).await),
                "invalid code"
            );
        }
        let right_but_too_late = confirm(&temp, alice, &code, now).await;

        assert_eq!(outcome(&right_but_too_late), "invalid code");
        assert_eq!(email_of(&temp, alice).0, "alice@example.com");
    }

    #[tokio::test]
    async fn an_expired_code_is_refused() {
        let temp = TempDatabase::open();
        let now = SystemTime::now();
        let alice = add_user(&temp, "alice").await;
        let (mailer, sent) = Mailer::capture();
        change_email(&temp, &mailer, alice, NEW_ADDRESS, now).await;
        let (_, code) = mails(&sent).remove(0);

        let late = now + CODE_LIFETIME + Duration::from_secs(1);
        let result = confirm(&temp, alice, &code, late).await;

        assert_eq!(outcome(&result), "invalid code");
        assert_eq!(email_of(&temp, alice).0, "alice@example.com");
    }

    #[tokio::test]
    async fn a_reset_code_mailed_to_the_old_address_is_refused_after_the_change() {
        let temp = TempDatabase::open();
        let now = SystemTime::now();
        let alice = add_user(&temp, "alice").await;
        let (mailer, sent) = Mailer::capture();
        let db = Arc::clone(&temp.db);
        send_reset_code(db, mailer.clone(), "alice@example.com".to_string(), now).await;
        change_email(&temp, &mailer, alice, NEW_ADDRESS, now).await;
        let mail = mails(&sent);
        let (reset_to, reset_code) = mail[0].clone();
        assert_eq!(reset_to, "alice@example.com");
        confirm(&temp, alice, &mail[1].1, now).await.unwrap();

        let through_the_new_address = ConfirmPasswordResetRequest {
            email: NEW_ADDRESS.to_string(),
            code: reset_code,
            new_password: NEW.to_string(),
            new_password_repeat: NEW.to_string(),
        };
        let result = confirm_reset(Arc::clone(&temp.db), through_the_new_address, now).await;

        assert_eq!(outcome(&result), "invalid code");
        assert_eq!(login_as(&temp, NEW_ADDRESS, OLD, now).await, "accepted");
    }

    #[tokio::test]
    async fn a_wrong_password_or_a_taken_unchanged_or_invalid_address_stores_no_code() {
        let temp = TempDatabase::open();
        let now = SystemTime::now();
        let alice = add_user(&temp, "alice").await;
        add_user(&temp, "bob").await;
        let (mailer, sent) = Mailer::capture();

        for (new_email, password, expected) in [
            (NEW_ADDRESS, "wrong horse battery", "incorrect password"),
            ("BOB@example.com", OLD, "email taken"),
            ("ALICE@example.com", OLD, "unchanged email"),
            ("not an address", OLD, "invalid email"),
        ] {
            let result = request_change(&temp, &mailer, alice, new_email, password, now).await;
            assert_eq!(requested(&result), expected, "{new_email}");
        }
        assert_eq!(code_rows(&temp), 0);
        assert!(sent.lock().unwrap().is_empty());
        assert_eq!(email_of(&temp, alice).0, "alice@example.com");
    }

    #[tokio::test]
    async fn an_address_taken_between_request_and_confirm_answers_409_and_changes_nothing() {
        let temp = TempDatabase::open();
        let now = SystemTime::now();
        let alice = add_user(&temp, "alice").await;
        let (mailer, sent) = Mailer::capture();
        change_email(&temp, &mailer, alice, "carol@example.com", now).await;
        let (_, code) = mails(&sent).remove(0);
        add_user(&temp, "carol").await;

        let result = confirm(&temp, alice, &code, now).await;

        assert_eq!(outcome(&result), "email taken");
        assert_eq!(email_of(&temp, alice).0, "alice@example.com");
        assert_eq!(code_rows(&temp), 1, "the code stays");
    }

    #[tokio::test]
    async fn without_email_the_change_applies_at_once() {
        let temp = TempDatabase::open();
        let now = SystemTime::now();
        let alice = add_user(&temp, "alice").await;

        let result = request_change(&temp, &Mailer::Disabled, alice, NEW_ADDRESS, OLD, now).await;

        assert_eq!(requested(&result), "applied");
        assert_eq!(email_of(&temp, alice), (NEW_ADDRESS.to_string(), true));
        assert_eq!(login_as(&temp, NEW_ADDRESS, OLD, now).await, "accepted");
    }

    #[tokio::test]
    async fn a_second_request_within_a_minute_sends_nothing_and_a_later_one_replaces_the_first() {
        let temp = TempDatabase::open();
        let start = SystemTime::now();
        let after = |secs: u64| start + Duration::from_secs(secs);
        let alice = add_user(&temp, "alice").await;
        let (mailer, sent) = Mailer::capture();
        let other = "alice.other@example.com";

        change_email(&temp, &mailer, alice, NEW_ADDRESS, start).await;
        change_email(&temp, &mailer, alice, other, after(30)).await;
        assert_eq!(mails(&sent).len(), 1, "within a minute of the first mail");

        change_email(&temp, &mailer, alice, other, after(61)).await;
        let mail = mails(&sent);
        assert_eq!(mail.len(), 2);
        assert_eq!(mail[1].0, other);
        let first = confirm(&temp, alice, &mail[0].1, after(62)).await;
        assert_eq!(
            outcome(&first),
            "invalid code",
            "the newer request replaced it"
        );
        confirm(&temp, alice, &mail[1].1, after(62)).await.unwrap();
        assert_eq!(email_of(&temp, alice).0, other);
    }

    #[tokio::test]
    async fn of_two_parallel_confirms_with_one_code_one_wins() {
        let temp = TempDatabase::open();
        let now = SystemTime::now();
        let alice = add_user(&temp, "alice").await;
        let (mailer, sent) = Mailer::capture();
        change_email(&temp, &mailer, alice, NEW_ADDRESS, now).await;
        let (_, code) = mails(&sent).remove(0);

        let (a, b) = tokio::join!(
            confirm(&temp, alice, &code, now),
            confirm(&temp, alice, &code, now),
        );

        let mut outcomes = [outcome(&a), outcome(&b)];
        outcomes.sort_unstable();
        assert_eq!(outcomes, ["accepted", "invalid code"]);
        assert_eq!(email_of(&temp, alice), (NEW_ADDRESS.to_string(), true));
    }
}
