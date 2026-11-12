//! Signing up, and confirming the email address with the emailed code.

use std::{
    sync::Arc,
    time::{Duration, SystemTime},
};

use rusqlite::Connection;

use crate::{
    data_models::authentication_model::{
        DeviceInformation, LoginResponse, UserCreationRequest, VerifyEmailRequest,
    },
    database_related_scripts::{
        db::convert_system_time_to_unix_time, db_email_codes, db_login_throttle, db_users,
    },
    db::Database,
    error_types::{AppErrors, AuthenticationError, DatabaseError},
    models::{
        CodePurpose, Comparison, QuerySeparator, Role, UserDatabaseColumns, UserMetadata,
        UserSettings,
    },
    routes::{
        auth::{check_email, check_new_password, check_username, hash_secret},
        email_codes::{
            check_code, code_mail, first_mail, generate_code, normalize_code, save_fresh_code,
            send_code,
        },
        email_helper::{EmailInformation, Mailer},
        login::{check_password, device_id_or_new, start_login, CheckedPassword},
        user_settings::check_locale,
    },
};

const PURPOSE: CodePurpose = CodePurpose::VerifyEmail;
/// How long a new account may stay unverified before the next sign-up deletes it.
const UNVERIFIED_LIFETIME: Duration = Duration::from_secs(48 * 60 * 60);

/// How a sign-up ended.
pub enum SignUpOutcome {
    /// The account was verified on the spot and is logged in.
    LoggedIn(LoginResponse),
    /// The account waits for the code that was just mailed.
    VerificationRequired,
}

/// What the sign-up transaction leaves to do after its commit.
enum Created {
    LoggedIn(LoginResponse),
    AwaitsCode {
        user_id: i64,
        mail: EmailInformation,
    },
}

/// The one-time code the first sign-up must carry, made at every start and logged while no
/// account exists. Only its BLAKE3 hash is kept: comparing `blake3::Hash`es takes the same
/// time however many characters match.
#[derive(Clone, Copy)]
pub struct SetupCode(blake3::Hash);

impl SetupCode {
    /// A fresh code, and its text for the server log.
    pub fn generate() -> (Self, String) {
        let text = generate_code();
        (Self::of(&text), text)
    }

    /// The code `text` stands for, typed any way `normalize_code` accepts.
    pub(crate) fn of(text: &str) -> Self {
        SetupCode(blake3::hash(normalize_code(text).as_bytes()))
    }

    pub fn matches(&self, typed: Option<&str>) -> bool {
        typed.is_some_and(|typed| Self::of(typed).0 == self.0)
    }
}

/// Creates an account. The first account must carry the setup code and becomes admin; it, and
/// every account while email is off, is verified on the spot and logged in. Any other account
/// gets its code by mail.
pub async fn create_account(
    db: Arc<Database>,
    mailer: &Mailer,
    setup_code: SetupCode,
    request: UserCreationRequest,
    now: SystemTime,
) -> Result<SignUpOutcome, AppErrors> {
    let UserCreationRequest {
        email,
        username,
        password,
        password_repeat,
        device_name,
        device_id,
        platform,
        setup_code: typed_setup_code,
        locale,
    } = request;
    check_email(&email)?;
    check_username(&username)?;
    check_new_password(&password, &password_repeat)?;

    let password_hash = hash_secret(password).await?;
    // Hashed before the transaction, which holds only SQL; whether the account is the first,
    // and so needs no code, is only known inside it.
    let code = if mailer.is_enabled() {
        let code = generate_code();
        let code_hash = hash_secret(code.clone()).await?;
        Some((code, code_hash))
    } else {
        None
    };
    let device_id = device_id_or_new(device_id);

    let created = Arc::clone(&db)
        .transact(move |conn| {
            if let Some(locale) = &locale {
                check_locale(conn, locale)?;
            }
            // First, so an address or a name held by an account nobody verified is free again.
            delete_stale_unverified(conn, now)?;
            check_name_and_email_are_free(conn, &email, &username)?;
            let first = !db_users::any_exists(conn)?;
            // Required from every address, localhost included: behind `tailscale serve` or
            // another proxy on this machine, every request arrives from 127.0.0.1.
            if first && !setup_code.matches(typed_setup_code.as_deref()) {
                return Err(AuthenticationError::InvalidSetupCode.into());
            }
            let code = code.filter(|_| !first);
            let mut user = UserMetadata {
                user_id: 0,
                username,
                password_hash,
                email,
                email_verified: code.is_none(),
                role: if first { Role::Admin } else { Role::Member },
                created_at: now,
                last_login: None,
            };
            user.user_id = Database::insert_with_connection(conn, &user)?;
            if locale.is_some() {
                let settings = UserSettings {
                    user_id: user.user_id,
                    locale,
                };
                Database::insert_with_connection(conn, &settings)?;
            }

            let Some((code, code_hash)) = code else {
                let device = DeviceInformation {
                    device_id: &device_id,
                    device_name: &device_name,
                    platform: &platform,
                };
                return Ok(Created::LoggedIn(start_login(
                    conn,
                    user.user_id,
                    device,
                    now,
                )?));
            };
            let stored = save_fresh_code(
                conn,
                user.user_id,
                PURPOSE,
                None,
                code_hash,
                first_mail(now),
                now,
            )?;
            Ok(Created::AwaitsCode {
                user_id: user.user_id,
                mail: code_mail(conn, &stored, &user, &code)?,
            })
        })
        .await?;

    match created {
        Created::LoggedIn(response) => Ok(SignUpOutcome::LoggedIn(response)),
        Created::AwaitsCode { user_id, mail } => {
            if let Err(err) = mailer.send(mail).await {
                remove_unverified(db, user_id).await;
                return Err(err.into());
            }
            Ok(SignUpOutcome::VerificationRequired)
        }
    }
}

