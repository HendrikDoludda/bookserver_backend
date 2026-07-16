use std::env;

use crate::models::EmailConfig;

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
