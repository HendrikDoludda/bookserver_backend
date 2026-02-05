use std::{fs::ReadDir, path::Path};
use crate::config;


//This function will scan all assigned folders for valid file types
//creates entries for them in the database
//should be done async and be able to run multiple in parralel
#[tokio::main]
pub async fn scan_all_folders() -> Result<(),()>{
    let folders_to_scan: Vec<String> = config::get_books_dirs();
    
    Ok(())
}

//This function will scan a single folder for valid file types
//creates entries for them in the database
pub async fn scan_folder(path: &str) -> Result<(),()>{

    Ok(())
}

//This function will be run when a valid file type has been found
fn create_book_entry() -> Result<(),()>{

    Ok(())
}

fn create_series_entry() -> Result<(),()>{

    Ok(())
}