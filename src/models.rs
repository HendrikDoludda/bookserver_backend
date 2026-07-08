use serde::{Deserialize, Serialize};
use std::{path::PathBuf, time::SystemTime};
use strum_macros::EnumIter;

//============  Row id + metadata wrapper =================
// Metadata structs don't store their own row id, but list endpoints need it so
// the client can drill down (e.g. fetch a library's series). This pairs the id
// with the metadata and flattens both into a single JSON object.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WithId<T: Serialize> {
    pub id: i64,
    #[serde(flatten)]
    pub data: T,
}

// ===========Database models =================
#[derive(Debug, Clone, Serialize, EnumIter, PartialEq, Deserialize)]
pub enum DatabaseTypes {
    Books,
    Library,
    LibraryElements,
    Series,
    Users,
}

impl DatabaseTypes {
    pub fn get_table_name(&self) -> &str {
        match self {
            DatabaseTypes::Books => "books",
            DatabaseTypes::Library => "library",
            DatabaseTypes::Series => "series",
            DatabaseTypes::Users => "users",
            DatabaseTypes::LibraryElements => "library_elements",
        }
    }

    pub fn get_file_path(&self) -> PathBuf {
        let base_path = std::path::Path::new("./data/databases/");
        if !base_path.exists() {
            std::fs::create_dir_all(base_path).unwrap();
        }
        base_path.join("app_data.sqlite")
    }
}

//============  Library model =================
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LibraryType {
    Books,
    Comics,
    Magazines,
    Documents,
    Others,
}

impl LibraryType {
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "books" => LibraryType::Books,
            "comics" => LibraryType::Comics,
            "magazines" => LibraryType::Magazines,
            "documents" => LibraryType::Documents,
            _ => LibraryType::Others,
        }
    }
    pub fn as_str(&self) -> &str {
        match self {
            LibraryType::Books => "books",
            LibraryType::Comics => "comics",
            LibraryType::Magazines => "magazines",
            LibraryType::Documents => "documents",
            LibraryType::Others => "others",
        }
    }
}

//============  Book format model =================
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BookFormat {
    Pdf,
    Epub,
    Cbz,
    ImageComic,
    None,
}

impl BookFormat {
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "pdf" => BookFormat::Pdf,
            "epub" => BookFormat::Epub,
            "cbz" => BookFormat::Cbz,
            "image_comic" => BookFormat::ImageComic,
            _ => BookFormat::None,
        }
    }
    pub fn as_str(&self) -> &str {
        match self {
            BookFormat::Pdf => "pdf",
            BookFormat::Epub => "epub",
            BookFormat::Cbz => "cbz",
            BookFormat::ImageComic => "image_comic",
            BookFormat::None => "none",
        }
    }
}

//============  Book language model =================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BookLanguage {
    English,
    German,
    French,
    Spanish,
    Portuguese,
    Italian,
    Russian,
    Chinese,
    Japanese,
    // Catch-all for any other language
    Other(String),
}

impl BookLanguage {
    pub fn from_code(code: &str) -> Self {
        match code.to_lowercase().as_str() {
            "en" => BookLanguage::English,
            "de" => BookLanguage::German,
            "fr" => BookLanguage::French,
            "es" => BookLanguage::Spanish,
            "pt" => BookLanguage::Portuguese,
            "it" => BookLanguage::Italian,
            "ru" => BookLanguage::Russian,
            "zh" => BookLanguage::Chinese,
            "ja" => BookLanguage::Japanese,
            other => BookLanguage::Other(other.to_string()),
        }
    }

    pub fn to_code(&self) -> String {
        match self {
            BookLanguage::English => "en".to_string(),
            BookLanguage::German => "de".to_string(),
            BookLanguage::French => "fr".to_string(),
            BookLanguage::Spanish => "es".to_string(),
            BookLanguage::Portuguese => "pt".to_string(),
            BookLanguage::Italian => "it".to_string(),
            BookLanguage::Russian => "ru".to_string(),
            BookLanguage::Chinese => "zh".to_string(),
            BookLanguage::Japanese => "ja".to_string(),
            BookLanguage::Other(lang) => lang.clone(),
        }
    }
}

//============  Book metadata model =================
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BookMetadata {
    pub title: String,
    pub author: String,
    pub cover_image: Option<String>,
    pub format: BookFormat,
    pub tags: Vec<String>,
    pub description: Option<String>,
    pub file_path: String,
    pub language: BookLanguage,
    pub page_count: Option<i32>,
    pub series: Option<i64>,
    pub volume_number: Option<i64>,
    pub chapter_number: Option<i64>,
    pub page_number: Option<i64>,
    pub file_hash: Option<String>,
    pub last_modified: Option<SystemTime>,
    pub file_size: Option<u64>,
}

