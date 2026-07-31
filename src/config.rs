use std::{env, fs, path::Path, sync::Arc};

use chrono::prelude::*;
use mail_send::mail_auth::hickory_resolver::proto::dnssec::rdata::key;
use serde::{Deserialize, Serialize};

use crate::error_types::ApplicationSetUpErrors;
use crate::models::EmailConfig;
use argon2::password_hash::SaltString;
use base64::{engine::general_purpose::STANDARD, Engine};
use directories::ProjectDirs;
use rand::rngs::SysRng;
use rand::TryRng;

#[derive(Serialize, Deserialize)]
struct Secrets {
    token_signing_key: String,
    previous_token_signing_key: Option<String>,
    previous_token_signing_key_expiration_date: Option<DateTime<Utc>>,
}

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
        //For the future the file should be marked as restricted so only the author is allowed to open the file not other users.
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
        previous_token_signing_key_expiration_date: None, //Utc::now()
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
        .split(':') // Assuming ':' as separator; adjust if needed
        .map(|s| s.trim().to_string())
        .collect()
}

pub fn get_api_key() -> Option<String> {
    env::var("API_KEY").ok()
}

pub fn get_email_config() -> Option<EmailConfig> {
    Some(EmailConfig {
        host: env::var("SMTP_HOST").ok()?,
        port: env::var("SMTP_PORT").ok()?.parse().ok()?,
        username: env::var("SMTP_USERNAME").ok()?,
        password: env::var("SMTP_PASSWORD").ok()?,
        from: env::var("SMTP_FROM").ok()?,
        from_name: env::var("SMTP_FROM_NAME").ok()?,
    })
}
