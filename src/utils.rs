use std::fs;
use std::io;

pub fn convert_file_path_to_blob(path: &str) -> io::Result<Vec<u8>> {
    fs::read(path)
}
