use std::{env, fs, sync::Arc};

use chrono::prelude::*;
use serde::{Deserialize, Serialize};

use crate::error_types::ApplicationSetUpErrors;
use crate::models::{EmailConfig, SmtpTls};
use base64::{engine::general_purpose::STANDARD, Engine};
use directories::ProjectDirs;
use rand::rngs::SysRng;
use rand::TryRng;

/// Server secrets persisted in `secrets.toml` in the OS config directory.
#[derive(Serialize, Deserialize)]
pub struct Secrets {
    token_signing_key: String,
    previous_token_signing_key: Option<String>,
    previous_token_signing_key_expiration_date: Option<DateTime<Utc>>,
}

/// Loads `secrets.toml`, generating it with a fresh 256-bit key on first run.
/// The file is written to a temp path and then renamed, so a crash mid-write
/// can't leave a half-written secrets file behind.
pub fn set_up_config_file() -> Result<Arc<Secrets>, ApplicationSetUpErrors> {
    let project_dirs = ProjectDirs::from("com", "placeholder_organization", "bookserver_backend")
        .ok_or(ApplicationSetUpErrors::NoConfigDirectory)?;
    let config_dir = project_dirs.config_dir();

    fs::create_dir_all(config_dir).map_err(|_| ApplicationSetUpErrors::DirectoryCreationFailure)?;

    let secrets_path = config_dir.join("secrets.toml");
    let secrets_tmp_path = config_dir.join("secrets.toml.tmp");

    if !secrets_path.exists() {
        let contents = create_new_set_up_secrets()?;
        let contents_string = toml::to_string_pretty(&contents)
            .map_err(|_| ApplicationSetUpErrors::SecretConversionFailure)?;
        // TODO: restrict the file to owner-only (0600) so other local users can't read it.
        fs::write(&secrets_tmp_path, contents_string)
            .map_err(|_| ApplicationSetUpErrors::DocumentWritingFailure)?;
        fs::rename(&secrets_tmp_path, &secrets_path)
            .map_err(|_| ApplicationSetUpErrors::RenamingFileFailure)?;
    }
    let read_content = fs::read_to_string(&secrets_path)
        .map_err(|_| ApplicationSetUpErrors::DocumentReadingFailure)?;
    let secret: Secrets = toml::from_str(&read_content)
        .map_err(|_| ApplicationSetUpErrors::SecretDeserializingError)?;
    Ok(Arc::new(secret))
}

fn create_new_set_up_secrets() -> Result<Secrets, ApplicationSetUpErrors> {
    let mut key_bytes = [0u8; 32];
    SysRng
        .try_fill_bytes(&mut key_bytes)
        .map_err(|_| ApplicationSetUpErrors::SysRngFailure)?;
    let signing_key = STANDARD.encode(key_bytes);
    let secret = Secrets {
        token_signing_key: signing_key,
        previous_token_signing_key: None,
        previous_token_signing_key_expiration_date: None,
    };
    Ok(secret)
}

pub fn get_port() -> u16 {
    env::var("PORT")
        .unwrap_or_else(|_| "8080".to_string())
        .parse()
        .expect("Invalid port number")
}

pub fn get_books_dirs() -> Vec<String> {
    env::var("BOOK_DIRS")
        .unwrap_or_else(|_| "../Downloads".to_string())
        .split(':')
        .map(|s| s.trim().to_string())
        .collect()
}

pub fn get_api_key() -> Option<String> {
    env::var("API_KEY").ok()
}

/// Reads the `SMTP_*` settings. `Ok(None)` means email is off because `SMTP_HOST` isn't
/// set; a half-filled configuration is an error, so a typo can't silently switch email off.
pub fn get_email_config() -> Result<Option<EmailConfig>, ApplicationSetUpErrors> {
    email_config_from(|name| env::var(name).ok())
}

