use axum::{routing::get, Json, Router};
use serde::Serialize;
use tracing::info;
use std::net::SocketAddr;
use tokio::net::TcpListener;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let app = Router::new()
        .route("/health", get(health))
        .route("/info", get(info));

    let address = "127.0.0.1:3000";

    info!("Starting server on {}!",address);

    let socket_address: SocketAddr = address.parse().unwrap();
    let listener = TcpListener::bind(socket_address)
    .await
    .unwrap();

    axum::serve(listener, app).await.unwrap();
}

async fn health() -> Json<HealthResponse> {
    Json(HealthResponse { status: "ok" })
}

async fn info() -> Json<InfoResponse> {
    Json(InfoResponse {
        name: "Book Server",
        version: env!("CARGO_PKG_VERSION"),
    })
}


#[derive(Serialize)]
struct HealthResponse{
    status: &'static str,
}

#[derive(Serialize)]
struct InfoResponse{
    name: &'static str,
    version: &'static str,
}