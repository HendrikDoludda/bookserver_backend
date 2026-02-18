use thiserror::Error;

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