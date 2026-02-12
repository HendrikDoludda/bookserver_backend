use crate::config;
use crate::db::Database;
use crate::models::{
    BookDatabaseColumns, BookFormat, BookMetadata, ColumnSelector, DatabaseTypes, ParsedName,
};
use regex::Regex;
use std::{fs::File, io::Read, io, path::PathBuf};
use tokio::fs::{self as other_fs};
use blake3;

//This function will scan all assigned folders for valid file types
//creates entries for them in the database
//should be done async and be able to run multiple in parralel
pub async fn scan_all_folders() -> Result<(), ()> {
    let folders_to_scan: Vec<String> = config::get_books_dirs();
    let db = Database::new().unwrap();
    for folder in folders_to_scan {
        let path: PathBuf = PathBuf::from(folder);
        scan_folder(path, &db).await.unwrap();
    }
    Ok(())
}

//This function will scan a single folder for valid file types
//creates entries for them in the database
pub async fn scan_folder(path: PathBuf, db: &Database) -> Result<(), io::Error> {
    let mut directories = vec![path];
    while let Some(dir) = directories.pop() {
        let mut entries = other_fs::read_dir(dir).await?;

        while let Some(entry) = entries.next_entry().await? {
            let entry_path = entry.path();
            if entry_path.is_dir() {
                directories.push(entry_path);
            } else {
                create_book_entry(&entry_path, db).ok();
            }
        }
    }

    Ok(())
}

//This function will be run when a valid file type has been found
fn create_book_entry(path: &PathBuf, db: &Database) -> Result<(), ()> {
    if is_valid_file_type(path) {
        let format = get_book_format(path);
        let (modified_time, file_size) = match path.metadata() {
            Ok(meta) => (meta.modified().ok(), Some(meta.len())),
            Err(_) => (None, None),
        };
        //get final folder path for entry
        let final_folder_path = get_final_folder_path(path, &format);
        //check if folder path exists in database
        if check_folder_path_exists_in_db(&final_folder_path, db) {
            println!(
                "Folder path already exists in database: {}",
                final_folder_path.display()
            );
            return Ok(());
        }
        //create final book name (removing Volume/chapter/page information if it exists)
        let file_name_information = get_final_file_name(&final_folder_path);
        //create basic metadata for book entry
        let file_hash = get_file_hash(&final_folder_path).ok();

        let metadata = BookMetadata {
            title: file_name_information.title,
            author: "Unknown Author".to_string(),
            format: BookFormat::from_str(&format),
            tags: vec![],
            description: None,
            file_path: final_folder_path.to_string_lossy().to_string(),
            language: crate::models::BookLanguage::Other("Unknown".to_string()),
            page_count: Some(0),
            cover_image: None,
            series: None,
            volume_number: file_name_information.volume_number,
            chapter_number: file_name_information.chapter_number,
            page_number: file_name_information.page_number,
            file_hash: file_hash,
            last_modified: modified_time,
            file_size: file_size,
        };

        //for file formats that support it we can create a partial metadata entry and update the basic one
        //should check for page count and cover image, language code if the file type supports it

        println!("Found valid file: {:?} with format: {}", path, format);
        if let Err(e) = db.insert(DatabaseTypes::Books, &metadata) {
            eprintln!("Failed to insert book: {e}");
        }
    }
    Ok(())
}

fn get_final_folder_path(path: &PathBuf, format: &str) -> PathBuf {
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

fn get_final_file_name(path: &PathBuf) -> ParsedName {
    let file_name = path.file_stem().unwrap().to_string_lossy().to_string();
    let volume_regex = Regex::new(r"(?i)\b(?:v|vol|volume)[ _]?(\d+)\b").unwrap();
    let chapter_regex = Regex::new(r"(?i)\b(?:c|chap|chapter)[ _]?(\d+)\b").unwrap();
    let page_regex = Regex::new(r"(?i)\b(?:p|pa|page)[ _]?(\d+)\b").unwrap();

    let volume = volume_regex
        .captures(&file_name)
        .and_then(|cap| cap.get(1))
        .map(|m| m.as_str().parse::<i64>().ok())
        .flatten();
    let chapter = chapter_regex
        .captures(&file_name)
        .and_then(|cap| cap.get(1))
        .map(|m| m.as_str().parse::<i64>().ok())
        .flatten();
    let page = page_regex
        .captures(&file_name)
        .and_then(|cap| cap.get(1))
        .map(|m| m.as_str().parse::<i64>().ok())
        .flatten();

    let cleaned_name = volume_regex.replace_all(&file_name, "").to_string();
    let cleaned_name = chapter_regex.replace_all(&cleaned_name, "").to_string();
    let cleaned_name = page_regex.replace_all(&cleaned_name, "").to_string();

    let parts: String = cleaned_name
        .replace('-', " ")
        .replace('_', " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .to_string();

    ParsedName {
        title: parts,
        volume_number: volume,
        chapter_number: chapter,
        page_number: page,
    }
}

fn is_valid_file_type(path: &PathBuf) -> bool {
    match path.extension().and_then(|ext| ext.to_str()) {
        Some("pdf") | Some("epub") | Some("cbz") | Some("jpg") | Some("jpeg") | Some("png") => true,
        _ => false,
    }
}

fn get_book_format(path: &PathBuf) -> String {
    match path.extension().and_then(|ext| ext.to_str()) {
        Some("pdf") => BookFormat::Pdf.as_str().to_string(),
        Some("epub") => BookFormat::Epub.as_str().to_string(),
        Some("cbz") => BookFormat::Cbz.as_str().to_string(),
        Some("jpg") | Some("jpeg") | Some("png") => BookFormat::ImageComic.as_str().to_string(),
        _default => {
            println!("Invalid file type: {:?}", path);
            BookFormat::None.as_str().to_string()
        }
    }
}

fn get_file_hash(path: &PathBuf)-> io::Result<String> {
    let mut file = File::open(path)?;
    let mut hasher = blake3::Hasher::new();

    std::io::copy(&mut file, &mut hasher)?;

    Ok(hasher.finalize().to_hex().to_string())
}

fn create_series_entry() -> Result<(), ()> {
    Ok(())
}
