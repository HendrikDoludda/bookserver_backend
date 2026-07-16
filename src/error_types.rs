use axum::{
    body::Body,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use thiserror::Error;

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
}

impl IntoResponse for DatabaseError {
    fn into_response(self) -> Response<Body> {
        let status = match self {
            DatabaseError::EntryNotFound => StatusCode::NOT_FOUND,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        };
        let body = if status.is_server_error() {
            tracing::error!("database error: {self}");
            "Internal server error".to_string()
        } else {
            self.to_string()
        };
        (status, body).into_response()
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
pub enum RequestErrors {
    #[error("Failed to authorize the user, so the request couldn't be completed")]
    AuthorizationFailed,

    #[error("Request failed to retrieve the desired data")]
    RequestFailed,
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
    #[error("Unautherized Session")]
    UnautherizedSession,
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
}
