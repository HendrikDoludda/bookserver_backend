use crate::{config};
use crate::models::{BookFormat, BookMetadata, DatabaseTypes};
use std::{io, path::PathBuf};
use tokio::fs::{self};
use crate::db::{Database};

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
        let mut entries = fs::read_dir(dir).await?;

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
        //get final folder path for entry
        //check if folder path exists in database
        
        //TODO: Add better metadata extraction here, maybe use a library for this
        //For now we just use the file name as the title and set everything else to default values
        //For image formats we need to encasulate all the images in a folder and create a book entry for the folder instead of the individual images
        let metadata = BookMetadata{
            title: path.file_stem().unwrap().to_string_lossy().to_string(),
            author: "Unknown Author".to_string(),
            format: BookFormat::from_str(&format),
            tags: vec![],
            description: None,
            folder_path: vec![path.to_string_lossy().to_string()],
            language: crate::models::BookLanguage::Other("Unknown".to_string()),
            page_count: Some(0),
            cover_image: None,
            series: None,
        };

        println!("Found valid file: {:?} with format: {}", path, format);
        db.insert(DatabaseTypes::Books, &metadata).unwrap();
    }
    Ok(())
}

fn is_image_inside_image_folder(path: &PathBuf, format: &str) {
    if (BookFormat::ImageComic.as_str() == format) && path.parent().is_some() {
        let parent = path.parent().unwrap();
        if parent.is_dir() {

        }
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

fn create_series_entry() -> Result<(), ()> {
    Ok(())
}