/// Deletes the accounts still unverified `UNVERIFIED_LIFETIME` after they were created; the
/// foreign keys' cascade takes their codes with them.
fn delete_stale_unverified(conn: &Connection, now: SystemTime) -> Result<usize, DatabaseError> {
    let cutoff = convert_system_time_to_unix_time(now) - UNVERIFIED_LIFETIME.as_secs() as i64;
    Database::remove_entries_where_with_connection::<UserMetadata>(
        conn,
        &[
            (UserDatabaseColumns::EmailVerified, Comparison::Equal),
            (UserDatabaseColumns::CreatedAt, Comparison::AtMost),
        ],
        &[&false, &cutoff],
        QuerySeparator::And,
    )
}

/// 409 for an email address or a username another account already has, ignoring case.
fn check_name_and_email_are_free(
    conn: &Connection,
    email: &str,
    username: &str,
) -> Result<(), AuthenticationError> {
    let existing = Database::search_for_single_row_with_connection::<UserMetadata>(
        conn,
        &[UserDatabaseColumns::Email, UserDatabaseColumns::Username],
        &[&email, &username],
        QuerySeparator::Or,
    )
    .map_err(|_| AuthenticationError::DatabaseSearchFailure)?;
    // ASCII-only, like SQLite's NOCASE on both columns: the database found this row
    // case-insensitively, so the comparison here must agree with it.
    if let Some(user) = existing {
        if user.email.eq_ignore_ascii_case(email) {
            return Err(AuthenticationError::EmailTaken);
        }
        if user.username.eq_ignore_ascii_case(username) {
            return Err(AuthenticationError::UsernameTaken);
        }
    }
    Ok(())
}

/// Deletes an account whose code mail couldn't be sent, so signing up again works. Only
/// while it is unverified: nobody can have its code yet.
async fn remove_unverified(db: Arc<Database>, user_id: i64) {
    let removed = db
        .transact(move |conn| {
            Ok(Database::remove_entry_with_connection_based_on_columns::<
                UserMetadata,
            >(
                conn,
                &[
                    UserDatabaseColumns::UserId,
                    UserDatabaseColumns::EmailVerified,
                ],
                &[&user_id, &false],
                QuerySeparator::And,
            )?)
        })
        .await;
    if let Err(err) = removed {
        tracing::error!("removing an account whose verification mail failed: {err}");
    }
}

/// Redeems a verification code together with the password, so whoever signed up with an
/// address first keeps no working password to the account its owner then verifies. On success
/// the email is verified and the answer is login's.
pub async fn confirm_email(
    db: Arc<Database>,
    request: VerifyEmailRequest,
    now: SystemTime,
) -> Result<LoginResponse, AppErrors> {
    let VerifyEmailRequest {
        username_or_email,
        password,
        code,
        device_name,
        device_id,
        platform,
    } = request;
    let CheckedPassword { user, throttle_key } =
        check_password(Arc::clone(&db), &username_or_email, password, now).await?;
    // Only a right password gets this far, so a wrong one can't use up the code's tries.
    let Some((user, code_hash)) =
        check_code(Arc::clone(&db), PURPOSE, Some(user), code, now).await?
    else {
        return Err(AuthenticationError::InvalidValidationCode.into());
    };

    let device_id = device_id_or_new(device_id);
    db.transact(move |conn| {
        // Exactly one row: a parallel request that used the same code finds it gone.
        if !db_email_codes::consume(conn, user.user_id, PURPOSE, &code_hash)? {
            return Err(AuthenticationError::InvalidValidationCode.into());
        }
        Database::update_value_with_connection::<UserMetadata>(
            conn,
            UserDatabaseColumns::EmailVerified,
            &true,
            user.user_id,
        )?;
        db_login_throttle::clear(conn, &throttle_key)?;
        let device = DeviceInformation {
            device_id: &device_id,
            device_name: &device_name,
            platform: &platform,
        };
        start_login(conn, user.user_id, device, now)
    })
    .await
}

/// Mails a new code to the unverified account behind `name`, within the limits; see
/// [`send_code`].
pub async fn send_verification_code(
    db: Arc<Database>,
    mailer: Mailer,
    name: String,
    now: SystemTime,
) {
    send_code(db, mailer, PURPOSE, name, now).await;
}

#[cfg(test)]
mod tests {
    use std::{sync::Mutex, time::Duration};

    use rusqlite::OptionalExtension;

