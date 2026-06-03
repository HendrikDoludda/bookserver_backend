use anyhow::Context;
use bookserver_backend::db::Database;
use bookserver_backend::routes::api_routes::start_server;
use std::sync::Arc;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let db = Arc::new(Database::new().context("Failed to initialize the database")?);

    start_server(db)
        .await
        .context("Server stopped with an error")?;

    Ok(())
}
