use std::env;

pub fn get_port() -> u16 {
    env::var("PORT")
        .unwrap_or_else(|_| "8080".to_string())
        .parse()
        .expect("Invalid port number")
}

pub fn get_books_dirs() -> Vec<String> {
    env::var("BOOK_DIRS")
        .unwrap_or_else(|_| "../Downloads".to_string())
        .split(':')  // Assuming ':' as separator; adjust if needed
        .map(|s| s.trim().to_string())
        .collect()
}

pub fn get_api_key() -> Option<String> {
    env::var("API_KEY").ok()
}