    use super::*;
    use crate::{
        data_models::authentication_model::{
            ConfirmPasswordResetRequest, LoginRequest, SessionTokens,
        },
        error_types::SettingsError,
        routes::{
            auth::hash_token,
            email_codes::CODE_LIFETIME,
            login::log_in,
            password_reset::{confirm_reset, send_reset_code},
        },
        test_support::{
            code_in, insert_user, memory_connection, stored_text, unreachable_mailer, TempDatabase,
            TEST_SETUP_CODE,
        },
    };

    const PASSWORD: &str = "correct horse battery";

    fn test_setup_code() -> SetupCode {
        SetupCode::of(TEST_SETUP_CODE)
    }

    /// A sign-up that carries the setup code; only the first account needs it.
    fn request(username: &str, email: &str) -> UserCreationRequest {
        UserCreationRequest {
            email: email.to_string(),
            username: username.to_string(),
            password: PASSWORD.to_string(),
            password_repeat: PASSWORD.to_string(),
            device_name: "phone".to_string(),
            device_id: None,
            platform: "ios".to_string(),
            setup_code: Some(TEST_SETUP_CODE.to_string()),
            locale: None,
        }
    }

    async fn create(
        temp: &TempDatabase,
        mailer: &Mailer,
        request: UserCreationRequest,
        now: SystemTime,
    ) -> Result<SignUpOutcome, AppErrors> {
        create_account(
            Arc::clone(&temp.db),
            mailer,
            test_setup_code(),
            request,
            now,
        )
        .await
    }

    /// Signs `username` up with the address `<username>@example.com`.
    async fn sign_up(
        temp: &TempDatabase,
        mailer: &Mailer,
        username: &str,
        now: SystemTime,
    ) -> Result<SignUpOutcome, AppErrors> {
        let email = format!("{username}@example.com");
        create(temp, mailer, request(username, &email), now).await
    }

    async fn verify(
        temp: &TempDatabase,
        name: &str,
        password: &str,
        code: &str,
        now: SystemTime,
    ) -> Result<LoginResponse, AppErrors> {
        let request = VerifyEmailRequest {
            username_or_email: name.to_string(),
            password: password.to_string(),
            code: code.to_string(),
            device_name: "phone".to_string(),
            device_id: None,
            platform: "ios".to_string(),
        };
        confirm_email(Arc::clone(&temp.db), request, now).await
    }

    async fn login(
        temp: &TempDatabase,
        name: &str,
        password: &str,
        now: SystemTime,
    ) -> Result<LoginResponse, AppErrors> {
        let request = LoginRequest {
            username_or_email: name.to_string(),
            password: password.to_string(),
            device_name: "phone".to_string(),
            device_id: None,
            platform: "ios".to_string(),
        };
        log_in(Arc::clone(&temp.db), request, now).await
    }

    async fn resend(temp: &TempDatabase, mailer: &Mailer, name: &str, now: SystemTime) {
        send_verification_code(Arc::clone(&temp.db), mailer.clone(), name.to_string(), now).await;
    }

    fn logged_in(result: Result<LoginResponse, AppErrors>) -> SessionTokens {
        match result {
            Ok(LoginResponse::Ok(tokens)) => tokens,
            Ok(LoginResponse::TotpRequired { .. }) => panic!("expected a session, got a challenge"),
            Err(err) => panic!("expected a session, got: {err}"),
        }
    }

    fn signed_in(outcome: Result<SignUpOutcome, AppErrors>) -> SessionTokens {
        match outcome {
            Ok(SignUpOutcome::LoggedIn(response)) => logged_in(Ok(response)),
            Ok(SignUpOutcome::VerificationRequired) => panic!("expected a session, got a code"),
            Err(err) => panic!("expected a session, got: {err}"),
        }
    }

    fn awaits_code(outcome: &Result<SignUpOutcome, AppErrors>) -> bool {
        matches!(outcome, Ok(SignUpOutcome::VerificationRequired))
    }

