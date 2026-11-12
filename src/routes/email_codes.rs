//! Codes sent by email, for password reset, email verification and email changes: making them,
//! mailing them within the limits on how often one may go out, and checking a typed code.

use std::{
    sync::Arc,
    time::{Duration, SystemTime},
};

use rand::RngExt;
use rusqlite::Connection;

use crate::{
    database_related_scripts::{db_email_codes, db_translations},
    db::Database,
    error_types::{AppErrors, DatabaseError, TranslationError},
    models::{CodePurpose, EmailCode, QuerySeparator, UserDatabaseColumns, UserMetadata},
    routes::{
        auth::{hash_secret, secret_matches},
        email_helper::{EmailInformation, Mailer},
        login::find_user_for_login,
        translations::interpolate,
    },
};

/// The namespace of the code mails' texts: `translations/<language>/backend/email.json`.
const MAIL_TEXTS: &str = "email";

// No 0/O or 1/I/L: the code is read from an email and typed on a phone.
const CODE_ALPHABET: &[u8] = b"ABCDEFGHJKMNPQRSTUVWXYZ23456789";
const CODE_LENGTH: usize = 8;

pub const CODE_LIFETIME: Duration = Duration::from_secs(15 * 60);
pub const MAX_ATTEMPTS: i64 = 5;
const RESEND_COOLDOWN: Duration = Duration::from_secs(60);
const MAILS_PER_WINDOW: i64 = 5;
const MAIL_WINDOW: Duration = Duration::from_secs(24 * 60 * 60);

pub fn generate_code() -> String {
    let mut rng = rand::rng();
    (0..CODE_LENGTH)
        .map(|_| CODE_ALPHABET[rng.random_range(0..CODE_ALPHABET.len())] as char)
        .collect()
}

/// The code as people type it: any case, with spaces or dashes.
pub fn normalize_code(input: &str) -> String {
    input
        .chars()
        .filter(|c| !c.is_whitespace() && *c != '-')
        .map(|c| c.to_ascii_uppercase())
        .collect()
}

/// The counters to store when one more mail goes out now.
#[derive(Debug, PartialEq, Eq)]
pub struct MailCounters {
    pub sent_in_window: i64,
    pub window_started_at: SystemTime,
}

/// The counters for a mail that starts a new 24-hour window of the daily mail cap: the
/// account's first code mail, or the first one once the previous 24 hours are over.
pub fn first_mail(now: SystemTime) -> MailCounters {
    MailCounters {
        sent_in_window: 1,
        window_started_at: now,
    }
}

/// The counters for one more mail now, or `None` while the limits forbid it: the last mail
/// went out less than a minute ago, or the day's mails are used up.
pub fn counters_for_next_mail(
    existing: Option<&EmailCode>,
    now: SystemTime,
) -> Option<MailCounters> {
    let Some(code) = existing else {
        return Some(first_mail(now));
    };
    if elapsed(code.last_sent_at, now) < RESEND_COOLDOWN {
        return None;
    }
    if elapsed(code.window_started_at, now) >= MAIL_WINDOW {
        return Some(first_mail(now));
    }
    if code.sent_in_window >= MAILS_PER_WINDOW {
        return None;
    }
    Some(MailCounters {
        sent_in_window: code.sent_in_window + 1,
        window_started_at: code.window_started_at,
    })
}

// A clock that went backwards counts as no time passed, so the limits stay in force.
pub(crate) fn elapsed(since: SystemTime, now: SystemTime) -> Duration {
    now.duration_since(since).unwrap_or(Duration::ZERO)
}

/// Who a code of this purpose is for: any account can reset its password or change its email,
/// but only an unverified one can verify its email.
fn is_for(purpose: CodePurpose, user: &UserMetadata) -> bool {
    match purpose {
        CodePurpose::ResetPassword | CodePurpose::ChangeEmail => true,
        CodePurpose::VerifyEmail => !user.email_verified,
    }
}

/// The account a code request names: a reset request takes only the email, a verification
/// takes what login takes. An email change comes from a session, never from a name.
pub fn find_account(
    conn: &Connection,
    purpose: CodePurpose,
    name: &str,
) -> Result<Option<UserMetadata>, DatabaseError> {
    match purpose {
        CodePurpose::ResetPassword => find_user_by_email(conn, name),
        CodePurpose::VerifyEmail => find_user_for_login(conn, name.trim()),
        CodePurpose::ChangeEmail => Ok(None),
    }
}

