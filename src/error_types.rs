use std::fmt::Display;

use axum::{
    body::Body,
    http::StatusCode,
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

    #[error("Tried to delete an item from the library-series table, which is not allowed")]
    LibrarySeriesException,

    #[error("Entry in database is not found")]
    EntryNotFound,

    #[error("Connection executable task has failed")]
    ConnectionExecutableFailure,

    #[error("connection task preparation has failed")]
    TaskPreparationFailure,

    #[error("Failed to get next row from database table")]
    NextRowFailure,

    #[error("Invalid combination of Database type and column")]
    InvalidRequest,

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
    #[error("The search result does not match the expected result")]
    UnexpectedSearchResult,
    #[error("Email already exists in users")]
    EmailAlreadyExists,
    #[error("Username already exists in users")]
    UsernameAlreadyExists,
}

impl DatabaseError {
    fn status_and_key(&self) -> (StatusCode, &'static str) {
        match self {
            DatabaseError::EntryNotFound => (StatusCode::NOT_FOUND, "not_found"),
            DatabaseError::EmailAlreadyExists => (StatusCode::CONFLICT, "email_taken"),
            DatabaseError::UsernameAlreadyExists => (StatusCode::CONFLICT, "username_taken"),
            DatabaseError::DatabaseInitializationFailure
            | DatabaseError::DirectoryCreationFailure
            | DatabaseError::FailedToConnectToDatabase
            | DatabaseError::ConnectionPoolFailure
            | DatabaseError::PoolConnectionRetrievalFailure
            | DatabaseError::InsertionFailure
            | DatabaseError::UpdatingFailure
            | DatabaseError::LibrarySeriesException
            | DatabaseError::ConnectionExecutableFailure
            | DatabaseError::TaskPreparationFailure
            | DatabaseError::NextRowFailure
            | DatabaseError::InvalidRequest
            | DatabaseError::QueryFailure
            | DatabaseError::IdRetrievalFailure
            | DatabaseError::TransactionFailure
            | DatabaseError::OperationFailure
            | DatabaseError::OperationCommitFailure
            | DatabaseError::InvalidParameters
            | DatabaseError::UnexpectedSearchResult => INTERNAL,
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

    #[error("Could not create new database connection")]
    DatabaseConnectionFailed,

    #[error("Failed to regex the book title properly")]
    RegexFailure,
}

#[derive(Error, Debug)]
pub enum CoverImageError {
    #[error("File not found")]
    FileNotFound,

    #[error("Database engine failed")]
    DatabaseEngineFailed,

    #[error("Network error")]
    NetworkError,

    #[error("General error")]
    GeneralError,

    #[error("failed creating the cover")]
    CoverCreationFailed,

    #[error("Invalid File Name")]
    InvalidFileName,

    #[error("File Already Exists")]
    FileAlreadyExists,

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
    #[error("Either username or email address is already taken")]
    AccountAlreadyExists,
    #[error("Wrong email/username or password")]
    IncorrectCredentials,
    #[error("Expired Session and Refresh Token")]
    ExpiredSession,
    #[error("Unauthorized session")]
    UnauthorizedSession,
    #[error("Expired authentication code")]
    ExpiredAuthenticationCode,
    #[error("Invalid validation code")]
    InvalidValidationCode,
    #[error("Searching the database produced an error")]
    DatabaseSearchFailure,
    #[error("Mismatching passwords")]
    MismatchingPasswords,
    #[error("Failed to hash the password")]
    PasswordHashingFailure,
    #[error("Failed to store data properly")]
    FailedDataStoring,
    #[error("Failed to create a new randomly generated value")]
    RngCreatorFailed,
    #[error("Hashing sensitive data failed")]
    ErrorHashingData,
    #[error("Failed to create the salt")]
    ErrorCreatingSalt,
    #[error("Failed to calculate the lifetime for tokens")]
    TokenLifetimeCalculationFailed,
    #[error("User was not found")]
    UserNotFound,
    #[error("Email verification code for user was not found")]
    EmailVerificationNotFound,
    #[error("Failed to convert string into PasswordHash type")]
    StringToPasswordHashConversionFailed,
    #[error("Failed to update a database entry")]
    DatabaseUpdateFailed,
    #[error("Failed to insert a database entry")]
    DatabaseInsertFailed,
    #[error("You do not have permission to perform this action")]
    Forbidden,
}

impl AuthenticationError {
    fn status_and_key(&self) -> (StatusCode, &'static str) {
        match self {
            AuthenticationError::UsernameTaken => (StatusCode::CONFLICT, "username_taken"),
            AuthenticationError::EmailTaken => (StatusCode::CONFLICT, "email_taken"),
            AuthenticationError::AccountAlreadyExists => (StatusCode::CONFLICT, "account_exists"),
            AuthenticationError::IncorrectCredentials => {
                (StatusCode::UNAUTHORIZED, "incorrect_credentials")
            }
            AuthenticationError::ExpiredSession | AuthenticationError::UnauthorizedSession => {
                (StatusCode::UNAUTHORIZED, "unauthorized")
            }
            AuthenticationError::ExpiredAuthenticationCode
            | AuthenticationError::InvalidValidationCode
            | AuthenticationError::EmailVerificationNotFound => {
                (StatusCode::BAD_REQUEST, "invalid_or_expired_code")
            }
            AuthenticationError::MismatchingPasswords => {
                (StatusCode::BAD_REQUEST, "passwords_do_not_match")
            }
            AuthenticationError::UserNotFound => (StatusCode::NOT_FOUND, "not_found"),
            AuthenticationError::Forbidden => (StatusCode::FORBIDDEN, "forbidden"),
            AuthenticationError::DatabaseSearchFailure
            | AuthenticationError::PasswordHashingFailure
            | AuthenticationError::FailedDataStoring
            | AuthenticationError::RngCreatorFailed
            | AuthenticationError::ErrorHashingData
            | AuthenticationError::ErrorCreatingSalt
            | AuthenticationError::TokenLifetimeCalculationFailed
            | AuthenticationError::StringToPasswordHashConversionFailed
            | AuthenticationError::DatabaseUpdateFailed
            | AuthenticationError::DatabaseInsertFailed => INTERNAL,
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
    #[error("Failed to adjust the current time.")]
    TimeAdjustmentFailure,
    #[error("Failed to insert entry into database")]
    DatabaseInsertionFailed,
    #[error("No email was attached to the user")]
    MissingEmail,
    #[error("Failed to send the email due to no config information")]
    SentFailedDueToNoConfig { code: String },
    #[error("Failed to create a hash from code")]
    HashingFailed,
    #[error("Failed to remove an existing Verification Token")]
    VerificationTokenRemovalFailure,

    #[error("Failed to retrieve the users entry from the database")]
    FailedToRetrieveUserEntry,
    #[error("Failed to find the user entry")]
    UserEntryNotFound,
}

impl EmailErrors {
    fn status_and_key(&self) -> (StatusCode, &'static str) {
        match self {
            EmailErrors::EmailSetUpNotFound => {
                (StatusCode::SERVICE_UNAVAILABLE, "email_not_configured")
            }
            EmailErrors::SMTPConnectionFailed
            | EmailErrors::ClientBuilderFailed
            | EmailErrors::SendingFailure
            | EmailErrors::TimeAdjustmentFailure
            | EmailErrors::DatabaseInsertionFailed
            | EmailErrors::MissingEmail
            | EmailErrors::SentFailedDueToNoConfig { .. }
            | EmailErrors::HashingFailed
            | EmailErrors::VerificationTokenRemovalFailure
            | EmailErrors::FailedToRetrieveUserEntry
            | EmailErrors::UserEntryNotFound => INTERNAL,
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
}

impl AppErrors {
    fn status_and_key(&self) -> (StatusCode, &'static str) {
        match self {
            AppErrors::Database(err) => err.status_and_key(),
            AppErrors::Authentication(err) => err.status_and_key(),
            AppErrors::Email(err) => err.status_and_key(),
        }
    }
}

impl IntoResponse for AppErrors {
    fn into_response(self) -> Response<Body> {
        respond(self.status_and_key(), &self)
    }
}

#[derive(Error, Debug)]
pub enum ApplicationSetUpErrors {
    #[error("Failed creating directory")]
    DirectoryCreationFailure,
    #[error("Failed filling bytes")]
    SysRngFailure,
    #[error("Failed encoding key")]
    KeyEncodingFailure,
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, Value};

    async fn answer(response: Response<Body>) -> (StatusCode, Value) {
        let status = response.status();
        let bytes = axum::body::to_bytes(response.into_body(), 1024)
            .await
            .unwrap();
        (status, serde_json::from_slice(&bytes).unwrap())
    }

    #[tokio::test]
    async fn client_errors_answer_with_their_own_status_and_key() {
        use AuthenticationError as Auth;
        let cases: Vec<(AppErrors, StatusCode, &str)> = vec![
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
                Auth::AccountAlreadyExists.into(),
                StatusCode::CONFLICT,
                "account_exists",
            ),
            (
                Auth::ExpiredAuthenticationCode.into(),
                StatusCode::BAD_REQUEST,
                "invalid_or_expired_code",
            ),
            (
                Auth::InvalidValidationCode.into(),
                StatusCode::BAD_REQUEST,
                "invalid_or_expired_code",
            ),
            (
                Auth::EmailVerificationNotFound.into(),
                StatusCode::BAD_REQUEST,
                "invalid_or_expired_code",
            ),
            (
                Auth::MismatchingPasswords.into(),
                StatusCode::BAD_REQUEST,
                "passwords_do_not_match",
            ),
            (
                Auth::UserNotFound.into(),
                StatusCode::NOT_FOUND,
                "not_found",
            ),
            (
                EmailErrors::EmailSetUpNotFound.into(),
                StatusCode::SERVICE_UNAVAILABLE,
                "email_not_configured",
            ),
            (
                DatabaseError::EntryNotFound.into(),
                StatusCode::NOT_FOUND,
                "not_found",
            ),
            (
                DatabaseError::EmailAlreadyExists.into(),
                StatusCode::CONFLICT,
                "email_taken",
            ),
            (
                DatabaseError::UsernameAlreadyExists.into(),
                StatusCode::CONFLICT,
                "username_taken",
            ),
        ];

        for (error, expected_status, expected_key) in cases {
            let label = error.to_string();
            let (status, body) = answer(error.into_response()).await;
            assert_eq!(status, expected_status, "{label}");
            assert_eq!(body, json!({ "error": expected_key }), "{label}");
        }
    }

    #[tokio::test]
    async fn internal_failures_answer_500_without_their_detail() {
        use AuthenticationError as Auth;
        let cases: Vec<AppErrors> = vec![
            Auth::DatabaseSearchFailure.into(),
            Auth::PasswordHashingFailure.into(),
            Auth::FailedDataStoring.into(),
            Auth::RngCreatorFailed.into(),
            Auth::ErrorHashingData.into(),
            Auth::ErrorCreatingSalt.into(),
            Auth::TokenLifetimeCalculationFailed.into(),
            Auth::StringToPasswordHashConversionFailed.into(),
            Auth::DatabaseUpdateFailed.into(),
            Auth::DatabaseInsertFailed.into(),
            EmailErrors::SMTPConnectionFailed.into(),
            EmailErrors::ClientBuilderFailed.into(),
            EmailErrors::SendingFailure.into(),
            EmailErrors::TimeAdjustmentFailure.into(),
            EmailErrors::DatabaseInsertionFailed.into(),
            EmailErrors::MissingEmail.into(),
            EmailErrors::SentFailedDueToNoConfig {
                code: "SECRET-CODE".to_string(),
            }
            .into(),
            EmailErrors::HashingFailed.into(),
            EmailErrors::VerificationTokenRemovalFailure.into(),
            EmailErrors::FailedToRetrieveUserEntry.into(),
            EmailErrors::UserEntryNotFound.into(),
            DatabaseError::DatabaseInitializationFailure.into(),
            DatabaseError::DirectoryCreationFailure.into(),
            DatabaseError::FailedToConnectToDatabase.into(),
            DatabaseError::ConnectionPoolFailure.into(),
            DatabaseError::PoolConnectionRetrievalFailure.into(),
            DatabaseError::InsertionFailure.into(),
            DatabaseError::UpdatingFailure.into(),
            DatabaseError::LibrarySeriesException.into(),
            DatabaseError::ConnectionExecutableFailure.into(),
            DatabaseError::TaskPreparationFailure.into(),
            DatabaseError::NextRowFailure.into(),
            DatabaseError::InvalidRequest.into(),
            DatabaseError::QueryFailure.into(),
            DatabaseError::IdRetrievalFailure.into(),
            DatabaseError::TransactionFailure.into(),
            DatabaseError::OperationFailure.into(),
            DatabaseError::OperationCommitFailure.into(),
            DatabaseError::InvalidParameters.into(),
            DatabaseError::UnexpectedSearchResult.into(),
        ];

        for error in cases {
            let label = error.to_string();
            let (status, body) = answer(error.into_response()).await;
            assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR, "{label}");
            assert_eq!(body, json!({ "error": "internal_error" }), "{label}");
        }
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
