use std::net::SocketAddr;
use tokio::net::TcpListener;
use axum::{routing::get, Json, Router};


fn get_base_address() -> &str{
    let port = env::var("PORT").unwrap_or_else(|_| "3000".to_string());
    let address = format!("0.0.0.0:{}",port).parse().unwrap();
    return address;
}

#[tokio::main]
async fn api_handler(){
    let address = get_base_address();
    let listener = TcpListener::bind(address)
        .await
        .unwrap();

    axum::serve::bind(listener, app)
        .await
        .unwrap();

}