fn find_user_by_email(
    conn: &Connection,
    email: &str,
) -> Result<Option<UserMetadata>, DatabaseError> {
    Database::search_for_single_row_with_connection::<UserMetadata>(
        conn,
        &[UserDatabaseColumns::Email],
        &[&email],
        QuerySeparator::And,
    )
}

/// Issues a code for the account behind `name` and mails it. The handler has already
/// answered, so every outcome ends here: no such account, an account the code isn't for, or a
/// request over the limits simply sends nothing.
pub async fn send_code(
    db: Arc<Database>,
    mailer: Mailer,
    purpose: CodePurpose,
    name: String,
    now: SystemTime,
) {
    match issue_code(db, purpose, name, now).await {
        Ok(Some(mail)) => {
            if let Err(err) = mailer.send(mail).await {
                tracing::error!("sending a {} email failed: {err}", purpose.as_str());
            }
        }
        Ok(None) => {}
        Err(err) => tracing::error!("issuing a {} code failed: {err}", purpose.as_str()),
    }
}

/// Stores a fresh code for the account behind `name` and returns the mail carrying it,
/// or `None` when no mail may go out.
async fn issue_code(
    db: Arc<Database>,
    purpose: CodePurpose,
    name: String,
    now: SystemTime,
) -> Result<Option<EmailInformation>, AppErrors> {
    // Checked before hashing, so requests for unknown names cost no Argon2 work.
    let lookup = name.clone();
    let may_send = Arc::clone(&db)
        .transact(move |conn| Ok(next_mail(conn, purpose, &lookup, now)?.is_some()))
        .await?;
    if !may_send {
        return Ok(None);
    }

    let code = generate_code();
    let code_hash = hash_secret(code.clone()).await?;

    // Checked again inside the write: a parallel request may have sent a mail meanwhile, or a
    // verification may have finished.
    db.transact(move |conn| {
        let Some((user, counters)) = next_mail(conn, purpose, &name, now)? else {
            return Ok(None);
        };
        let stored = save_fresh_code(conn, user.user_id, purpose, None, code_hash, counters, now)?;
        Ok(Some(code_mail(conn, &stored, &user, &code)?))
    })
    .await
}

/// The account behind `name` and its counters for one more mail; `None` when there is no
/// such account, the code isn't for it, or the limits forbid a mail now.
fn next_mail(
    conn: &Connection,
    purpose: CodePurpose,
    name: &str,
    now: SystemTime,
) -> Result<Option<(UserMetadata, MailCounters)>, DatabaseError> {
    let Some(user) = find_account(conn, purpose, name)?.filter(|user| is_for(purpose, user)) else {
        return Ok(None);
    };
    let existing = db_email_codes::find(conn, user.user_id, purpose)?;
    Ok(counters_for_next_mail(existing.as_ref(), now).map(|counters| (user, counters)))
}

/// Stores the hash of a code that is mailed now, replacing the user's earlier code for
/// this purpose; it starts with no tries used. `new_email` is the address an email change waits
/// to confirm. Answers the stored code, for `code_mail` to address.
pub fn save_fresh_code(
    conn: &Connection,
    user_id: i64,
    purpose: CodePurpose,
    new_email: Option<String>,
    code_hash: String,
    counters: MailCounters,
    now: SystemTime,
) -> Result<EmailCode, DatabaseError> {
    let code = EmailCode {
        user_id,
        purpose,
        code_hash,
        new_email,
        expires_at: now + CODE_LIFETIME,
        attempts: 0,
        last_sent_at: now,
        sent_in_window: counters.sent_in_window,
        window_started_at: counters.window_started_at,
    };
    db_email_codes::save(conn, &code)?;
    Ok(code)
}

/// The mail that carries a stored code, in the recipient's language: to the address stored with
/// it for an email change, to the account's stored address otherwise — never to one straight
/// from a request.
pub fn code_mail(
    conn: &Connection,
    stored: &EmailCode,
    user: &UserMetadata,
    code: &str,
) -> Result<EmailInformation, AppErrors> {
    let locale = db_translations::recipient_locale(conn, user.user_id)?;
    let minutes = (CODE_LIFETIME.as_secs() / 60).to_string();
    let values = [
        ("username", user.username.as_str()),
        ("code", code),
        ("minutes", minutes.as_str()),
    ];
    let text = |part: &str| -> Result<String, AppErrors> {
        let name = format!("{}.{part}", stored.purpose.as_str());
        let template =
            db_translations::text(conn, &locale, MAIL_TEXTS, &name)?.ok_or_else(|| {
                TranslationError::MissingText {
                    key: format!("{MAIL_TEXTS}.{name}"),
                }
            })?;
        Ok(interpolate(&template, &values))
    };
    Ok(EmailInformation {
        username: user.username.clone(),
        email: stored
            .new_email
            .clone()
            .unwrap_or_else(|| user.email.clone()),
        subject: text("subject")?,
        body: text("body")?,
    })
}

