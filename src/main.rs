use anyhow::Context;
use bookserver_backend::app_state::AppState;
use bookserver_backend::config;
use bookserver_backend::database_related_scripts::db_users;
use bookserver_backend::db::Database;
use bookserver_backend::models::SmtpTls;
use bookserver_backend::routes::api_routes::start_server;
use bookserver_backend::routes::email_helper::Mailer;
use bookserver_backend::routes::registration::SetupCode;
use bookserver_backend::routes::totp_cipher::TotpCipher;
use std::sync::Arc;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let listen_address = config::get_listen_address().context("The listen settings are invalid")?;
    let db = Arc::new(Database::new().context("Failed to initialize the database")?);
    let secrets = config::set_up_config_file().context("Failed to load secrets.toml")?;
    let totp_cipher = TotpCipher::from_signing_key(&secrets.signing_key()?);

    let email = config::get_email_config().context("The SMTP_* settings are incomplete")?;
    match &email {
        None => tracing::info!("Email is off: SMTP_HOST is not set"),
        Some(email) if email.tls == SmtpTls::Off => tracing::warn!(
            "SMTP_TLS=none: mail leaves this server unencrypted; use it only with a local catcher such as Mailpit"
        ),
        Some(_) => {}
    }

    let (setup_code, setup_code_text) = SetupCode::generate();
    let has_accounts = Arc::clone(&db)
        .read(|conn| Ok(db_users::any_exists(conn)?))
        .await
        .context("Failed to check whether an account exists")?;
    if !has_accounts {
        // The one secret that is logged: the log is how it reaches whoever installed the
        // server, and it is worthless once the first account exists.
        tracing::warn!(
            "No account exists yet: the first sign-up needs the setup code {setup_code_text}"
        );
    }

    let state = AppState {
        db,
        mailer: Mailer::from_config(email),
        setup_code,
        totp_cipher,
    };

    start_server(state, listen_address)
        .await
        .context("Server stopped with an error")?;

    Ok(())
}