    fn failure<T>(result: &Result<T, AppErrors>) -> &'static str {
        match result {
            Ok(_) => "accepted",
            Err(AppErrors::Authentication(err)) => match err {
                AuthenticationError::EmailTaken => "email taken",
                AuthenticationError::UsernameTaken => "username taken",
                AuthenticationError::InvalidEmail => "invalid email",
                AuthenticationError::InvalidUsername => "invalid username",
                AuthenticationError::PasswordTooShort => "password too short",
                AuthenticationError::IncorrectCredentials => "incorrect",
                AuthenticationError::InvalidValidationCode => "invalid code",
                AuthenticationError::TooManyAttempts { .. } => "too many",
                AuthenticationError::EmailNotVerified => "unverified",
                AuthenticationError::InvalidSetupCode => "invalid setup code",
                _ => "other auth error",
            },
            Err(AppErrors::Email(_)) => "email error",
            Err(AppErrors::Settings(_)) => "settings error",
            Err(AppErrors::Translation(_)) => "translation error",
            Err(AppErrors::Database(_)) => "database error",
            Err(AppErrors::InvalidRequest) => "invalid request",
        }
    }

    /// The `(address, username, code)` of every mail sent so far.
    fn mails(sent: &Mutex<Vec<EmailInformation>>) -> Vec<(String, String, String)> {
        sent.lock()
            .unwrap()
            .iter()
            .map(|mail| (mail.email.clone(), mail.username.clone(), code_in(mail)))
            .collect()
    }

    /// `(is an admin, email_verified)` of the account, or `None` when it doesn't exist.
    fn account(temp: &TempDatabase, username: &str) -> Option<(bool, bool)> {
        temp.db
            .setup_new_transaction(|conn| {
                Ok(conn
                    .query_row(
                        "SELECT role = 'admin', email_verified FROM users WHERE username = ?1",
                        [username],
                        |row| Ok((row.get(0)?, row.get(1)?)),
                    )
                    .optional()
                    .unwrap())
            })
            .unwrap()
    }

    fn count(temp: &TempDatabase, sql: &str) -> i64 {
        temp.db
            .setup_new_transaction(|conn| Ok(conn.query_row(sql, [], |row| row.get(0)).unwrap()))
            .unwrap()
    }

    fn sessions_of_bob(temp: &TempDatabase) -> i64 {
        count(
            temp,
            "SELECT COUNT(*) FROM sessions JOIN users ON users.id = sessions.user_id \
             WHERE users.username = 'bob'",
        )
    }

    fn session_exists(temp: &TempDatabase, session_token: &str) -> bool {
        let hash = hash_token(session_token);
        temp.db
            .setup_new_transaction(|conn| {
                let found: i64 = conn
                    .query_row(
                        "SELECT COUNT(*) FROM sessions WHERE session_token_hashed = ?1",
                        [&hash],
                        |row| row.get(0),
                    )
                    .unwrap();
                Ok(found == 1)
            })
            .unwrap()
    }

    /// Tries used on the one verification code in the database.
    fn code_attempts(temp: &TempDatabase) -> i64 {
        count(
            temp,
            "SELECT attempts FROM email_codes WHERE purpose = 'verify_email'",
        )
    }

    /// Alice, the first account, and Bob, signed up with email on; returns Bob's mailed code.
    async fn bob_waiting_for_his_code(
        temp: &TempDatabase,
        mailer: &Mailer,
        sent: &Mutex<Vec<EmailInformation>>,
        now: SystemTime,
    ) -> String {
        signed_in(sign_up(temp, mailer, "alice", now).await);
        assert!(awaits_code(&sign_up(temp, mailer, "bob", now).await));
        mails(sent).pop().expect("bob's code mail").2
    }

    // === Sign-up =============================================================

    #[tokio::test]
    async fn the_first_sign_up_becomes_a_verified_admin_and_is_logged_in() {
        let temp = TempDatabase::open();
        let (mailer, sent) = Mailer::capture();

        let session = signed_in(sign_up(&temp, &mailer, "alice", SystemTime::now()).await);

        assert_eq!(account(&temp, "alice"), Some((true, true)));
        assert!(session_exists(&temp, &session.session_token));
        assert!(mails(&sent).is_empty(), "the first account gets no mail");
        assert_eq!(count(&temp, "SELECT COUNT(*) FROM email_codes"), 0);
    }

    #[tokio::test]
    async fn a_later_sign_up_with_email_waits_for_its_mailed_code() {
        let temp = TempDatabase::open();
        let (mailer, sent) = Mailer::capture();
        let now = SystemTime::now();
        signed_in(sign_up(&temp, &mailer, "alice", now).await);

        let outcome = sign_up(&temp, &mailer, "bob", now).await;

        assert!(awaits_code(&outcome));
        assert_eq!(account(&temp, "bob"), Some((false, false)));
        let mail = mails(&sent);
        assert_eq!(mail.len(), 1);
        let (address, username, code) = &mail[0];
        assert_eq!(address, "bob@example.com");
        assert_eq!(username, "bob");
        assert_eq!(code.len(), 8);
        assert_eq!(code_attempts(&temp), 0);
        assert_eq!(
            failure(&login(&temp, "bob", PASSWORD, now).await),
            "unverified"
        );
    }

    #[tokio::test]
    async fn later_sign_ups_without_email_are_verified_on_the_spot() {
        let temp = TempDatabase::open();
        let now = SystemTime::now();
        signed_in(sign_up(&temp, &Mailer::Disabled, "alice", now).await);

        for name in ["bob", "carol"] {
            signed_in(sign_up(&temp, &Mailer::Disabled, name, now).await);
            assert_eq!(account(&temp, name), Some((false, true)));
        }
    }

    #[tokio::test]
    async fn only_the_very_first_account_becomes_admin() {
        let temp = TempDatabase::open();
        // An account but no admin, as if the admin row had been lost.
        temp.db
            .setup_new_transaction(|conn| Ok(insert_user(conn, "member")))
            .unwrap();

        signed_in(sign_up(&temp, &Mailer::Disabled, "bob", SystemTime::now()).await);
        assert_eq!(account(&temp, "bob"), Some((false, true)));
    }

    #[tokio::test]
    async fn of_two_parallel_first_sign_ups_exactly_one_becomes_admin() {
        let temp = TempDatabase::open();
        let now = SystemTime::now();

        let (alice, bob) = tokio::join!(
            sign_up(&temp, &Mailer::Disabled, "alice", now),
            sign_up(&temp, &Mailer::Disabled, "bob", now),
        );

        signed_in(alice);
        signed_in(bob);
        assert_eq!(
            count(&temp, "SELECT COUNT(*) FROM users WHERE role = 'admin'"),
            1
        );
    }

    #[tokio::test]
    async fn of_two_parallel_sign_ups_with_the_same_details_one_gets_409() {
        let temp = TempDatabase::open();
        let now = SystemTime::now();
        signed_in(sign_up(&temp, &Mailer::Disabled, "alice", now).await);

        let (first, second) = tokio::join!(
            sign_up(&temp, &Mailer::Disabled, "bob", now),
            sign_up(&temp, &Mailer::Disabled, "bob", now),
        );

        let mut outcomes = [failure(&first), failure(&second)];
        outcomes.sort();
        assert_eq!(outcomes, ["accepted", "email taken"], "never a 500");
    }

    fn with_alice() -> Connection {
        let conn = memory_connection();
        conn.execute(
            "INSERT INTO users (username, password_hash, email) \
             VALUES ('Alice', 'hash', 'Alice@Example.com')",
            [],
        )
        .unwrap();
        conn
    }

    #[test]
    fn an_email_differing_only_in_case_is_taken() {
        let conn = with_alice();
        let result = check_name_and_email_are_free(&conn, "alice@example.com", "bob");
        assert!(matches!(result, Err(AuthenticationError::EmailTaken)));
    }

    #[test]
    fn a_username_differing_only_in_case_is_taken() {
        let conn = with_alice();
        let result = check_name_and_email_are_free(&conn, "other@example.com", "ALICE");
        assert!(matches!(result, Err(AuthenticationError::UsernameTaken)));
    }

    #[test]
    fn a_new_username_and_email_pass() {
        let conn = with_alice();
        assert!(check_name_and_email_are_free(&conn, "bob@example.com", "bob").is_ok());
    }

    #[tokio::test]
    async fn bad_details_are_refused_before_anything_is_written() {
        let temp = TempDatabase::open();
        let mut short_password = request("bob", "bob@example.com");
        short_password.password = "too short".to_string();
        short_password.password_repeat = "too short".to_string();
        let cases = [
            (request("bob", "bob@example.com\r\nDATA"), "invalid email"),
            (request("bob", "<bob@example.com>"), "invalid email"),
            (request("bob", "bob@example@com"), "invalid email"),
            (request("bo", "bob@example.com"), "invalid username"),
            (request("bob@home", "bob@example.com"), "invalid username"),
            (request("bob:1", "bob@example.com"), "invalid username"),
            (short_password, "password too short"),
        ];

        for (request, expected) in cases {
            let result = create(&temp, &Mailer::Disabled, request, SystemTime::now()).await;
            assert_eq!(failure(&result), expected);
        }
        assert_eq!(count(&temp, "SELECT COUNT(*) FROM users"), 0);
    }

    fn settings_rows(temp: &TempDatabase) -> Vec<(String, Option<String>)> {
        temp.db
            .setup_new_transaction(|conn| {
                Ok(conn
                    .prepare(
                        "SELECT users.username, user_settings.locale FROM user_settings \
                         JOIN users ON users.id = user_settings.user_id ORDER BY users.username",
                    )
                    .unwrap()
                    .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
                    .unwrap()
                    .collect::<Result<_, _>>()
                    .unwrap())
            })
            .unwrap()
    }

    #[tokio::test]
    async fn a_sign_up_stores_the_language_it_names_from_the_first_account_on() {
        let temp = TempDatabase::open();
        let now = SystemTime::now();
        for (name, locale) in [("alice", "de"), ("bob", "en")] {
            let mut request = request(name, &format!("{name}@example.com"));
            request.locale = Some(locale.to_string());
            signed_in(create(&temp, &Mailer::Disabled, request, now).await);
        }

        assert_eq!(
            settings_rows(&temp),
            [
                ("alice".to_string(), Some("de".to_string())),
                ("bob".to_string(), Some("en".to_string())),
            ]
        );
    }

    #[tokio::test]
    async fn a_sign_up_naming_a_language_the_server_doesnt_offer_creates_nothing() {
        let temp = TempDatabase::open();
        for locale in ["fr", "DE", ""] {
            let mut request = request("alice", "alice@example.com");
            request.locale = Some(locale.to_string());

            let result = create(&temp, &Mailer::Disabled, request, SystemTime::now()).await;

            assert!(
                matches!(
                    result,
                    Err(AppErrors::Settings(SettingsError::InvalidLocale))
                ),
                "{locale:?}"
            );
        }
        assert_eq!(count(&temp, "SELECT COUNT(*) FROM users"), 0);
        assert!(settings_rows(&temp).is_empty());
    }

    #[tokio::test]
    async fn the_verification_mail_is_in_the_language_named_at_sign_up() {
        let temp = TempDatabase::open();
        let (mailer, sent) = Mailer::capture();
        let now = SystemTime::now();
        signed_in(sign_up(&temp, &mailer, "alice", now).await);

        let mut german = request("bob", "bob@example.com");
        german.locale = Some("de".to_string());
        assert!(awaits_code(&create(&temp, &mailer, german, now).await));

        let expected = temp
            .db
            .setup_new_transaction(|conn| {
                Ok(stored_text(conn, "de", "email", "verify_email.subject"))
            })
            .unwrap();
        let sent = sent.lock().unwrap();
        assert_eq!(sent.len(), 1);
        assert_eq!(sent[0].subject, expected);
        assert_eq!(
            code_in(&sent[0]).len(),
            8,
            "the German mail carries the code"
        );
    }

    #[tokio::test]
    async fn a_sign_up_without_a_language_leaves_it_unchosen() {
        let temp = TempDatabase::open();

        signed_in(sign_up(&temp, &Mailer::Disabled, "alice", SystemTime::now()).await);

        assert!(settings_rows(&temp).is_empty());
    }

    #[tokio::test]
    async fn the_first_sign_up_needs_the_setup_code() {
        let temp = TempDatabase::open();
        let now = SystemTime::now();
        let with_code = |code: Option<String>| {
            let mut request = request("alice", "alice@example.com");
            request.setup_code = code;
            request
        };

        for refused in [None, Some("00000000".to_string())] {
            let result = create(&temp, &Mailer::Disabled, with_code(refused), now).await;
            assert_eq!(failure(&result), "invalid setup code");
        }
        assert_eq!(
            count(&temp, "SELECT COUNT(*) FROM users"),
            0,
            "nothing written"
        );

        let (start, end) = TEST_SETUP_CODE.split_at(4);
        let typed_casually = format!("{start}-{end}").to_lowercase();
        signed_in(
            create(
                &temp,
                &Mailer::Disabled,
                with_code(Some(typed_casually)),
                now,
            )
            .await,
        );
        assert_eq!(account(&temp, "alice"), Some((true, true)));
    }

    #[tokio::test]
    async fn later_sign_ups_ignore_the_setup_code() {
        let temp = TempDatabase::open();
        let now = SystemTime::now();
        signed_in(sign_up(&temp, &Mailer::Disabled, "alice", now).await);

        for (name, code) in [("bob", None), ("carol", Some("00000000".to_string()))] {
            let mut request = request(name, &format!("{name}@example.com"));
            request.setup_code = code;
            signed_in(create(&temp, &Mailer::Disabled, request, now).await);
        }
    }

    #[test]
    fn a_setup_code_matches_only_its_own_text() {
        let (code, text) = SetupCode::generate();
        assert!(code.matches(Some(&text)));
        assert!(code.matches(Some(&text.to_lowercase())));
        // `0` is not in the code alphabet, so this can never be the code.
        assert!(!code.matches(Some("00000000")));
        assert!(!code.matches(None));

        let (_, other) = SetupCode::generate();
        assert_ne!(text, other, "every start makes a new code");
    }

    #[tokio::test]
    async fn a_code_mail_that_cannot_be_sent_undoes_the_sign_up() {
        let temp = TempDatabase::open();
        let now = SystemTime::now();
        signed_in(sign_up(&temp, &Mailer::Disabled, "alice", now).await);

        let failed = sign_up(&temp, &unreachable_mailer(), "bob", now).await;

        assert_eq!(failure(&failed), "email error");
        assert_eq!(account(&temp, "bob"), None, "the account is gone again");
        assert_eq!(count(&temp, "SELECT COUNT(*) FROM email_codes"), 0);
        let (mailer, _sent) = Mailer::capture();
        assert!(
            awaits_code(&sign_up(&temp, &mailer, "bob", now).await),
            "signing up again works"
        );
    }

    const FORTY_EIGHT_HOURS: Duration = Duration::from_secs(48 * 60 * 60);

    #[tokio::test]
    async fn a_sign_up_deletes_the_accounts_still_unverified_48_hours_after_their_sign_up() {
        let temp = TempDatabase::open();
        let (mailer, sent) = Mailer::capture();
        let start = SystemTime::now();
        signed_in(sign_up(&temp, &mailer, "alice", start).await);
        for name in ["bob", "carol", "dave"] {
            assert!(awaits_code(&sign_up(&temp, &mailer, name, start).await));
        }
        let (_, _, carols_code) = mails(&sent)
            .into_iter()
            .find(|(_, name, _)| name == "carol")
            .expect("carol's code mail");
        logged_in(verify(&temp, "carol", PASSWORD, &carols_code, start).await);

        let exactly_48_hours_later = start + FORTY_EIGHT_HOURS;
        let again = sign_up(&temp, &mailer, "bob", exactly_48_hours_later).await;

        assert!(awaits_code(&again), "bob's address and name are free again");
        assert_eq!(account(&temp, "dave"), None, "every such account goes");
        assert_eq!(account(&temp, "alice"), Some((true, true)));
        assert_eq!(account(&temp, "carol"), Some((false, true)));
        assert_eq!(
            count(&temp, "SELECT COUNT(*) FROM email_codes"),
            1,
            "only the new account's code is left"
        );
    }

    #[tokio::test]
    async fn an_account_unverified_for_a_second_less_keeps_its_address_and_name() {
        let temp = TempDatabase::open();
        let (mailer, _sent) = Mailer::capture();
        let start = SystemTime::now();
        signed_in(sign_up(&temp, &mailer, "alice", start).await);
        assert!(awaits_code(&sign_up(&temp, &mailer, "bob", start).await));
        let a_second_short = start + FORTY_EIGHT_HOURS - Duration::from_secs(1);

        let same_address = create(
            &temp,
            &mailer,
            request("robert", "bob@example.com"),
            a_second_short,
        )
        .await;
        let same_name = create(
            &temp,
            &mailer,
            request("bob", "robert@example.com"),
            a_second_short,
        )
        .await;

        assert_eq!(failure(&same_address), "email taken");
        assert_eq!(failure(&same_name), "username taken");
        assert_eq!(account(&temp, "bob"), Some((false, false)));
    }

    // === Verification ========================================================

    #[tokio::test]
    async fn the_mailed_code_with_the_password_verifies_and_logs_in_once() {
        let temp = TempDatabase::open();
        let (mailer, sent) = Mailer::capture();
        let now = SystemTime::now();
        let code = bob_waiting_for_his_code(&temp, &mailer, &sent, now).await;

        let typed_casually = format!("{}-{}", &code[..4], &code[4..]).to_lowercase();
        let session = logged_in(verify(&temp, "bob", PASSWORD, &typed_casually, now).await);

        assert!(session_exists(&temp, &session.session_token));
        assert_eq!(account(&temp, "bob"), Some((false, true)));
        assert_eq!(count(&temp, "SELECT COUNT(*) FROM email_codes"), 0);
        assert_eq!(
            failure(&verify(&temp, "bob", PASSWORD, &code, now).await),
            "invalid code",
            "a code works only once"
        );
        logged_in(login(&temp, "bob@example.com", PASSWORD, now).await);
    }

    #[tokio::test]
    async fn a_wrong_password_is_refused_before_the_code_is_tried() {
        let temp = TempDatabase::open();
        let (mailer, sent) = Mailer::capture();
        let now = SystemTime::now();
        let code = bob_waiting_for_his_code(&temp, &mailer, &sent, now).await;

        let result = verify(&temp, "bob", "wrong horse battery", &code, now).await;

        assert_eq!(failure(&result), "incorrect");
        assert_eq!(code_attempts(&temp), 0, "the code's tries are untouched");
        assert_eq!(account(&temp, "bob"), Some((false, false)));
    }

    #[tokio::test]
    async fn wrong_passwords_at_verification_count_toward_the_login_throttle() {
        let temp = TempDatabase::open();
        let (mailer, sent) = Mailer::capture();
        let now = SystemTime::now();
        let code = bob_waiting_for_his_code(&temp, &mailer, &sent, now).await;

        for _ in 0..10 {
            let result = verify(&temp, "bob", "wrong horse battery", &code, now).await;
            assert_eq!(failure(&result), "incorrect");
        }

        assert_eq!(
            failure(&verify(&temp, "bob", PASSWORD, &code, now).await),
            "too many"
        );
        assert_eq!(
            failure(&login(&temp, "bob", PASSWORD, now).await),
            "too many"
        );
    }

    #[tokio::test]
    async fn five_wrong_codes_end_the_code() {
        let temp = TempDatabase::open();
        let (mailer, sent) = Mailer::capture();
        let now = SystemTime::now();
        let code = bob_waiting_for_his_code(&temp, &mailer, &sent, now).await;

        // `0` is not in the code alphabet, so this can never be the real code.
        for _ in 0..5 {
            let result = verify(&temp, "bob", PASSWORD, "00000000", now).await;
            assert_eq!(failure(&result), "invalid code");
        }
        assert_eq!(code_attempts(&temp), 5, "every wrong code stays counted");

        let right_but_too_late = verify(&temp, "bob", PASSWORD, &code, now).await;
        assert_eq!(failure(&right_but_too_late), "invalid code");
        assert_eq!(account(&temp, "bob"), Some((false, false)));
    }

    #[tokio::test]
    async fn an_expired_code_is_refused() {
        let temp = TempDatabase::open();
        let (mailer, sent) = Mailer::capture();
        let now = SystemTime::now();
        let code = bob_waiting_for_his_code(&temp, &mailer, &sent, now).await;

        let late = now + CODE_LIFETIME + Duration::from_secs(1);
        assert_eq!(
            failure(&verify(&temp, "bob", PASSWORD, &code, late).await),
            "invalid code"
        );
        assert_eq!(account(&temp, "bob"), Some((false, false)));
    }

    #[tokio::test]
    async fn a_verified_account_has_no_code_to_redeem() {
        let temp = TempDatabase::open();
        let now = SystemTime::now();
        signed_in(sign_up(&temp, &Mailer::Disabled, "alice", now).await);

        let result = verify(&temp, "alice", PASSWORD, "ABCDEFGH", now).await;
        assert_eq!(failure(&result), "invalid code");
    }

    #[tokio::test]
    async fn two_parallel_verifications_with_the_right_code_start_one_session() {
        let temp = TempDatabase::open();
        let (mailer, sent) = Mailer::capture();
        let now = SystemTime::now();
        let code = bob_waiting_for_his_code(&temp, &mailer, &sent, now).await;

        let (a, b) = tokio::join!(
            verify(&temp, "bob", PASSWORD, &code, now),
            verify(&temp, "bob", PASSWORD, &code, now),
        );

        assert_eq!([a.is_ok(), b.is_ok()].iter().filter(|ok| **ok).count(), 1);
        assert_eq!(sessions_of_bob(&temp), 1);
    }

    #[tokio::test]
    async fn verifying_an_account_with_totp_gives_a_challenge_not_a_session() {
        let temp = TempDatabase::open();
        let (mailer, sent) = Mailer::capture();
        let now = SystemTime::now();
        let code = bob_waiting_for_his_code(&temp, &mailer, &sent, now).await;
        temp.db
            .setup_new_transaction(|conn| {
                conn.execute(
                    "INSERT INTO totp (user_id, authentication_secret, confirmed) \
                     SELECT id, 'secret', 1 FROM users WHERE username = 'bob'",
                    [],
                )
                .unwrap();
                Ok(())
            })
            .unwrap();

        let result = verify(&temp, "bob", PASSWORD, &code, now).await;

        assert!(matches!(result, Ok(LoginResponse::TotpRequired { .. })));
        assert_eq!(sessions_of_bob(&temp), 0, "no session before the TOTP code");
    }

    #[tokio::test]
    async fn a_hijacked_sign_up_goes_to_the_address_owner_through_a_password_reset() {
        let temp = TempDatabase::open();
        let (mailer, sent) = Mailer::capture();
        let start = SystemTime::now();
        let at = |secs: u64| start + Duration::from_secs(secs);
        signed_in(sign_up(&temp, &mailer, "alice", start).await);

        // Mallory signs up first, with the victim's address and a password of her own.
        let mut hijack = request("mallory", "victim@example.com");
        hijack.password = "mallory's password".to_string();
        hijack.password_repeat = "mallory's password".to_string();
        let outcome = create(&temp, &mailer, hijack, start).await;
        assert!(awaits_code(&outcome));
        let verification_code = mails(&sent)[0].2.clone();

        // The code reaches the victim's mailbox, but without Mallory's password it opens nothing.
        let victims_password = "the victim's own password";
        let refused = verify(
            &temp,
            "victim@example.com",
            victims_password,
            &verification_code,
            at(10),
        )
        .await;
        assert_eq!(failure(&refused), "incorrect");

        send_reset_code(
            Arc::clone(&temp.db),
            mailer.clone(),
            "victim@example.com".to_string(),
            at(20),
        )
        .await;
        let reset = ConfirmPasswordResetRequest {
            email: "victim@example.com".to_string(),
            code: mails(&sent)[1].2.clone(),
            new_password: victims_password.to_string(),
            new_password_repeat: victims_password.to_string(),
        };
        confirm_reset(Arc::clone(&temp.db), reset, at(30))
            .await
            .unwrap();

        // The reset code reached the victim's mailbox too, so the reset verified the address.
        logged_in(login(&temp, "victim@example.com", victims_password, at(40)).await);
        assert_eq!(
            failure(
                &verify(
                    &temp,
                    "victim@example.com",
                    victims_password,
                    &verification_code,
                    at(40),
                )
                .await
            ),
            "invalid code",
            "a verified account has no code left to redeem"
        );
        assert_eq!(
            failure(&login(&temp, "mallory", "mallory's password", at(50)).await),
            "incorrect"
        );
    }

    // === Resend ==============================================================

    #[tokio::test]
    async fn a_resent_code_replaces_the_old_one() {
        let temp = TempDatabase::open();
        let (mailer, sent) = Mailer::capture();
        let start = SystemTime::now();
        let old_code = bob_waiting_for_his_code(&temp, &mailer, &sent, start).await;
        let later = start + Duration::from_secs(61);

        resend(&temp, &mailer, "bob", later).await;

        let mail = mails(&sent);
        assert_eq!(mail.len(), 2);
        assert_eq!(mail[1].0, "bob@example.com");
        assert_eq!(
            failure(&verify(&temp, "bob", PASSWORD, &old_code, later).await),
            "invalid code"
        );
        logged_in(verify(&temp, "bob", PASSWORD, &mail[1].2, later).await);
    }

    #[tokio::test]
    async fn resending_keeps_to_the_limits_and_counts_the_sign_up_mail() {
        let temp = TempDatabase::open();
        let (mailer, sent) = Mailer::capture();
        let start = SystemTime::now();
        let after = |secs: u64| start + Duration::from_secs(secs);
        bob_waiting_for_his_code(&temp, &mailer, &sent, start).await;
        let sent_so_far = || mails(&sent).len();

        resend(&temp, &mailer, "bob", after(30)).await;
        assert_eq!(sent_so_far(), 1, "within a minute of the sign-up mail");

        for secs in [61, 122, 183, 244] {
            resend(&temp, &mailer, "BOB@example.com", after(secs)).await;
        }
        assert_eq!(sent_so_far(), 5);

        resend(&temp, &mailer, "bob", after(305)).await;
        assert_eq!(sent_so_far(), 5, "a sixth mail on the same day");

        resend(&temp, &mailer, "bob", after(24 * 60 * 60)).await;
        assert_eq!(sent_so_far(), 6, "a new day allows a new mail");
    }

    #[tokio::test]
    async fn nothing_is_resent_for_a_verified_or_unknown_account() {
        let temp = TempDatabase::open();
        let (mailer, sent) = Mailer::capture();
        let now = SystemTime::now();
        signed_in(sign_up(&temp, &mailer, "alice", now).await);

        for name in ["alice", "alice@example.com", "nobody", "nobody@example.com"] {
            resend(&temp, &mailer, name, now).await;
        }
        assert!(mails(&sent).is_empty());
    }
}
