//! Codes sent by email: making them, reading them back as typed, and the limits on how
//! often one may be mailed. Password reset uses them; email verification will too.

use std::time::{Duration, SystemTime};

use rand::RngExt;

use crate::models::EmailCode;

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

/// The counters for one more mail now, or `None` while the limits forbid it: the last mail
/// went out less than a minute ago, or the day's mails are used up.
pub fn counters_for_next_mail(
    existing: Option<&EmailCode>,
    now: SystemTime,
) -> Option<MailCounters> {
    let Some(code) = existing else {
        return Some(MailCounters {
            sent_in_window: 1,
            window_started_at: now,
        });
    };
    if elapsed(code.last_sent_at, now) < RESEND_COOLDOWN {
        return None;
    }
    if elapsed(code.window_started_at, now) >= MAIL_WINDOW {
        return Some(MailCounters {
            sent_in_window: 1,
            window_started_at: now,
        });
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
fn elapsed(since: SystemTime, now: SystemTime) -> Duration {
    now.duration_since(since).unwrap_or(Duration::ZERO)
}

#[cfg(test)]
mod tests {
    use std::time::UNIX_EPOCH;

    use super::*;
    use crate::models::CodePurpose;

    fn at(secs: u64) -> SystemTime {
        UNIX_EPOCH + Duration::from_secs(secs)
    }

    fn sent(last_sent_at: u64, sent_in_window: i64, window_started_at: u64) -> EmailCode {
        EmailCode {
            user_id: 1,
            purpose: CodePurpose::ResetPassword,
            code_hash: "hash".to_string(),
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
