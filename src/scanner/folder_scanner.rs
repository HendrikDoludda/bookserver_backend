use std::{
    fs::File,
    io,
    path::{Path, PathBuf},
    sync::Arc,
};

use blake3::Hasher;
use tracing::{error, warn};
use once_cell::sync::Lazy;
use regex::Regex;
use tokio::{fs, sync::Semaphore};

use crate::config;
use crate::data_models::models::{
    BookDatabaseColumns, BookFormat, BookLanguage, BookMetadata, BookSeriesMetadata,
    ColumnSelector, DatabaseTypes, FileExtractedMetadata, LibraryDatabaseColumns, LibraryMetadata,
    LibraryType, ParsedName, SeriesDatabaseColumns, SeriesLibraryConnection,
};
use crate::db::Database;
use crate::error_types::{DatabaseError, FolderScannerError};
use crate::scanner::cover_image_retriever::get_cover_image;

// Filename patterns, compiled once on first use and then reused.
static VOLUME_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)\b(?:v|vol|volume)[ _]?(\d+)\b").unwrap());

static CHAPTER_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)\b(?:c|chap|chapter)[ _]?(\d+)\b").unwrap());

static PAGE_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)\b(?:p|pa|page)[ _]?(\d+)\b").unwrap());

/// Scans every directory in `BOOK_DIRS` for supported files and creates database
/// entries for them. Each directory becomes (or reuses) a library.
pub async fn scan_all_folders(db: Arc<Database>) -> anyhow::Result<()> {
    let folders = config::get_books_dirs();

    for folder in folders {
        let library_id = get_library_id(folder.clone(), &db)?;
        scan_folder(PathBuf::from(folder), db.clone(), library_id).await?;
    }

    Ok(())
}

/// Walks one directory tree and creates a book entry for each supported file.
/// File processing runs on the blocking thread pool, gated by a semaphore.
pub async fn scan_folder(
    path: PathBuf,
    db: Arc<Database>,
    library_id: i64,
) -> Result<(), FolderScannerError> {
    let semaphore = Arc::new(Semaphore::new(1)); // limit concurrency
    let mut directories = vec![path];

    while let Some(dir) = directories.pop() {
        let mut entries = fs::read_dir(dir)
            .await
            .map_err(|_| FolderScannerError::MissingDirectory)?;

        while let Some(entry) = entries
            .next_entry()
            .await
            .map_err(|_| FolderScannerError::FileNotFound)?
        {
            let entry_path = entry.path();

            if entry_path.is_dir() {
                directories.push(entry_path);
            } else {
                let db = db.clone();
                let permit = semaphore
                    .clone()
                    .acquire_owned()
                    .await
                    .map_err(|_| FolderScannerError::PermitCreationFailed);

                tokio::spawn(async move {
                    let _permit = permit;

                    if let Err(e) = tokio::task::spawn_blocking(move || {
                        create_book_entry(&entry_path, &db, library_id)
                    })
                    .await
                    {
                        error!("Task join error: {e}");
                    }
                });
            }
        }
    }

    Ok(())
}

/// Creates the database entry (plus series and cover image) for one file.
/// Unsupported files and files already in the database are skipped.
fn create_book_entry(
    path: &Path,
    db: &Database,
    library_id: i64,
) -> Result<(), FolderScannerError> {
    if !is_valid_file_type(path) {
        return Ok(());
    }

    let format = get_book_format(path)?;

    let file = File::open(path).map_err(|_| FolderScannerError::FileNotFound)?;
    let metadata = get_file_metadata(&file, path);

    let file_hash = match hash_file(file).map_err(|_| FolderScannerError::FileHashingFailed) {
        Ok(hash) => Some(hash),
        Err(e) => {
            warn!("Failed to hash the file {:?}", e);
            None
        }
    };

    let final_path = get_final_folder_path(path, format.as_str());
    if check_folder_path_exists_in_db(&final_path, db) {
        // TODO: also detect modified files and update their entry.
        return Ok(());
    }

    let parsed_name = get_final_file_name(&final_path);
    let series_id = match get_series_id(parsed_name.title.clone(), db, library_id) {
        Ok(id) => Some(id),
        Err(e) => {
            warn!("Failed to get series id because of : {:?}", e);
            None
        }
    };
    let cover_image = get_cover_image_location(
        &final_path,
        format.clone(),
        &file_hash.clone().unwrap_or_default(),
    );

    let metadata_entry = BookMetadata {
        title: parsed_name.title,
        author: metadata.author,
        format,
        tags: vec![],
        description: None,
        file_path: final_path.to_string_lossy().to_string(),
        language: metadata.language_code,
        page_count: metadata.page_number,
        cover_image,
        series: series_id,
        volume_number: parsed_name.volume_number,
        chapter_number: parsed_name.chapter_number,
        page_number: parsed_name.page_number,
        file_hash,
        last_modified: metadata.modified_date,
        file_size: metadata.file_size,
    };

    db.insert(&metadata_entry)
        .map_err(|_| FolderScannerError::BookInsertionFailed)?;

    Ok(())
}

