use crate::config::get_email_config;
use crate::error_types::EmailErrors;
use mail_send::mail_builder::MessageBuilder;
use mail_send::SmtpClientBuilder;

/// Builds a plain-text email and sends it over SMTP using the `SMTP_*` env config.
/// Returns `EmailErrors::EmailSetUpNotFound` when SMTP isn't configured.
// TODO: move this onto a struct so the SMTP config is loaded once at startup
// instead of on every send.
pub async fn create_new_email(email_information: EmailInformation) -> Result<(), EmailErrors> {
    let sender = get_email_config().ok_or(EmailErrors::EmailSetUpNotFound)?;
    let email = MessageBuilder::new()
        .from((sender.from_name, sender.from))
        .to(vec![email_information.username, email_information.email])
        .subject(email_information.subject)
        .text_body(email_information.body);

    SmtpClientBuilder::new(sender.host, sender.port)
        .map_err(|e| {
            log::error!("Failed to create the smtp client: {e}");
            EmailErrors::ClientBuilderFailed
        })?
        .implicit_tls(false)
        .credentials((sender.username, sender.password))
        .connect()
        .await
        .map_err(|e| {
            log::error!("Failed to connect the smtp server: {e}");
            EmailErrors::SMTPConnectionFailed
        })?
        .send(email)
        .await
        .map_err(|e| {
            log::error!("Failed to send the email: {e}");
            EmailErrors::SendingFailure
        })?;
    Ok(())
}

pub struct EmailInformation {
    pub username: String,
    pub email: String,
    pub body: String,
    pub subject: String,
}
