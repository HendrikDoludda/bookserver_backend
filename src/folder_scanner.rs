use crate::config;
use crate::cover_image_retriever::get_cover_image;
use crate::db::Database;
use crate::error_types::FolderScannerError;
use crate::models::{
    BookDatabaseColumns, BookFormat, BookMetadata, BookSeriesMetadata, ColumnSelector,
    DatabaseTypes, FileExtractedMetadata, ParsedName,
};
use blake3::Hasher;
use log::warn;
use once_cell::sync::Lazy;
use regex::Regex;
use std::{
    fs::File,
    io::{self},
    path::{Path, PathBuf},
    sync::Arc,
};
use tokio::{fs, sync::Semaphore};
use crate::insert::Insert;

//preloaded regex expressions that can be used without the need to re-create any
static VOLUME_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)\b(?:v|vol|volume)[ _]?(\d+)\b").unwrap());

static CHAPTER_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)\b(?:c|chap|chapter)[ _]?(\d+)\b").unwrap());

static PAGE_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)\b(?:p|pa|page)[ _]?(\d+)\b").unwrap());

//This function will scan all assigned folders for valid file types
//creates entries for them in the database
//should be done async and be able to run multiple in parralel
pub async fn scan_all_folders() -> anyhow::Result<()> {
    let folders = config::get_books_dirs();
    let db = Arc::new(Database::new()?);

    for folder in folders {
        scan_folder(PathBuf::from(folder), db.clone()).await?;
    }

    Ok(())
}

//This function will scan a single folder for valid file types
//creates entries for them in the database
pub async fn scan_folder(
    path: PathBuf,
    db: Arc<Database>,
) -> anyhow::Result<(), FolderScannerError> {
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

                    if let Err(e) =
                        tokio::task::spawn_blocking(move || create_book_entry(&entry_path, &db))
                            .await
                    {
                        eprintln!("Task join error: {e}");
                    }
                });
            }
        }
    }

    Ok(())
}

//This function will be run when a valid file type has been found
fn create_book_entry(path: &Path, db: &Database) -> anyhow::Result<(), FolderScannerError> {
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
        //check for modification as well and then update
        return Ok(());
    }

    let parsed_name = get_final_file_name(&final_path);
    let series_id = match get_series_id(parsed_name.title.clone(), db) {
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
        format: format,
        tags: vec![],
        description: None,
        file_path: final_path.to_string_lossy().to_string(),
        language: metadata.language_code,
        page_count: metadata.page_number,
        cover_image: cover_image,
        series: series_id,
        volume_number: parsed_name.volume_number,
        chapter_number: parsed_name.chapter_number,
        page_number: parsed_name.page_number,
        file_hash: file_hash,
        last_modified: metadata.modified_date,
        file_size: metadata.file_size,
    };

    db.insert( &metadata_entry)
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
            language_code: crate::models::BookLanguage::Other("Unkown".to_string()),
            author: "Unknown Author".to_string(),
        }
    } else {
        FileExtractedMetadata {
            file_size: None,
            modified_date: None,
            page_number: None,
            language_code: crate::models::BookLanguage::Other("Unkown".to_string()),
            author: "Unknown Author".to_string(),
        }
    }
}

fn get_cover_image_location(path: &PathBuf, format: BookFormat, hash: &String) -> Option<String> {
    let cover_image_location = get_cover_image(path, format, hash);
    let cover_image = match cover_image_location {
        Ok(path) => path.to_str().map(String::from),
        Err(e) => {
            eprintln!("Failed cover creation for path {:?}: {}", path, e);
            None
        }
    };
    cover_image
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

fn check_folder_path_exists_in_db(path: &PathBuf, db: &Database) -> bool {
    let result = db.get_id_from_table(
        DatabaseTypes::Books,
        ColumnSelector::Book(BookDatabaseColumns::FilePath),
        &path.to_string_lossy().to_string(),
    );
    match result {
        Ok(Some(_)) => true,
        _ => false,
    }
}
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

fn get_book_format(path: &Path) -> anyhow::Result<BookFormat, FolderScannerError> {
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

fn get_series_id(file_name: String, db: &Database) -> Result<i64, FolderScannerError> {
    match db.get_id_from_table(
        DatabaseTypes::Series,
        ColumnSelector::Series(crate::models::SeriesDatabaseColumns::Name),
        &file_name.to_lowercase(),
    ) {
        Ok(Some(id)) => Ok(id), // found an existing series
        Ok(None) | Err(_) => create_series_entry(file_name, db), // not found or DB error
    }
}

fn create_series_entry(file_name: String, db: &Database) -> Result<i64, FolderScannerError> {
    let series_entry = BookSeriesMetadata {
        name: file_name.to_lowercase(),
        description: None,
        cover_image: None,
        start_release_year: None,
        end_release_year: None,
    };
    db.insert(&series_entry)
        .map_err(|_| FolderScannerError::SeriesInsertionFailed)
}