fn get_file_metadata(file: &File, path: &Path) -> FileExtractedMetadata {
    let metadata = match file
        .metadata()
        .map_err(|_| FolderScannerError::MetadataExtractionFailed)
    {
        Ok(m) => Some(m),
        Err(e) => {
            warn!("Could not extract {:?}'s metadata  {:?}", path, e);
            None
        }
    };

    if let Some(metadata) = metadata {
        let modified_time = match metadata
            .modified()
            .map_err(|_| FolderScannerError::UnsupportedOperatingSystem)
        {
            Ok(date) => Some(date),
            Err(e) => {
                warn!("Failed to retrieve the modified date from file: {:?}", e);
                None
            }
        };
        let file_size = metadata.len();

        FileExtractedMetadata {
            file_size: Some(file_size),
            modified_date: modified_time,
            page_number: None,
            language_code: BookLanguage::Other("Unknown".to_string()),
            author: "Unknown Author".to_string(),
        }
    } else {
        FileExtractedMetadata {
            file_size: None,
            modified_date: None,
            page_number: None,
            language_code: BookLanguage::Other("Unknown".to_string()),
            author: "Unknown Author".to_string(),
        }
    }
}

fn get_cover_image_location(path: &Path, format: BookFormat, hash: &str) -> Option<String> {
    match get_cover_image(path, format, hash) {
        Ok(cover_path) => cover_path.to_str().map(String::from),
        Err(e) => {
            error!("Failed cover creation for path {:?}: {}", path, e);
            None
        }
    }
}

fn get_final_folder_path(path: &Path, format: &str) -> PathBuf {
    if (BookFormat::ImageComic.as_str() == format) && path.parent().is_some() {
        let parent = path.parent().unwrap();
        if parent.is_dir() {
            return parent.to_path_buf();
        }
    }
    path.to_path_buf()
}

fn check_folder_path_exists_in_db(path: &Path, db: &Database) -> bool {
    let result = db.get_id_from_table(
        DatabaseTypes::Books,
        ColumnSelector::Book(BookDatabaseColumns::FilePath),
        &path.to_string_lossy().to_string(),
    );
    matches!(result, Ok(Some(_)))
}

/// Splits a filename like `My_Series v02 c13` into a clean title plus the
/// volume / chapter / page numbers found in it.
fn get_final_file_name(path: &Path) -> ParsedName {
    let file_name = path
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    let volume = extract_number(&VOLUME_REGEX, &file_name);
    let chapter = extract_number(&CHAPTER_REGEX, &file_name);
    let page = extract_number(&PAGE_REGEX, &file_name);

    let cleaned = PAGE_REGEX
        .replace_all(
            &CHAPTER_REGEX.replace_all(&VOLUME_REGEX.replace_all(&file_name, ""), ""),
            "",
        )
        .to_string();

    let title = cleaned
        .replace(['-', '_'], " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");

    ParsedName {
        title,
        volume_number: volume,
        chapter_number: chapter,
        page_number: page,
    }
}

fn extract_number(regex: &Regex, text: &str) -> Option<i64> {
    regex
        .captures(text)
        .and_then(|cap| cap.get(1))
        .and_then(|m| m.as_str().parse::<i64>().ok())
}

fn is_valid_file_type(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|ext| ext.to_str()),
        Some("pdf" | "epub" | "cbz" | "jpg" | "jpeg" | "png")
    )
}

fn get_book_format(path: &Path) -> Result<BookFormat, FolderScannerError> {
    match path.extension().and_then(|ext| ext.to_str()) {
        Some("pdf") => Ok(BookFormat::Pdf),
        Some("epub") => Ok(BookFormat::Epub),
        Some("cbz") => Ok(BookFormat::Cbz),
        Some("jpg" | "jpeg" | "png") => Ok(BookFormat::ImageComic),
        _ => Err(FolderScannerError::FileTypeNotRecognized),
    }
}

fn hash_file(mut file: File) -> io::Result<String> {
    let mut hasher = Hasher::new();
    std::io::copy(&mut file, &mut hasher)?;
    Ok(hasher.finalize().to_hex().to_string())
}

fn get_series_id(
    file_name: String,
    db: &Database,
    library_id: i64,
) -> Result<i64, FolderScannerError> {
    match db.get_id_from_table(
        DatabaseTypes::Series,
        ColumnSelector::Series(SeriesDatabaseColumns::Name),
        &file_name.to_lowercase(),
    ) {
        Ok(Some(id)) => Ok(id), // found an existing series
        Ok(None) | Err(_) => create_series_entry(file_name, db, library_id), // not found or DB error
    }
}

fn create_series_entry(
    file_name: String,
    db: &Database,
    library_id: i64,
) -> Result<i64, FolderScannerError> {
    let series_entry = BookSeriesMetadata {
        name: file_name.to_lowercase(),
        description: None,
        cover_image: None,
        start_release_year: None,
        end_release_year: None,
    };
    let id = db
        .insert(&series_entry)
        .map_err(|_| FolderScannerError::SeriesInsertionFailed)?;
    link_series_to_default_library(id, library_id, db)
        .map_err(|_| FolderScannerError::SeriesInsertionFailed)?;
    Ok(id)
}

fn link_series_to_default_library(
    series_id: i64,
    library_id: i64,
    db: &Database,
) -> Result<(), DatabaseError> {
    let linked_series = SeriesLibraryConnection {
        series_id,
        library_id,
    };
    db.insert(&linked_series)?;
    Ok(())
}

fn get_library_id(name: String, db: &Database) -> Result<i64, DatabaseError> {
    match db.get_id_from_table(
        DatabaseTypes::Library,
        ColumnSelector::Library(LibraryDatabaseColumns::Name),
        &name.to_lowercase(),
    ) {
        Ok(Some(result)) => Ok(result),
        Ok(None) | Err(_) => create_library_entry(name, db),
    }
}

fn create_library_entry(name: String, db: &Database) -> Result<i64, DatabaseError> {
    let library_entry = LibraryMetadata {
        name: name.to_lowercase(),
        library_type: LibraryType::Books,
        cover_image: None,
        description: None,
    };
    db.insert(&library_entry)
}
