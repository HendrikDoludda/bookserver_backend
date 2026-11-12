use std::{fmt::Display, time::Duration};

use axum::{
    body::Body,
    http::{header::RETRY_AFTER, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;
use thiserror::Error;

/// The body of every error answer: a stable key the app translates through the catalog.
#[derive(Serialize)]
struct ErrorBody {
    error: &'static str,
}

const INTERNAL: (StatusCode, &str) = (StatusCode::INTERNAL_SERVER_ERROR, "internal_error");

pub fn error_response(status: StatusCode, key: &'static str) -> Response<Body> {
    (status, Json(ErrorBody { error: key })).into_response()
}

/// A 500 never tells the client why; the detail goes to the server log instead.
fn respond(status_and_key: (StatusCode, &'static str), detail: &dyn Display) -> Response<Body> {
    let (status, key) = status_and_key;
    if status == StatusCode::INTERNAL_SERVER_ERROR {
        tracing::error!("request failed: {detail}");
    }
    error_response(status, key)
}

/// The answer for a failure whose error type has no `IntoResponse` (e.g. `anyhow`);
/// the caller logs the detail.
pub fn internal_error_response() -> Response<Body> {
    error_response(INTERNAL.0, INTERNAL.1)
}

#[derive(Error, Debug)]
pub enum DatabaseError {
    #[error("Could not initialize the database")]
    DatabaseInitializationFailure,

    #[error("Could not create required directory for database creation")]
    DirectoryCreationFailure,

    #[error("Could not establish a connection to the database")]
    FailedToConnectToDatabase,

    #[error("Failed creation of the database connection pool")]
    ConnectionPoolFailure,

    #[error("Failed getting a connection from the pool")]
    PoolConnectionRetrievalFailure,

    #[error("Failed inserting metadata for one item")]
    InsertionFailure,

    #[error("Failed updating an already existing entry in the database")]
    UpdatingFailure,

    #[error("Tried to delete by key from a table without a single key column")]
    NoSingleKeyColumn,

    #[error("Entry in database is not found")]
    EntryNotFound,

    #[error("Connection executable task has failed")]
    ConnectionExecutableFailure,

    #[error("connection task preparation has failed")]
    TaskPreparationFailure,

    #[error("Failed to get next row from database table")]
    NextRowFailure,

    #[error("Statement Query has failed")]
    QueryFailure,

    #[error("Retrieving Id has failed")]
    IdRetrievalFailure,

    #[error("Failed to create a transaction")]
    TransactionFailure,

    #[error("Failed to create a new operation")]
    OperationFailure,

    #[error("Failed to commit the created operation to the transaction")]
    OperationCommitFailure,

    #[error("number of paramaters did not match the number of columns")]
    InvalidParameters,
}

impl DatabaseError {
    fn status_and_key(&self) -> (StatusCode, &'static str) {
        match self {
            DatabaseError::EntryNotFound => (StatusCode::NOT_FOUND, "not_found"),
            DatabaseError::DatabaseInitializationFailure
            | DatabaseError::DirectoryCreationFailure
            | DatabaseError::FailedToConnectToDatabase
            | DatabaseError::ConnectionPoolFailure
            | DatabaseError::PoolConnectionRetrievalFailure
            | DatabaseError::InsertionFailure
            | DatabaseError::UpdatingFailure
            | DatabaseError::NoSingleKeyColumn
            | DatabaseError::ConnectionExecutableFailure
            | DatabaseError::TaskPreparationFailure
            | DatabaseError::NextRowFailure
            | DatabaseError::QueryFailure
            | DatabaseError::IdRetrievalFailure
            | DatabaseError::TransactionFailure
            | DatabaseError::OperationFailure
            | DatabaseError::OperationCommitFailure
            | DatabaseError::InvalidParameters => INTERNAL,
        }
    }
}

impl IntoResponse for DatabaseError {
    fn into_response(self) -> Response<Body> {
        respond(self.status_and_key(), &self)
    }
}

#[derive(Error, Debug)]
pub enum FolderScannerError {
    #[error("File type not recognized")]
    FileTypeNotRecognized,

    #[error("File Not Found")]
    FileNotFound,

    #[error("Metadata extraction failed")]
    MetadataExtractionFailed,

    #[error("System does not support getting modified file information")]
    UnsupportedOperatingSystem,

    #[error("File Hashing Failed")]
    FileHashingFailed,

    #[error("Could not insert book entry into the database")]
    BookInsertionFailed,

    #[error("Could not insert series entry into the database")]
    SeriesInsertionFailed,

    #[error("Directory not found")]
    MissingDirectory,

    #[error("Permit creation failed")]
    PermitCreationFailed,
}

#[derive(Error, Debug)]
pub enum CoverImageError {
    #[error("File not found")]
    FileNotFound,

    #[error("General error")]
    GeneralError,

    #[error("failed creating the cover")]
    CoverCreationFailed,

    #[error("Invalid File Name")]
    InvalidFileName,

    #[error("File Reading Failed")]
    FileCouldNotBeRead,

    #[error("Page Retrieval failed")]
    PageCouldNotBeRetrieved,

    #[error("Could Not convert the page to image")]
    PageToImageConversionFailed,

    #[error("Zip Archive tool failed in initializing")]
    ZipArchiveFailure,

    #[error("The page sorter failed")]
    SortingFailed,

    #[error("System failed to identify the type of image that has been retrieved")]
    ImageFormatFailed,

    #[error("System failed to create a document from pdf")]
    PdfToDocumentError,
}

#[derive(Error, Debug, Clone)]
pub enum StreamReaderErrors {
    #[error("File Not Found")]
    FileNotFound,

    #[error("Could not extract the string to str")]
    FileSizeConversionFailed,

    #[error("The range header that was provided is not valid")]
    InvalidRangeHeader,

    #[error("No file type found in the metadata")]
    NoFileTypeFound,

    #[error("Could not create the server response!")]
    ResponseCreationFailed,

    #[error("Could not create the body component!")]
    BodyCreationFailed,
}

#[derive(Error, Debug)]
pub enum RoutingErrors {
    #[error("Failed to start Axum")]
    AxumInitializationFailed(#[source] std::io::Error),
    #[error("Creating TcpListener failed")]
    TcpListenerCreationFailed(#[source] std::io::Error),
}

#[derive(Error, Debug)]
pub enum AuthenticationError {
    #[error("Username already taken")]
    UsernameTaken,
    #[error("Email already taken")]
    EmailTaken,
    #[error("Wrong email/username or password")]
    IncorrectCredentials,
    #[error("Expired Session and Refresh Token")]
    ExpiredSession,
    #[error("Unauthorized session")]
    UnauthorizedSession,
    #[error("Invalid validation code")]
    InvalidValidationCode,
    #[error("Searching the database produced an error")]
    DatabaseSearchFailure,
    #[error("Mismatching passwords")]
    MismatchingPasswords,
    #[error("Failed to store data properly")]
    FailedDataStoring,
    #[error("Failed to create a new randomly generated value")]
    RngCreatorFailed,
    #[error("Hashing sensitive data failed")]
    ErrorHashingData,
    #[error("Failed to calculate the lifetime for tokens")]
    TokenLifetimeCalculationFailed,
    #[error("Failed to convert string into PasswordHash type")]
    StringToPasswordHashConversionFailed,
    #[error("You do not have permission to perform this action")]
    Forbidden,
    #[error("The new password is too short")]
    PasswordTooShort,
    #[error("The new password is too long")]
    PasswordTooLong,
    #[error("Too many failed tries; wait a while before trying again")]
    TooManyAttempts { retry_after: Duration },
    #[error("The email address is not verified yet")]
    EmailNotVerified,
    #[error("The email address is not valid")]
    InvalidEmail,
    #[error("The username is not valid")]
    InvalidUsername,
    #[error("The first sign-up's setup code is missing or wrong")]
    InvalidSetupCode,
    #[error("The current password is wrong")]
    IncorrectPassword,
    #[error("The new password is the current one")]
    PasswordUnchanged,
    #[error("The new email address is the current one")]
    EmailUnchanged,
    #[error("Two-factor authentication is already on")]
    TotpAlreadyEnabled,
    #[error("There is no two-factor setup waiting to be confirmed")]
    TotpSetupNotPending,
    #[error("Two-factor authentication is not on")]
    TotpNotEnabled,
    #[error("A TOTP secret could not be made, encrypted or read")]
    TotpFailure,
    #[error("The last admin can't be made a member")]
    LastAdmin,
}

impl AuthenticationError {
    fn status_and_key(&self) -> (StatusCode, &'static str) {
        match self {
            AuthenticationError::UsernameTaken => (StatusCode::CONFLICT, "username_taken"),
            AuthenticationError::EmailTaken => (StatusCode::CONFLICT, "email_taken"),
            AuthenticationError::IncorrectCredentials => {
                (StatusCode::UNAUTHORIZED, "incorrect_credentials")
            }
            AuthenticationError::ExpiredSession | AuthenticationError::UnauthorizedSession => {
                (StatusCode::UNAUTHORIZED, "unauthorized")
            }
            AuthenticationError::InvalidValidationCode => {
                (StatusCode::BAD_REQUEST, "invalid_or_expired_code")
            }
            AuthenticationError::MismatchingPasswords => {
                (StatusCode::BAD_REQUEST, "passwords_do_not_match")
            }
            AuthenticationError::PasswordTooShort => {
                (StatusCode::BAD_REQUEST, "password_too_short")
            }
            AuthenticationError::PasswordTooLong => (StatusCode::BAD_REQUEST, "password_too_long"),
            AuthenticationError::TooManyAttempts { .. } => {
                (StatusCode::TOO_MANY_REQUESTS, "too_many_attempts")
            }
            AuthenticationError::EmailNotVerified => (StatusCode::FORBIDDEN, "email_not_verified"),
            AuthenticationError::InvalidEmail => (StatusCode::BAD_REQUEST, "invalid_email"),
            AuthenticationError::InvalidUsername => (StatusCode::BAD_REQUEST, "invalid_username"),
            AuthenticationError::InvalidSetupCode => (StatusCode::FORBIDDEN, "invalid_setup_code"),
            AuthenticationError::IncorrectPassword => {
                (StatusCode::BAD_REQUEST, "incorrect_password")
            }
            AuthenticationError::PasswordUnchanged => {
                (StatusCode::BAD_REQUEST, "password_unchanged")
            }
            AuthenticationError::EmailUnchanged => (StatusCode::BAD_REQUEST, "email_unchanged"),
            AuthenticationError::TotpAlreadyEnabled => {
                (StatusCode::CONFLICT, "totp_already_enabled")
            }
            AuthenticationError::TotpSetupNotPending => {
                (StatusCode::CONFLICT, "totp_setup_not_pending")
            }
            AuthenticationError::TotpNotEnabled => (StatusCode::CONFLICT, "totp_not_enabled"),
            AuthenticationError::LastAdmin => (StatusCode::CONFLICT, "last_admin"),
            AuthenticationError::Forbidden => (StatusCode::FORBIDDEN, "forbidden"),
            AuthenticationError::DatabaseSearchFailure
            | AuthenticationError::FailedDataStoring
            | AuthenticationError::RngCreatorFailed
            | AuthenticationError::ErrorHashingData
            | AuthenticationError::TokenLifetimeCalculationFailed
            | AuthenticationError::StringToPasswordHashConversionFailed
            | AuthenticationError::TotpFailure => INTERNAL,
        }
    }
}

#[derive(Error, Debug)]
pub enum EmailErrors {
    #[error("Email is not set up")]
    EmailSetUpNotFound,
    #[error("Could not set up the smtp connection")]
    SMTPConnectionFailed,
    #[error("Could not create a smtp client builder")]
    ClientBuilderFailed,
    #[error("Failed to send the email")]
    SendingFailure,
}

impl EmailErrors {
    fn status_and_key(&self) -> (StatusCode, &'static str) {
        match self {
            EmailErrors::EmailSetUpNotFound => {
                (StatusCode::SERVICE_UNAVAILABLE, "email_not_configured")
            }
            EmailErrors::SMTPConnectionFailed
            | EmailErrors::ClientBuilderFailed
            | EmailErrors::SendingFailure => INTERNAL,
        }
    }
}

#[derive(Error, Debug)]
pub enum SettingsError {
    #[error("The locale is not a language tag such as de or pt-BR")]
    InvalidLocale,
}

impl SettingsError {
    fn status_and_key(&self) -> (StatusCode, &'static str) {
        match self {
            SettingsError::InvalidLocale => (StatusCode::BAD_REQUEST, "invalid_locale"),
        }
    }
}

/// A shipped translation file that breaks a rule: the server doesn't start with one. `file` is
/// the path under `translations/`, `key` the dotted name inside the file.
#[derive(Debug, Error, PartialEq)]
pub enum ShippedTranslationError {
    #[error(
        "{file}: not <language>/language.json or <language>/frontend|backend/<namespace>.json"
    )]
    UnexpectedFile { file: String },
    #[error("{file}: {code} is not a language code such as de or pt-BR")]
    BadLanguageCode { file: String, code: String },
    #[error("{file}: {reason}")]
    InvalidJson { file: String, reason: String },
    #[error("{file}: the file must hold one JSON object")]
    NotAnObject { file: String },
    #[error("{file}: {key}: a value must be a text or an object of texts")]
    NotText { file: String, key: String },
    #[error("{file}: {key}: names use only a-z, 0-9 and _")]
    BadName { file: String, key: String },
    #[error("{file}: {key}: the text is empty")]
    EmptyText { file: String, key: String },
    #[error("{file}: {key}: a hint needs a text of the same name next to it")]
    HintWithoutText { file: String, key: String },
    #[error("{file}: {key}: hints belong in the English files")]
    HintOutsideSource { file: String, key: String },
    #[error("translations/{code}/language.json is missing")]
    MissingLanguageFile { code: String },
    #[error("{file}: {namespace} is in frontend/ in one file and in backend/ in another")]
    SideMismatch { file: String, namespace: String },
    #[error("{file}: {key}: the English files have no such key")]
    UnknownKey { file: String, key: String },
    #[error("{file}: {key}: the placeholders must be the English text's: {expected}")]
    PlaceholdersDiffer {
        file: String,
        key: String,
        expected: String,
    },
}

#[derive(Error, Debug)]
pub enum TranslationError {
    #[error("The server doesn't offer this language")]
    UnknownLocale,
}

impl TranslationError {
    fn status_and_key(&self) -> (StatusCode, &'static str) {
        match self {
            TranslationError::UnknownLocale => (StatusCode::NOT_FOUND, "not_found"),
        }
    }
}

#[derive(Debug, Error)]
pub enum AppErrors {
    #[error(transparent)]
    Database(#[from] DatabaseError),
    #[error(transparent)]
    Authentication(#[from] AuthenticationError),
    #[error(transparent)]
    Email(#[from] EmailErrors),
    #[error(transparent)]
    Settings(#[from] SettingsError),
    #[error(transparent)]
    Translation(#[from] TranslationError),
    #[error("The request's body, path or query could not be read")]
    InvalidRequest,
}

impl AppErrors {
    fn status_and_key(&self) -> (StatusCode, &'static str) {
        match self {
            AppErrors::Database(err) => err.status_and_key(),
            AppErrors::Authentication(err) => err.status_and_key(),
            AppErrors::Email(err) => err.status_and_key(),
            AppErrors::Settings(err) => err.status_and_key(),
            AppErrors::Translation(err) => err.status_and_key(),
            AppErrors::InvalidRequest => (StatusCode::BAD_REQUEST, "invalid_request"),
        }
    }
}

impl IntoResponse for AppErrors {
    fn into_response(self) -> Response<Body> {
        let mut response = respond(self.status_and_key(), &self);
        if let AppErrors::Authentication(AuthenticationError::TooManyAttempts { retry_after }) =
            self
        {
            response.headers_mut().insert(
                RETRY_AFTER,
                HeaderValue::from(whole_seconds_rounded_up(retry_after)),
            );
        }
        response
    }
}

/// Rounded up, so a client that waits exactly this long finds the lock open.
fn whole_seconds_rounded_up(duration: Duration) -> u64 {
    duration.as_secs() + u64::from(duration.subsec_nanos() > 0)
}

#[derive(Error, Debug)]
pub enum ApplicationSetUpErrors {
    #[error("Failed creating directory")]
    DirectoryCreationFailure,
    #[error("Failed filling bytes")]
    SysRngFailure,
    #[error("Failed to convert secrets to toml string")]
    SecretConversionFailure,
    #[error("Failed to write to document")]
    DocumentWritingFailure,
    #[error("Failed to read from document")]
    DocumentReadingFailure,
    #[error("Deserializing the secret has failed")]
    SecretDeserializingError,
    #[error("No Config folder found for this system")]
    NoConfigDirectory,
    #[error("Renaming file failed")]
    RenamingFileFailure,
    #[error("the setting {0} is missing; it is required when SMTP_HOST is set")]
    MissingSetting(&'static str),
    #[error("the setting {0} has an invalid value")]
    InvalidSetting(&'static str),
    #[error("SMTP_USERNAME and SMTP_PASSWORD must be set together")]
    IncompleteSmtpLogin,
    #[error("SMTP_TLS=none would send the SMTP password unencrypted")]
    PlaintextSmtpLogin,
    #[error("the signing key in secrets.toml is not 32 base64-encoded bytes")]
    InvalidSigningKey,
    #[error("secrets.toml could not be restricted to its owner")]
    SecretsPermissionFailure,
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;
    use crate::database_related_scripts::translation_seed;
    use serde_json::{json, Value};

    async fn answer(response: Response<Body>) -> (StatusCode, Value) {
        let status = response.status();
        let bytes = axum::body::to_bytes(response.into_body(), 1024)
            .await
            .unwrap();
        (status, serde_json::from_slice(&bytes).unwrap())
    }

    /// Every error a client can get besides a 500, with its status and key.
    fn client_error_cases() -> Vec<(AppErrors, StatusCode, &'static str)> {
        use AuthenticationError as Auth;
        vec![
            (
                Auth::IncorrectCredentials.into(),
                StatusCode::UNAUTHORIZED,
                "incorrect_credentials",
            ),
            (
                Auth::UnauthorizedSession.into(),
                StatusCode::UNAUTHORIZED,
                "unauthorized",
            ),
            (
                Auth::ExpiredSession.into(),
                StatusCode::UNAUTHORIZED,
                "unauthorized",
            ),
            (Auth::Forbidden.into(), StatusCode::FORBIDDEN, "forbidden"),
            (Auth::EmailTaken.into(), StatusCode::CONFLICT, "email_taken"),
            (
                Auth::UsernameTaken.into(),
                StatusCode::CONFLICT,
                "username_taken",
            ),
            (
                Auth::InvalidValidationCode.into(),
                StatusCode::BAD_REQUEST,
                "invalid_or_expired_code",
            ),
            (
                Auth::InvalidEmail.into(),
                StatusCode::BAD_REQUEST,
                "invalid_email",
            ),
            (
                Auth::InvalidUsername.into(),
                StatusCode::BAD_REQUEST,
                "invalid_username",
            ),
            (
                Auth::InvalidSetupCode.into(),
                StatusCode::FORBIDDEN,
                "invalid_setup_code",
            ),
            (
                Auth::IncorrectPassword.into(),
                StatusCode::BAD_REQUEST,
                "incorrect_password",
            ),
            (
                Auth::PasswordUnchanged.into(),
                StatusCode::BAD_REQUEST,
                "password_unchanged",
            ),
            (
                Auth::EmailUnchanged.into(),
                StatusCode::BAD_REQUEST,
                "email_unchanged",
            ),
            (
                Auth::TotpAlreadyEnabled.into(),
                StatusCode::CONFLICT,
                "totp_already_enabled",
            ),
            (
                Auth::TotpSetupNotPending.into(),
                StatusCode::CONFLICT,
                "totp_setup_not_pending",
            ),
            (
                Auth::TotpNotEnabled.into(),
                StatusCode::CONFLICT,
                "totp_not_enabled",
            ),
            (Auth::LastAdmin.into(), StatusCode::CONFLICT, "last_admin"),
            (
                Auth::MismatchingPasswords.into(),
                StatusCode::BAD_REQUEST,
                "passwords_do_not_match",
            ),
            (
                Auth::PasswordTooShort.into(),
                StatusCode::BAD_REQUEST,
                "password_too_short",
            ),
            (
                Auth::PasswordTooLong.into(),
                StatusCode::BAD_REQUEST,
                "password_too_long",
            ),
            (
                Auth::TooManyAttempts {
                    retry_after: Duration::from_secs(60),
                }
                .into(),
                StatusCode::TOO_MANY_REQUESTS,
                "too_many_attempts",
            ),
            (
                Auth::EmailNotVerified.into(),
                StatusCode::FORBIDDEN,
                "email_not_verified",
            ),
            (
                EmailErrors::EmailSetUpNotFound.into(),
                StatusCode::SERVICE_UNAVAILABLE,
                "email_not_configured",
            ),
            (
                SettingsError::InvalidLocale.into(),
                StatusCode::BAD_REQUEST,
                "invalid_locale",
            ),
            (
                DatabaseError::EntryNotFound.into(),
                StatusCode::NOT_FOUND,
                "not_found",
            ),
            (
                TranslationError::UnknownLocale.into(),
                StatusCode::NOT_FOUND,
                "not_found",
            ),
            (
                AppErrors::InvalidRequest,
                StatusCode::BAD_REQUEST,
                "invalid_request",
            ),
        ]
    }

    #[tokio::test]
    async fn client_errors_answer_with_their_own_status_and_key() {
        for (error, expected_status, expected_key) in client_error_cases() {
            let label = error.to_string();
            let (status, body) = answer(error.into_response()).await;
            assert_eq!(status, expected_status, "{label}");
            assert_eq!(body, json!({ "error": expected_key }), "{label}");
        }
    }

    #[test]
    fn the_english_error_texts_cover_exactly_the_keys_the_api_answers() {
        let mut answered: BTreeSet<&str> = client_error_cases()
            .iter()
            .map(|(_, _, key)| *key)
            .collect();
        // Answered outside the error enums: every 500, and the router's own answers (`helpers.rs`).
        answered.extend([
            "internal_error",
            "not_found",
            "method_not_allowed",
            "not_implemented",
        ]);

        let catalog = translation_seed::shipped().unwrap();
        let texts: BTreeSet<&str> = catalog
            .keys
            .keys()
            .filter(|key| key.namespace == "errors")
            .map(|key| key.name.as_str())
            .collect();
        assert_eq!(texts, answered);
    }

    #[tokio::test]
    async fn internal_failures_answer_500_without_their_detail() {
        use AuthenticationError as Auth;
        let cases: Vec<AppErrors> = vec![
            Auth::DatabaseSearchFailure.into(),
            Auth::FailedDataStoring.into(),
            Auth::RngCreatorFailed.into(),
            Auth::ErrorHashingData.into(),
            Auth::TokenLifetimeCalculationFailed.into(),
            Auth::StringToPasswordHashConversionFailed.into(),
            Auth::TotpFailure.into(),
            EmailErrors::SMTPConnectionFailed.into(),
            EmailErrors::ClientBuilderFailed.into(),
            EmailErrors::SendingFailure.into(),
            DatabaseError::DatabaseInitializationFailure.into(),
            DatabaseError::DirectoryCreationFailure.into(),
            DatabaseError::FailedToConnectToDatabase.into(),
            DatabaseError::ConnectionPoolFailure.into(),
            DatabaseError::PoolConnectionRetrievalFailure.into(),
            DatabaseError::InsertionFailure.into(),
            DatabaseError::UpdatingFailure.into(),
            DatabaseError::NoSingleKeyColumn.into(),
            DatabaseError::ConnectionExecutableFailure.into(),
            DatabaseError::TaskPreparationFailure.into(),
            DatabaseError::NextRowFailure.into(),
            DatabaseError::QueryFailure.into(),
            DatabaseError::IdRetrievalFailure.into(),
            DatabaseError::TransactionFailure.into(),
            DatabaseError::OperationFailure.into(),
            DatabaseError::OperationCommitFailure.into(),
            DatabaseError::InvalidParameters.into(),
        ];

        for error in cases {
            let label = error.to_string();
            let (status, body) = answer(error.into_response()).await;
            assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR, "{label}");
            assert_eq!(body, json!({ "error": "internal_error" }), "{label}");
        }
    }

    #[test]
    fn a_429_says_when_to_try_again_in_whole_seconds_rounded_up() {
        for (retry_after, expected) in [
            (Duration::from_secs(90), "90"),
            (Duration::from_millis(1500), "2"),
            (Duration::from_millis(1), "1"),
        ] {
            let error: AppErrors = AuthenticationError::TooManyAttempts { retry_after }.into();
            let response = error.into_response();
            assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
            assert_eq!(response.headers()[RETRY_AFTER], expected, "{retry_after:?}");
        }

        let other: AppErrors = AuthenticationError::IncorrectCredentials.into();
        assert!(
            !other.into_response().headers().contains_key(RETRY_AFTER),
            "only a 429 says when to retry"
        );
    }

    #[tokio::test]
    async fn a_database_error_answers_in_the_same_shape_on_its_own() {
        let (status, body) = answer(DatabaseError::EntryNotFound.into_response()).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body, json!({ "error": "not_found" }));

        let (status, body) = answer(DatabaseError::QueryFailure.into_response()).await;
        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(body, json!({ "error": "internal_error" }));
    }

    #[tokio::test]
    async fn the_generic_internal_answer_is_json_too() {
        let (status, body) = answer(internal_error_response()).await;
        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(body, json!({ "error": "internal_error" }));
    }
}
