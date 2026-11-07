use anyhow::Context;
use bookserver_backend::app_state::AppState;
use bookserver_backend::config;
use bookserver_backend::db::Database;
use bookserver_backend::models::SmtpTls;
use bookserver_backend::routes::api_routes::start_server;
use bookserver_backend::routes::email_helper::Mailer;
use std::sync::Arc;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let db = Arc::new(Database::new().context("Failed to initialize the database")?);
    // Not consumed yet; see docs/features/12-security-hardening.md.
    let _secrets = config::set_up_config_file()?;

    let email = config::get_email_config().context("The SMTP_* settings are incomplete")?;
    match &email {
        None => tracing::info!("Email is off: SMTP_HOST is not set"),
        Some(email) if email.tls == SmtpTls::Off => tracing::warn!(
            "SMTP_TLS=none: mail leaves this server unencrypted; use it only with a local catcher such as Mailpit"
        ),
        Some(_) => {}
    }
    let state = AppState {
        db,
        mailer: Mailer::from_config(email),
    };

    start_server(state)
        .await
        .context("Server stopped with an error")?;

    Ok(())
}
