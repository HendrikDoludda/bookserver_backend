pub mod app_state;
pub mod config;
pub mod data_models;
pub mod database_related_scripts;
pub mod error_types;
pub mod routes;
pub mod scanner;
pub mod stream_reader;
pub mod utils;

#[cfg(test)]
mod test_support;

// Short paths for the most-used modules, so code can write `crate::models::…`
// and `crate::db::…` instead of the full nested path.
pub use data_models::models;
pub use database_related_scripts::db;
pub use scanner::cover_image_retriever;
pub use scanner::folder_scanner;
