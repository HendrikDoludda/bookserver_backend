use std::{fs::ReadDir, path::Path};

pub fn initiate_folder_scanner() -> crate::models::ScanResult {
    let book_dirs: Vec<String> = crate::config::get_books_dirs();
    let mut scan_result = crate::models::ScanResult {
        series: Vec::new(),
        books: Vec::new(),
    };

    for dir in book_dirs {
        println!("Scanning directory: {}", dir);
        scan_directory(dir, &mut scan_result);
    }
    // Implementation for folder scanning

    scan_result
}
//implement parallel scanning rather than sequential
fn scan_directory(dir: ReadDir, scan_result: &mut crate::models::ScanResult) {
    for entry in dir {
        if let Ok(entry) = entry {
            let path = entry.path();
            if path.is_dir() {
                // Recursively scan subdirectories
                if let Ok(sub_dir) = path.read_dir() {
                    scan_directory(sub_dir, scan_result);
                }
            } else {
                // Process files
                if let Some(extension) = path.extension() {
                    match extension.to_str().unwrap_or("").to_lowercase().as_str() {
                        "pdf" | "epub" | "cbz" => {
                            let format =
                                match extension.to_str().unwrap_or("").to_lowercase().as_str() {
                                    "pdf" => crate::models::BookFormat::Pdf,
                                    "epub" => crate::models::BookFormat::Epub,
                                    "cbz" => crate::models::BookFormat::Cbz,
                                    _ => crate::models::BookFormat::None, // Default to Pdf if unknown
                                };
                            let bookEntry = create_book_entry(path.as_path(), format);
                            scan_result.books.push(bookEntry);
                        }
                        "jpg" | "png" | "jpeg" => {
                            // Handle image files for comics
                        }
                        _ => {}
                    }
                }
            }
        }
    }
}

fn create_series_if_not_exists(
    name: &str,
    book: crate::models::BookMetadata,
    scan_result: &mut crate::models::ScanResult,
) {
    if !scan_result.series.iter().any(|s| s.name == name) {
        let mut new_series = crate::models::BookSeriesMetadata {
            name: name.to_string(),
            books: Vec::new(),
            cover_image: None,
            description: None,
            tags: Vec::new(),
        };
        new_series.books.push(book);
        scan_result.series.push(new_series);
    }
}

fn create_book_entry(
    path: &Path,
    fileType: crate::models::BookFormat,
) -> crate::models::BookMetadata {
    crate::models::BookMetadata {
        title: path.file_name().unwrap().to_string_lossy().to_string(),
        author: String::new(),
        cover_image: None,
        format: fileType,
        tags: Vec::new(),
        description: None,
        series: None,
        series_index: None,
        folder_path: vec![path.parent().unwrap().to_string_lossy().to_string()],
        language: None,
    }
}
