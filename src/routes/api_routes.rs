use std::sync::Arc;

use axum::{
    extract::State,
    routing::{delete, get, patch, post, put},
    Router,
};

use crate::{config::get_port, db::Database};
use crate::{
    error_types::RoutingErrors,
    routes::api_caller::{
        delete_library, get_libraries, get_series_children, get_series_in_library,
        initiate_database, insert_database_entry, ping_server, request_file, scan_all_directories,
        scan_for_metadata, update_database_entry,
    },
};
use tokio::net::TcpListener;
use tracing::info;

pub async fn start_server(db: Arc<Database>) -> Result<(), RoutingErrors> {
    let app = set_up_routes().with_state(db);

    let port = get_port();
    let address = format!("0.0.0.0:{}", port);

    info!("Starting server on {}!", address);

    let listener = TcpListener::bind(address)
        .await
        .map_err(RoutingErrors::TcpListenerCreationFailed)?;

    axum::serve(listener, app)
        .await
        .map_err(RoutingErrors::AxumInitializationFailed)?;
    Ok(())
}
//change get to others
/*
Rule of thumb:
GET → read
POST → create
PUT/PATCH → update
DELETE → delete
*/
pub fn set_up_routes() -> Router<Arc<Database>> {
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
        .route("/reading_progress/:id", patch(ping_server))
        .route("/book_metadata/:id", get(ping_server))
        .route("/login", get(ping_server))
    //register
    //logout
    //scan single directory
    //get covers
    //search
}
