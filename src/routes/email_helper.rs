use crate::config::get_email_config;
use crate::error_types::EmailErrors;
use mail_send::mail_builder::{self, MessageBuilder};
use mail_send::SmtpClientBuilder;

//Eventually I need to change it to use impl with a struct this way we can load the config once on opening the back end.
pub async fn create_new_email(email_information: EmailInformation) -> Result<(), EmailErrors> {
    //the call requires a subject and body as well as the receivers email address.
    //set up the email using a library.
    //sender email will always be the same.
    //send the email.
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

fn send_email() {}

pub struct EmailInformation {
    pub username: String,
    pub email: String,
    pub body: String,
    pub subject: String,
}