pub enum BookDatabaseColumns {
    Title,
    Author,
    CoverImage,
    Format,
    Tags,
    Description,
    FilePath,
    Language,
    PageCount,
    Series,
    VolumeNumber,
    ChapterNumber,
    PageNumber,
    FileHash,
    LastModified,
    FileSize,
}

impl AsRef<str> for BookDatabaseColumns {
    fn as_ref(&self) -> &str {
        match self {
            BookDatabaseColumns::Title => "title",
            BookDatabaseColumns::Author => "author",
            BookDatabaseColumns::CoverImage => "cover_image",
            BookDatabaseColumns::Format => "format",
            BookDatabaseColumns::Tags => "tags",
            BookDatabaseColumns::Description => "description",
            BookDatabaseColumns::FilePath => "file_path",
            BookDatabaseColumns::Language => "language",
            BookDatabaseColumns::PageCount => "page_count",
            BookDatabaseColumns::Series => "series",
            BookDatabaseColumns::VolumeNumber => "volume_number",
            BookDatabaseColumns::ChapterNumber => "chapter_number",
            BookDatabaseColumns::PageNumber => "page_number",
            BookDatabaseColumns::FileHash => "file_hash",
            BookDatabaseColumns::LastModified => "last_modified",
            BookDatabaseColumns::FileSize => "file_size",
        }
    }
}

//============  Book series metadata model =================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BookSeriesMetadata {
    pub name: String,
    pub description: Option<String>,
    pub cover_image: Option<String>,
    pub start_release_year: Option<i64>,
    pub end_release_year: Option<i64>,
}

pub enum SeriesDatabaseColumns {
    Name,
    Description,
    CoverImage,
    StartReleaseYear,
    EndReleaseYear,
}

impl AsRef<str> for SeriesDatabaseColumns {
    fn as_ref(&self) -> &str {
        match self {
            SeriesDatabaseColumns::Name => "name",
            SeriesDatabaseColumns::Description => "description",
            SeriesDatabaseColumns::CoverImage => "cover_image",
            SeriesDatabaseColumns::StartReleaseYear => "start_release_year",
            SeriesDatabaseColumns::EndReleaseYear => "end_release_year",
        }
    }
}

//============  User metadata model =================
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserMetadata {
    pub username: String,
    pub password_hash: String,
    pub email: Option<String>,
    pub email_verified: bool,
    pub is_admin: bool,
    pub created_at: SystemTime,
    pub last_login: SystemTime,
}

pub enum UserDatabaseColumns {
    Username,
    PasswordHash,
    Email,
}
impl AsRef<str> for UserDatabaseColumns {
    fn as_ref(&self) -> &str {
        match self {
            UserDatabaseColumns::Username => "username",
            UserDatabaseColumns::PasswordHash => "password_hash",
            UserDatabaseColumns::Email => "email",
        }
    }
}

//============  Library metadata model =================
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LibraryMetadata {
    pub name: String,
    pub library_type: LibraryType,
    pub cover_image: Option<String>,
    pub description: Option<String>,
}

pub enum LibraryDatabaseColumns {
    Name,
    LibraryType,
    CoverImage,
    Description,
}

impl AsRef<str> for LibraryDatabaseColumns {
    fn as_ref(&self) -> &str {
        match self {
            LibraryDatabaseColumns::Name => "library_name",
            LibraryDatabaseColumns::LibraryType => "library_type",
            LibraryDatabaseColumns::CoverImage => "cover_image",
            LibraryDatabaseColumns::Description => "description",
        }
    }
}

//============  Column Selector enum =================
pub enum ColumnSelector {
    Book(BookDatabaseColumns),
    Library(LibraryDatabaseColumns),
    Series(SeriesDatabaseColumns),
    User(UserDatabaseColumns),
}

impl From<BookDatabaseColumns> for ColumnSelector {
    fn from(col: BookDatabaseColumns) -> Self {
        ColumnSelector::Book(col)
    }
}

impl From<LibraryDatabaseColumns> for ColumnSelector {
    fn from(col: LibraryDatabaseColumns) -> Self {
        ColumnSelector::Library(col)
    }
}

impl From<SeriesDatabaseColumns> for ColumnSelector {
    fn from(col: SeriesDatabaseColumns) -> Self {
        ColumnSelector::Series(col)
    }
}

impl From<UserDatabaseColumns> for ColumnSelector {
    fn from(col: UserDatabaseColumns) -> Self {
        ColumnSelector::User(col)
    }
}

