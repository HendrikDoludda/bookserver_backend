use std::{sync::Arc, time::Duration};

#[cfg(test)]
use std::sync::Mutex;

use mail_send::{mail_builder::MessageBuilder, SmtpClientBuilder};

use crate::error_types::EmailErrors;
use crate::models::{EmailConfig, SmtpTls};

// An unresponsive SMTP host would otherwise hold the request for `SmtpClientBuilder`'s
// default timeout of an hour.
const SEND_TIMEOUT: Duration = Duration::from_secs(30);

/// One plain-text email. No `Debug`: bodies carry codes.
pub struct EmailInformation {
    pub username: String,
    pub email: String,
    pub body: String,
    pub subject: String,
}

/// Where outgoing mail goes. An enum rather than a trait object, because an `async fn`
/// in a trait can't be called through `dyn`.
#[derive(Clone)]
pub enum Mailer {
    Smtp(Arc<EmailConfig>),
    Disabled,
    /// Records the mail instead of sending it, so tests can read the codes it carries.
    #[cfg(test)]
    Capture(Arc<Mutex<Vec<EmailInformation>>>),
}

impl Mailer {
    pub fn from_config(config: Option<EmailConfig>) -> Self {
        config.map_or(Mailer::Disabled, |config| Mailer::Smtp(Arc::new(config)))
    }

    pub fn is_enabled(&self) -> bool {
        !matches!(self, Mailer::Disabled)
    }

    /// Returns `EmailErrors::EmailSetUpNotFound` when email is off.
    pub async fn send(&self, mail: EmailInformation) -> Result<(), EmailErrors> {
        match self {
            Mailer::Smtp(config) => send_over_smtp(config, mail).await,
            Mailer::Disabled => Err(EmailErrors::EmailSetUpNotFound),
            #[cfg(test)]
            Mailer::Capture(sent) => {
                sent.lock().unwrap().push(mail);
                Ok(())
            }
        }
    }

    /// A capturing mailer and a handle to read what it recorded.
    #[cfg(test)]
    pub fn capture() -> (Self, Arc<Mutex<Vec<EmailInformation>>>) {
        let sent = Arc::new(Mutex::new(Vec::new()));
        (Mailer::Capture(Arc::clone(&sent)), sent)
    }
}

fn build_message(config: &EmailConfig, mail: EmailInformation) -> MessageBuilder<'static> {
    MessageBuilder::new()
        .from((config.from_name.clone(), config.from.clone()))
        .to((mail.username, mail.email))
        .subject(mail.subject)
        .text_body(mail.body)
}

async fn send_over_smtp(config: &EmailConfig, mail: EmailInformation) -> Result<(), EmailErrors> {
    let mut builder = SmtpClientBuilder::new(config.host.clone(), config.port)
        .map_err(|e| {
            tracing::error!("Failed to create the smtp client: {e}");
            EmailErrors::ClientBuilderFailed
        })?
        .implicit_tls(config.tls == SmtpTls::Implicit)
        .timeout(SEND_TIMEOUT);
    if let Some((username, password)) = &config.credentials {
        builder = builder.credentials((username.clone(), password.clone()));
    }
    let message = build_message(config, mail);

    // The plain and the TLS connection are different types, so each arm sends on its own.
    let sent = match config.tls {
        SmtpTls::Off => {
            builder
                .connect_plain()
                .await
                .map_err(connection_failed)?
                .send(message)
                .await
        }
        SmtpTls::StartTls | SmtpTls::Implicit => {
            builder
                .connect()
                .await
                .map_err(connection_failed)?
                .send(message)
                .await
        }
    };
    sent.map_err(|e| {
        tracing::error!("Failed to send the email: {e}");
        EmailErrors::SendingFailure
    })
}

fn connection_failed(e: mail_send::Error) -> EmailErrors {
    tracing::error!("Failed to connect the smtp server: {e}");
    EmailErrors::SMTPConnectionFailed
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config(port: u16) -> EmailConfig {
        EmailConfig {
            host: "127.0.0.1".to_string(),
            port,
            tls: SmtpTls::Off,
            credentials: None,
            from: "server@example.com".to_string(),
            from_name: "Book Server".to_string(),
        }
    }

    fn mail() -> EmailInformation {
        EmailInformation {
            username: "Alice".to_string(),
            email: "alice@example.com".to_string(),
            body: "Code: ABCD2345".to_string(),
            subject: "Password Reset".to_string(),
        }
    }

    #[test]
    fn a_message_goes_to_exactly_one_named_recipient() {
        let rendered = build_message(&config(25), mail())
            .write_to_string()
            .unwrap();

        let to: Vec<&str> = rendered
            .lines()
            .filter(|line| line.starts_with("To:"))
            .collect();
        assert_eq!(to, vec!["To: \"Alice\" <alice@example.com>"], "{rendered}");
        assert!(
            rendered.contains("From: \"Book Server\" <server@example.com>"),
            "{rendered}"
        );
    }

    #[tokio::test]
    async fn a_disabled_mailer_reports_email_as_off() {
        let result = Mailer::Disabled.send(mail()).await;
        assert!(matches!(result, Err(EmailErrors::EmailSetUpNotFound)));
        assert!(!Mailer::Disabled.is_enabled());
    }

    #[tokio::test]
    async fn a_capturing_mailer_records_what_it_was_given() {
        let (mailer, sent) = Mailer::capture();
        mailer.send(mail()).await.unwrap();

        let sent = sent.lock().unwrap();
        assert_eq!(sent.len(), 1);
        assert_eq!(sent[0].email, "alice@example.com");
        assert_eq!(sent[0].body, "Code: ABCD2345");
    }

    #[tokio::test]
    async fn an_unreachable_smtp_server_is_a_connection_failure() {
        // Bind and release a port, so nothing is listening on it.
        let port = std::net::TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        let mailer = Mailer::from_config(Some(config(port)));
        assert!(mailer.is_enabled());

        let result = mailer.send(mail()).await;
        assert!(matches!(result, Err(EmailErrors::SMTPConnectionFailed)));
    }
}
