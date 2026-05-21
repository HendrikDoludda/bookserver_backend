use axum::{
    routing::{delete, get, post, put},
    Router,
};

use crate::config::get_port;
use crate::routes::api_caller::{
    delete_library, get_libraries, get_series_children, get_series_in_library, initiate_database,
    insert_database_entry, ping_server, request_file, scan_all_directories, scan_for_metadata,
    update_database_entry,
};
use tokio::net::TcpListener;
use tracing::info;

pub async fn start_server() {
    let app = set_up_routes();

    let port = get_port();
    let address = format!("0.0.0.0:{}", port);

    info!("Starting server on {}!", address);

    let listener = TcpListener::bind(address).await.unwrap();

    axum::serve(listener, app).await.unwrap();
}
//change get to others
/*
Rule of thumb:
GET → read
POST → create
PUT/PATCH → update
DELETE → delete
*/
pub fn set_up_routes() -> Router {
    Router::new()
        .route("/health", get(ping_server))
        .route("/get_libraries", get(get_libraries))
        .route("/get_series_from_library", get(get_series_in_library))
        .route("/get_series_children", get(get_series_children))
        .route("/update_entry", put(update_database_entry))
        .route("/insert_entry", post(insert_database_entry))
        .route("/scan_metadata", post(scan_for_metadata))
        .route("/setup_db", post(initiate_database))
        .route("/scan_all_directories", post(scan_all_directories))
        .route("/book/:id", get(request_file))
        .route("/delete_library/:id", delete(delete_library))
}
