use std::sync::Arc;

use axum::{
    routing::{delete, get, patch, post, put},
    Router,
};
use tokio::net::TcpListener;
use tracing::info;

use crate::{
    config::get_port,
    db::Database,
    error_types::RoutingErrors,
    routes::api_caller::{
        helpers::ping_server,
        library::{
            delete_library, get_libraries, get_series_children, get_series_in_library,
            initiate_database, insert_database_entry, request_file, retrieve_metadata,
            scan_all_directories, scan_for_metadata, update_database_entry,
        },
    },
};

/// Binds to `0.0.0.0:$PORT` and serves the API until the process stops.
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

/// The route table. Everything except `/health` lives under `/api/v1`.
/// Routes mapped to `ping_server` are placeholders whose handlers don't exist yet.
pub fn set_up_routes() -> Router<Arc<Database>> {
    let api = Router::new()
        .route("/get_libraries", get(get_libraries))
        .route("/get_series_from_library/{id}", get(get_series_in_library))
        .route("/get_series_children/{id}", get(get_series_children))
        .route("/update_entry", put(update_database_entry))
        .route("/insert_entry", post(insert_database_entry))
        .route("/scan_metadata", post(scan_for_metadata))
        .route("/setup_db", post(initiate_database))
        .route("/scan_all_directories", post(scan_all_directories))
        .route("/book/{id}", get(request_file))
        .route("/delete_library/{id}", delete(delete_library))
        .route(
            "/reading_progress/{id}",
            patch(ping_server).get(ping_server),
        )
        .route("/book_metadata/{id}", get(retrieve_metadata))
        .route("/login", post(ping_server))
        .route("/register_user", post(ping_server))
        .route("/log_out", post(ping_server))
        .route("/scan_directory/{id}", post(ping_server))
        .route("/get_cover/{kind}/{id}", get(ping_server))
        .route("/scan/status", get(ping_server))
        .route("/search", get(ping_server));

    Router::new()
        .route("/health", get(ping_server))
        .nest("/api/v1", api)
}

#[cfg(test)]
mod tests {
    use axum::{
        body::{to_bytes, Body},
        http::{Request, StatusCode},
    };
    use tower::ServiceExt;

    use super::set_up_routes;
    use crate::test_support::TempDatabase;

    #[tokio::test]
    async fn health_answers_pong_through_the_router() {
        let temp = TempDatabase::open();
        let app = set_up_routes().with_state(temp.db.clone());

        let response = app
            .oneshot(Request::get("/health").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), 1024).await.unwrap();
        assert_eq!(&body[..], b"pong");
    }
}
