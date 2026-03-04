use axum::{routing::get, Json, Router};
use bookserver_backend::stream_reader::streaming_file;
use serde::Serialize;
use tracing::info;
use std::net::SocketAddr;
use tokio::net::TcpListener;
use bookserver_backend::db::Database;
use bookserver_backend::config::{get_port,get_books_dirs};
use bookserver_backend::folder_scanner::scan_all_folders;

//to delete
use axum::{
    body::Body,
    http::{Request, StatusCode},
    response::IntoResponse,
};
use tower::ServiceExt; // for `oneshot`
use tower_http::services::ServeFile;


#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let app = Router::new()
        .route("/health", get(get_health))
        .route("/info", get(info))
        .route("/setup_db", get(setup_db))
        .route("/scan_all_directories", get(initiate_folder_scanner))
        .route("/book",get(get_book))
        .route("/partial_book", get(get_partial_book));
        //.route("/scan", get(scan));

    let port = get_port();
    let address = format!("127.0.0.1:{}", port);

    info!("Starting server on {}!",address);

    let socket_address: SocketAddr = address.parse().unwrap();
    let listener = TcpListener::bind(socket_address)
    .await
    .unwrap();

    axum::serve(listener, app).await.unwrap();
}

async fn get_health() -> Json<HealthResponse> {
    let response: Json<HealthResponse> = health().await;
    response
}

async fn health() -> Json<HealthResponse> {
    tokio::time::sleep(std::time::Duration::from_secs(5)).await; // Simulate some work
    Json(HealthResponse { status: "ok" })

}

async fn info() -> Json<InfoResponse> {
    Json(InfoResponse {
        name: "Book Server",
        version: env!("CARGO_PKG_VERSION"),
    })
}

// async fn scan() -> Json<crate::models::ScanResult> {
//     let result = folder_scanner::initiate_folder_scanner();
//     Json(result)
// }


#[derive(Serialize)]
struct HealthResponse{
    status: &'static str,
}

#[derive(Serialize)]
struct InfoResponse{
    name: &'static str,
    version: &'static str,
}

async fn setup_db() -> &'static str {
    Database::new().unwrap();
    "Database setup complete!"
}

async fn initiate_folder_scanner() -> &'static str {
    scan_all_folders().await.unwrap();
    "Folder scanning initiated!"
}

async fn get_book(request: Request<Body>)-> impl IntoResponse{
    let service = ServeFile::new("../Downloads/pkpadmin,+529-2711-1-CE.pdf");
    let db = Database::new()?;
    let metadata = db.
    streaming_file(metadata, request);
    service
        .oneshot(request)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

async fn get_partial_book(){

}