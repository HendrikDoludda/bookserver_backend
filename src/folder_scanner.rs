use crate::config;
use crate::db::Database;
use crate::models::{
    BookDatabaseColumns, BookFormat, BookMetadata, BookSeriesMetadata, ColumnSelector, DatabaseTypes, ParsedName
};
use crate::cover_image_retriever::get_cover_image;
use blake3::Hasher;
use once_cell::sync::Lazy;
use regex::Regex;
use std::{
    fs::File,
    io::{self},
    path::{Path, PathBuf},
    sync::Arc,
};
use tokio::{fs, sync::Semaphore};

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
pub async fn scan_folder(path: PathBuf, db: Arc<Database>) -> anyhow::Result<()> {
    let semaphore = Arc::new(Semaphore::new(1)); // limit concurrency
    let mut directories = vec![path];

    while let Some(dir) = directories.pop() {
        let mut entries = fs::read_dir(dir).await?;

        while let Some(entry) = entries.next_entry().await? {
            let entry_path = entry.path();

            if entry_path.is_dir() {
                directories.push(entry_path);
            } else {
                let db = db.clone();
                let permit = semaphore.clone().acquire_owned().await?;

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
fn create_book_entry(path: &Path, db: &Database) -> anyhow::Result<()> {
    if !is_valid_file_type(path) {
        return Ok(());
    }

    let format = get_book_format(path)?;

    let file = File::open(path)?;
    let metadata = file.metadata()?;

    let modified_time = metadata.modified().ok();
    let file_size = metadata.len();

    let file_hash = hash_file(file)?;

    let final_path = get_final_folder_path(path, format.as_str());
    if check_folder_path_exists_in_db(&final_path, db){
        return Ok(());
    }

    let parsed_name = get_final_file_name(&final_path);
    let series_id: i64 = get_series_id(parsed_name.title.clone(), db);
    let cover_image = get_cover_image_location(&final_path,format.clone(),&file_hash.clone());

    let metadata_entry = BookMetadata {
        title: parsed_name.title,
        author: "Unknown Author".to_string(),
        format: format,
        tags: vec![],
        description: None,
        file_path: final_path.to_string_lossy().to_string(),
        language: crate::models::BookLanguage::Other("Unknown".to_string()),
        page_count: Some(0),
        cover_image: cover_image,
        series: Some(series_id),
        volume_number: parsed_name.volume_number,
        chapter_number: parsed_name.chapter_number,
        page_number: parsed_name.page_number,
        file_hash: Some(file_hash),
        last_modified: modified_time,
        file_size: Some(file_size),
    };

    db.insert(DatabaseTypes::Books, &metadata_entry)?;

    Ok(())
}

fn get_cover_image_location(path: &PathBuf, format: BookFormat, hash: &String) -> Option<String>{
    let cover_image_location = get_cover_image(path, format, hash);
    let cover_image = match cover_image_location {
    Ok(path) => path.to_str().map(String::from),
    Err(e) => {
        eprintln!(
            "Failed cover creation for path {:?}: {}",
            path, e
        );
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
            &CHAPTER_REGEX.replace_all(
                &VOLUME_REGEX.replace_all(&file_name, ""),
                "",
            ),
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

fn get_book_format(path: &Path) -> anyhow::Result<BookFormat> {
    match path.extension().and_then(|ext| ext.to_str()) {
        Some("pdf") => Ok(BookFormat::Pdf),
        Some("epub") => Ok(BookFormat::Epub),
        Some("cbz") => Ok(BookFormat::Cbz),
        Some("jpg" | "jpeg" | "png") => Ok(BookFormat::ImageComic),
        _ => Err(anyhow::anyhow!("Unsupported file format")),
    }
}


fn hash_file(mut file: File) -> io::Result<String> {
    let mut hasher = Hasher::new();
    std::io::copy(&mut file, &mut hasher)?;
    Ok(hasher.finalize().to_hex().to_string())
}

fn get_series_id(file_name: String, db: &Database) -> i64{
    match db.get_id_from_table(
        DatabaseTypes::Series,
        ColumnSelector::Series(crate::models::SeriesDatabaseColumns::Name),
        &file_name.to_lowercase(),
    ) {
        Ok(Some(id)) => id, // found an existing series
        Ok(None) | Err(_) => create_series_entry(file_name, db), // not found or DB error
    }
}

fn create_series_entry(file_name: String, db: &Database) -> i64{
    let series_entry = BookSeriesMetadata{
        name: file_name.to_lowercase(),
        description: None,
        cover_image: None,
        start_release_year: None,
        end_release_year: None
    };
    db.insert(DatabaseTypes::Series, &series_entry).unwrap_or_default()
}