/// Counts a try at the user's live code, then checks `code` against it. On success returns
/// the user and the code's hash, which the caller's final write consumes. Without a user or a
/// live code it checks against a dummy hash, so every failure takes as long.
pub async fn check_code(
    db: Arc<Database>,
    purpose: CodePurpose,
    user: Option<UserMetadata>,
    code: String,
    now: SystemTime,
) -> Result<Option<(UserMetadata, String)>, AppErrors> {
    // The try is counted and committed before the code is checked: counted in the same
    // transaction as a failed check, it would be rolled back with it.
    let reserved = db
        .transact(move |conn| {
            let Some(user) = user.filter(|user| is_for(purpose, user)) else {
                return Ok(None);
            };
            if !db_email_codes::reserve_attempt(conn, user.user_id, purpose, now, MAX_ATTEMPTS)? {
                return Ok(None);
            }
            let stored = db_email_codes::find(conn, user.user_id, purpose)?;
            Ok(stored.map(|row| (user, row.code_hash)))
        })
        .await?;

    let stored_hash = reserved.as_ref().map(|(_, hash)| hash.clone());
    let code_is_right = secret_matches(normalize_code(&code), stored_hash).await?;
    Ok(reserved.filter(|_| code_is_right))
}

#[cfg(test)]
mod tests {
    use std::time::UNIX_EPOCH;

    use super::*;
    use crate::{
        models::{CodePurpose, Role},
        test_support::{code_in, insert_user, memory_connection, stored_text},
    };

    fn at(secs: u64) -> SystemTime {
        UNIX_EPOCH + Duration::from_secs(secs)
    }

    const PURPOSES: [CodePurpose; 3] = [
        CodePurpose::ResetPassword,
        CodePurpose::VerifyEmail,
        CodePurpose::ChangeEmail,
    ];

    /// An account with this language setting, or none.
    fn account(conn: &Connection, username: &str, locale: Option<&str>) -> UserMetadata {
        let user_id = insert_user(conn, username);
        if let Some(locale) = locale {
            conn.execute(
                "INSERT INTO user_settings (user_id, locale) VALUES (?1, ?2)",
                rusqlite::params![user_id, locale],
            )
            .unwrap();
        }
        UserMetadata {
            user_id,
            username: username.to_string(),
            password_hash: "not-a-hash".to_string(),
            email: format!("{username}@example.com"),
            email_verified: false,
            role: Role::Member,
            created_at: UNIX_EPOCH,
            last_login: None,
        }
    }

    fn stored_code(user_id: i64, purpose: CodePurpose, new_email: Option<&str>) -> EmailCode {
        EmailCode {
            user_id,
            purpose,
            code_hash: "hash".to_string(),
            new_email: new_email.map(str::to_string),
            expires_at: at(900),
            attempts: 0,
            last_sent_at: at(0),
            sent_in_window: 1,
            window_started_at: at(0),
        }
    }

    #[test]
    fn every_code_mail_in_every_shipped_language_carries_the_code_on_its_own_line() {
        let conn = memory_connection();
        for locale in ["en", "de"] {
            let user = account(&conn, &format!("user_{locale}"), Some(locale));
            for purpose in PURPOSES {
                let code = stored_code(user.user_id, purpose, None);
                let mail = code_mail(&conn, &code, &user, "ABCD2345").unwrap();

                let label = format!("{locale} {purpose:?}");
                let subject = format!("{}.subject", purpose.as_str());
                assert_eq!(
                    mail.subject,
                    stored_text(&conn, locale, "email", &subject),
                    "{label}"
                );
                assert_eq!(code_in(&mail), "ABCD2345", "{label}");
                assert!(mail.body.contains(&user.username), "{label}");
                assert!(mail.body.contains("15"), "{label}: the minutes");
                assert!(
                    !mail.body.contains('{'),
                    "{label}: every placeholder is filled"
                );
            }
        }
    }