/// The lookup is a parameter so tests can pass their own settings: changing the process
/// environment would leak between tests, which run on parallel threads.
fn email_config_from(
    lookup: impl Fn(&'static str) -> Option<String>,
) -> Result<Option<EmailConfig>, ApplicationSetUpErrors> {
    let setting = |name: &'static str| lookup(name).filter(|value| !value.trim().is_empty());
    let required =
        |name: &'static str| setting(name).ok_or(ApplicationSetUpErrors::MissingSetting(name));

    let Some(host) = setting("SMTP_HOST") else {
        return Ok(None);
    };
    let port = required("SMTP_PORT")?
        .parse()
        .map_err(|_| ApplicationSetUpErrors::InvalidSetting("SMTP_PORT"))?;
    let tls = match setting("SMTP_TLS").as_deref() {
        None | Some("starttls") => SmtpTls::StartTls,
        Some("implicit") => SmtpTls::Implicit,
        Some("none") => SmtpTls::Off,
        Some(_) => return Err(ApplicationSetUpErrors::InvalidSetting("SMTP_TLS")),
    };
    let credentials = match (setting("SMTP_USERNAME"), setting("SMTP_PASSWORD")) {
        (Some(username), Some(password)) => Some((username, password)),
        (None, None) => None,
        _ => return Err(ApplicationSetUpErrors::IncompleteSmtpLogin),
    };
    if tls == SmtpTls::Off && credentials.is_some() {
        return Err(ApplicationSetUpErrors::PlaintextSmtpLogin);
    }

    Ok(Some(EmailConfig {
        host,
        port,
        tls,
        credentials,
        from: required("SMTP_FROM")?,
        from_name: required("SMTP_FROM_NAME")?,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn parse(
        settings: &[(&'static str, &str)],
    ) -> Result<Option<EmailConfig>, ApplicationSetUpErrors> {
        let settings: HashMap<&str, String> = settings
            .iter()
            .map(|(name, value)| (*name, value.to_string()))
            .collect();
        email_config_from(|name| settings.get(name).cloned())
    }

    const MAILPIT: [(&str, &str); 5] = [
        ("SMTP_HOST", "localhost"),
        ("SMTP_PORT", "1025"),
        ("SMTP_TLS", "none"),
        ("SMTP_FROM", "server@example.com"),
        ("SMTP_FROM_NAME", "Book Server"),
    ];

    fn with(extra: &[(&'static str, &'static str)]) -> Vec<(&'static str, &'static str)> {
        let mut settings: Vec<_> = MAILPIT
            .iter()
            .filter(|(name, _)| !extra.iter().any(|(other, _)| other == name))
            .copied()
            .collect();
        settings.extend_from_slice(extra);
        settings
    }

    fn without(name: &str) -> Vec<(&'static str, &'static str)> {
        MAILPIT
            .iter()
            .filter(|(n, _)| *n != name)
            .copied()
            .collect()
    }

    #[test]
    fn no_smtp_host_means_email_is_off() {
        assert!(parse(&[]).unwrap().is_none());
        assert!(parse(&[("SMTP_HOST", "  ")]).unwrap().is_none());
    }

    #[test]
    fn a_mailpit_setup_needs_no_login() {
        let config = parse(&MAILPIT).unwrap().expect("email is on");
        assert_eq!(config.host, "localhost");
        assert_eq!(config.port, 1025);
        assert_eq!(config.tls, SmtpTls::Off);
        assert!(config.credentials.is_none());
        assert_eq!(config.from, "server@example.com");
        assert_eq!(config.from_name, "Book Server");
    }

    #[test]
    fn each_tls_mode_is_understood_and_starttls_is_the_default() {
        let login = [("SMTP_USERNAME", "user"), ("SMTP_PASSWORD", "secret")];
        for (value, expected) in [
            ("starttls", SmtpTls::StartTls),
            ("implicit", SmtpTls::Implicit),
        ] {
            let mut settings = with(&[("SMTP_TLS", value)]);
            settings.extend_from_slice(&login);
            let config = parse(&settings).unwrap().unwrap();
            assert_eq!(config.tls, expected, "{value}");
            assert_eq!(
                config.credentials,
                Some(("user".to_string(), "secret".to_string()))
            );
        }

        let config = parse(&without("SMTP_TLS")).unwrap().unwrap();
        assert_eq!(config.tls, SmtpTls::StartTls);
    }

    #[test]
    fn an_unknown_tls_mode_is_refused() {
        let result = parse(&with(&[("SMTP_TLS", "ssl")]));
        assert!(matches!(
            result,
            Err(ApplicationSetUpErrors::InvalidSetting("SMTP_TLS"))
        ));
    }

    #[test]
    fn a_password_is_never_sent_unencrypted() {
        let result = parse(&with(&[
            ("SMTP_USERNAME", "user"),
            ("SMTP_PASSWORD", "secret"),
        ]));
        assert!(matches!(
            result,
            Err(ApplicationSetUpErrors::PlaintextSmtpLogin)
        ));
    }

    #[test]
    fn half_a_login_is_refused() {
        let result = parse(&with(&[
            ("SMTP_TLS", "starttls"),
            ("SMTP_USERNAME", "user"),
        ]));
        assert!(matches!(
            result,
            Err(ApplicationSetUpErrors::IncompleteSmtpLogin)
        ));
    }

    #[test]
    fn a_missing_or_broken_setting_is_named() {
        for name in ["SMTP_PORT", "SMTP_FROM", "SMTP_FROM_NAME"] {
            let result = parse(&without(name));
            assert!(
                matches!(result, Err(ApplicationSetUpErrors::MissingSetting(missing)) if missing == name),
                "{name}"
            );
        }

        let result = parse(&with(&[("SMTP_PORT", "smtp")]));
        assert!(matches!(
            result,
            Err(ApplicationSetUpErrors::InvalidSetting("SMTP_PORT"))
        ));
    }
}
