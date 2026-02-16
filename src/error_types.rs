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
}