//============  Series-Library connection model =================
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SeriesLibraryConnection {
    pub series_id: i64,
    pub library_id: i64,
}

pub enum SeriesLibraryConnectionDatabaseColumns {
    SeriesId,
    LibraryId,
}

impl AsRef<str> for SeriesLibraryConnectionDatabaseColumns {
    fn as_ref(&self) -> &str {
        match self {
            SeriesLibraryConnectionDatabaseColumns::SeriesId => "series_id",
            SeriesLibraryConnectionDatabaseColumns::LibraryId => "library_id",
        }
    }
}

//============Parsed Name model =================
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedName {
    pub title: String,
    pub volume_number: Option<i64>,
    pub chapter_number: Option<i64>,
    pub page_number: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileExtractedMetadata {
    pub file_size: Option<u64>,
    pub modified_date: Option<SystemTime>,
    pub page_number: Option<i32>,
    pub language_code: BookLanguage,
    pub author: String,
}

pub enum DatabaseEntry {
    Book(BookMetadata),
    Library(LibraryMetadata),
    Series(BookSeriesMetadata),
    User(UserMetadata),
}

//====================Email Verification====================
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmailVerification {
    pub user_id: i64,
    pub verification_token: String,
    pub expiration_date: SystemTime,
    pub invalidated: bool,
}

pub enum EmailVerificationDatabaseColumns {
    UserId,
    VerificationToken,
    ExpirationDate,
    Invalidated,
}

impl AsRef<str> for EmailVerificationDatabaseColumns {
    fn as_ref(&self) -> &str {
        match self {
            EmailVerificationDatabaseColumns::UserId => "user_id",
            EmailVerificationDatabaseColumns::VerificationToken => "email_verification_token",
            EmailVerificationDatabaseColumns::ExpirationDate => "expires_at",
            EmailVerificationDatabaseColumns::Invalidated => "invalidated",
        }
    }
}

//===============Sessions==================
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Sessions {
    pub session_id: i64,
    pub user_id: i64,
    pub refresh_token: String,
    pub session_token: String,
    pub refresh_token_expiration_date: SystemTime,
    pub session_token_expiration_date: SystemTime,
    pub created_at: SystemTime,
    pub last_used_at: SystemTime,
}

pub enum SessionsDatabaseColumns {
    SessionId,
    UserId,
    RefreshToken,
    SessionToken,
    RefreshTokenExpirationDate,
    SessionTokenExpirationDate,
    CreatedAt,
    LastUsedAt,
}

impl AsRef<str> for SessionsDatabaseColumns {
    fn as_ref(&self) -> &str {
        match self {
            SessionsDatabaseColumns::SessionId => "session_id",
            SessionsDatabaseColumns::UserId => "user_id",
            SessionsDatabaseColumns::RefreshToken => "refresh_token_hashed",
            SessionsDatabaseColumns::SessionToken => "session_token_hashed",
            SessionsDatabaseColumns::RefreshTokenExpirationDate => "refresh_token_valid_until",
            SessionsDatabaseColumns::SessionTokenExpirationDate => "session_token_valid_until",
            SessionsDatabaseColumns::CreatedAt => "created_at",
            SessionsDatabaseColumns::LastUsedAt => "last_used_at",
        }
    }
}

//===============Recovery codes================
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecoveryCodes {
    pub recovery_id: i64,
    pub user_id: i64,
    pub code_hashed: String,
    pub used: bool,
}

pub enum RecoveryCodesColumns {
    RecoveryId,
    UserId,
    CodeHashed,
    Used,
}

impl AsRef<str> for RecoveryCodesColumns {
    fn as_ref(&self) -> &str {
        match self {
            RecoveryCodesColumns::RecoveryId => "recovery_id",
            RecoveryCodesColumns::UserId => "user_id",
            RecoveryCodesColumns::CodeHashed => "code_hash",
            RecoveryCodesColumns::Used => "used",
        }
    }
}

//===================TOTP======================
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TOTP {
    pub user_id: i64,
    pub authentication_secret: String,
    pub created_at: SystemTime,
}

pub enum TOTPDatabaseColumns {
    UserId,
    AuthenticationSecret,
    CreatedAt,
}

impl AsRef<str> for TOTPDatabaseColumns {
    fn as_ref(&self) -> &str {
        match self {
            TOTPDatabaseColumns::UserId => "user_id",
            TOTPDatabaseColumns::AuthenticationSecret => "authentication_secret",
            TOTPDatabaseColumns::CreatedAt => "created_at",
        }
    }
}