    #[test]
    fn a_code_mail_goes_to_the_stored_address_in_the_default_language_without_a_setting() {
        let conn = memory_connection();
        let bob = account(&conn, "bob", None);

        let change = stored_code(
            bob.user_id,
            CodePurpose::ChangeEmail,
            Some("new@example.com"),
        );
        let mail = code_mail(&conn, &change, &bob, "ABCD2345").unwrap();
        assert_eq!(
            mail.email, "new@example.com",
            "an email change: the new address"
        );
        assert_eq!(mail.username, "bob");
        assert_eq!(
            mail.subject,
            stored_text(&conn, "en", "email", "change_email.subject")
        );

        let reset = stored_code(bob.user_id, CodePurpose::ResetPassword, None);
        let mail = code_mail(&conn, &reset, &bob, "ABCD2345").unwrap();
        assert_eq!(mail.email, "bob@example.com");
    }

    #[test]
    fn a_missing_mail_text_is_an_error_not_an_empty_mail() {
        let conn = memory_connection();
        let bob = account(&conn, "bob", None);
        conn.execute(
            "DELETE FROM translation_keys WHERE namespace = 'email' AND name = 'verify_email.body'",
            [],
        )
        .unwrap();

        let code = stored_code(bob.user_id, CodePurpose::VerifyEmail, None);
        let result = code_mail(&conn, &code, &bob, "ABCD2345");

        assert!(matches!(
            &result,
            Err(AppErrors::Translation(TranslationError::MissingText { key }))
                if key == "email.verify_email.body"
        ));
    }

    fn sent(last_sent_at: u64, sent_in_window: i64, window_started_at: u64) -> EmailCode {
        EmailCode {
            user_id: 1,
            purpose: CodePurpose::ResetPassword,
            code_hash: "hash".to_string(),
            new_email: None,
            expires_at: at(last_sent_at + 900),
            attempts: 0,
            last_sent_at: at(last_sent_at),
            sent_in_window,
            window_started_at: at(window_started_at),
        }
    }

    #[test]
    fn codes_use_only_the_unambiguous_alphabet() {
        let codes: Vec<String> = (0..200).map(|_| generate_code()).collect();
        for code in &codes {
            assert_eq!(code.len(), CODE_LENGTH);
            assert!(code.bytes().all(|c| CODE_ALPHABET.contains(&c)), "{code}");
        }
        assert!(
            codes.iter().any(|code| code != &codes[0]),
            "codes must differ"
        );
    }

    #[test]
    fn typed_codes_are_normalised() {
        assert_eq!(normalize_code(" abcd-2345 "), "ABCD2345");
        assert_eq!(normalize_code("ABCD 2345"), "ABCD2345");
    }

    #[test]
    fn the_first_mail_starts_a_window() {
        let now = at(1_700_000_000);
        assert_eq!(
            counters_for_next_mail(None, now),
            Some(MailCounters {
                sent_in_window: 1,
                window_started_at: now
            })
        );
    }

    #[test]
    fn a_mail_within_the_cooldown_is_refused() {
        let existing = sent(1_700_000_000, 1, 1_700_000_000);
        assert_eq!(
            counters_for_next_mail(Some(&existing), at(1_700_000_059)),
            None
        );
    }

    #[test]
    fn after_the_cooldown_the_window_count_goes_up() {
        let existing = sent(1_700_000_000, 2, 1_699_999_000);
        assert_eq!(
            counters_for_next_mail(Some(&existing), at(1_700_000_060)),
            Some(MailCounters {
                sent_in_window: 3,
                window_started_at: at(1_699_999_000)
            })
        );
    }

    #[test]
    fn the_daily_cap_holds_until_the_window_is_over() {
        let existing = sent(1_700_000_000, MAILS_PER_WINDOW, 1_699_990_000);
        assert_eq!(
            counters_for_next_mail(Some(&existing), at(1_700_000_600)),
            None
        );

        let a_day_later = at(1_699_990_000 + 24 * 60 * 60);
        assert_eq!(
            counters_for_next_mail(Some(&existing), a_day_later),
            Some(MailCounters {
                sent_in_window: 1,
                window_started_at: a_day_later
            })
        );
    }

    #[test]
    fn a_clock_that_went_backwards_keeps_the_cooldown() {
        let existing = sent(1_700_000_000, 1, 1_700_000_000);
        assert_eq!(
            counters_for_next_mail(Some(&existing), at(1_699_000_000)),
            None
        );
    }
}
