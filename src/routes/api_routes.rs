use axum::{
    routing::{delete, get, patch, post, put},
    Router,
};
use tokio::net::TcpListener;
use tracing::info;

use crate::{
    app_state::AppState,
    config::get_port,
    error_types::RoutingErrors,
    routes::api_caller::{
        account::change_password,
        authentication::{
            confirm_password_reset, log_out, refresh_session, request_password_reset,
            resend_verification_code, sign_in, sign_up, verify_email,
        },
        helpers::{method_not_allowed, not_found, not_implemented, ping_server},
        library::{
            delete_library, get_libraries, get_series_children, get_series_in_library,
            request_file, retrieve_metadata, scan_all_directories,
        },
        totp::{
            disable_totp, finalize_totp_set_up, renew_totp_recovery_codes, sign_in_totp,
            sign_in_totp_with_recovery_code, totp_start_set_up, totp_status,
        },
        users::{list_users, update_user},
    },
};

/// Binds to `0.0.0.0:$PORT` and serves the API until the process stops.
pub async fn start_server(state: AppState) -> Result<(), RoutingErrors> {
    let app = set_up_routes().with_state(state);

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
/// Routes mapped to `not_implemented` are placeholders whose handlers don't exist yet.
pub fn set_up_routes() -> Router<AppState> {
    let api = Router::new()
        .route("/libraries", get(get_libraries).post(not_implemented))
        .route(
            "/libraries/{id}",
            delete(delete_library).patch(not_implemented),
        )
        .route("/libraries/{id}/series", get(get_series_in_library))
        .route("/series/{id}/books", get(get_series_children))
        .route("/books/{id}", get(retrieve_metadata))
        .route("/books/{id}/file", get(request_file))
        .route(
            "/books/{id}/progress",
            get(not_implemented).patch(not_implemented),
        )
        .route("/covers/{kind}/{id}", get(not_implemented))
        .route("/scan", post(scan_all_directories))
        .route("/scan/status", get(not_implemented))
        .route("/directories/{id}/scan", post(not_implemented))
        .route("/metadata/scan", post(not_implemented))
        .route("/search", get(not_implemented))
        .route("/auth/register", post(sign_up))
        .route("/auth/verify_email", post(verify_email))
        .route("/auth/verify_email/resend", post(resend_verification_code))
        .route("/auth/login", post(sign_in))
        .route("/auth/login/totp", post(sign_in_totp))
        .route(
            "/auth/login/recovery_code",
            post(sign_in_totp_with_recovery_code),
        )
        .route("/auth/logout", post(log_out))
        .route("/auth/refresh", post(refresh_session))
        .route("/auth/password_reset", post(request_password_reset))
        .route("/auth/password_reset/confirm", post(confirm_password_reset))
        .route("/account/password", put(change_password))
        .route("/account/totp", get(totp_status).post(totp_start_set_up))
        .route("/account/totp/confirm", post(finalize_totp_set_up))
        .route("/account/totp/disable", post(disable_totp))
        .route(
            "/account/totp/recovery_codes",
            post(renew_totp_recovery_codes),
        )
        .route("/users", get(list_users))
        .route("/users/{id}", patch(update_user));

    Router::new()
        .route("/health", get(ping_server))
        .nest("/api/v1", api)
        .fallback(not_found)
        .method_not_allowed_fallback(method_not_allowed)
}

#[cfg(test)]
mod tests {
    use axum::{
        body::{to_bytes, Body},
        http::{Request, StatusCode},
    };
    use tower::ServiceExt;

    use super::set_up_routes;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    use axum::{http::HeaderMap, Router};

    use crate::{
        routes::{
            auth::{hash_secret, hash_token},
            email_helper::Mailer,
        },
        test_support::{code_in, insert_user, TempDatabase, TEST_SETUP_CODE},
    };

    #[tokio::test]
    async fn health_answers_pong_through_the_router() {
        let temp = TempDatabase::open();
        let app = set_up_routes().with_state(temp.state(Mailer::Disabled));

        let response = app
            .oneshot(Request::get("/health").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), 1024).await.unwrap();
        assert_eq!(&body[..], b"pong");
    }

    async fn json_answer(request: Request<Body>) -> (StatusCode, serde_json::Value) {
        let temp = TempDatabase::open();
        let app = set_up_routes().with_state(temp.state(Mailer::Disabled));
        let response = app.oneshot(request).await.unwrap();
        let status = response.status();
        let body = to_bytes(response.into_body(), 1024).await.unwrap();
        (status, serde_json::from_slice(&body).unwrap())
    }

    #[tokio::test]
    async fn a_guarded_route_without_a_session_answers_401_json() {
        let (status, body) = json_answer(guarded(None)).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(body, serde_json::json!({ "error": "unauthorized" }));
    }

    #[tokio::test]
    async fn a_missing_book_answers_404_json() {
        let temp = TempDatabase::open();
        let member = logged_in(&temp, "bob", "member");
        let app = set_up_routes().with_state(temp.state(Mailer::Disabled));

        let (status, _, body) =
            call(&app, authorized("GET", "/api/v1/books/999", &member, None)).await;

        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(json(&body), serde_json::json!({ "error": "not_found" }));
    }

    fn bare(method: &str, uri: &str) -> Request<Body> {
        Request::builder()
            .method(method)
            .uri(uri)
            .body(Body::empty())
            .unwrap()
    }

    /// A new user with this role (`"admin"` or `"member"`) and a live session; answers the
    /// session token.
    fn logged_in(temp: &TempDatabase, username: &str, role: &str) -> String {
        let user = temp
            .db
            .setup_new_transaction(|conn| {
                let user = insert_user(conn, username);
                conn.execute(
                    "UPDATE users SET role = ?1 WHERE id = ?2",
                    rusqlite::params![role, user],
                )
                .unwrap();
                Ok(user)
            })
            .unwrap();
        let token = format!("{username}-session");
        add_session(
            temp,
            user,
            "device",
            &token,
            &format!("{username}-refresh"),
            unix_now() + 600,
        );
        token
    }

    #[tokio::test]
    async fn every_route_answers_on_its_new_path_and_the_old_paths_are_gone() {
        let temp = TempDatabase::open();
        let member = logged_in(&temp, "bob", "member");
        let admin = logged_in(&temp, "alice", "admin");
        let app = set_up_routes().with_state(temp.state(Mailer::Disabled));

        for uri in [
            "/api/v1/libraries",
            "/api/v1/libraries/1/series",
            "/api/v1/series/1/books",
        ] {
            let (status, _, body) = call(&app, authorized("GET", uri, &member, None)).await;
            assert_eq!(status, StatusCode::OK, "{uri}");
            assert_eq!(json(&body), serde_json::json!([]), "{uri}");
        }
        for uri in ["/api/v1/books/1", "/api/v1/books/1/file"] {
            let (status, _, body) = call(&app, authorized("GET", uri, &member, None)).await;
            assert_eq!(status, StatusCode::NOT_FOUND, "{uri}");
            assert_eq!(
                json(&body),
                serde_json::json!({ "error": "not_found" }),
                "{uri}"
            );
        }

        let library = temp
            .db
            .setup_new_transaction(|conn| {
                conn.execute(
                    "INSERT INTO library (library_name, library_type) VALUES ('books', 'books')",
                    [],
                )
                .unwrap();
                Ok(conn.last_insert_rowid())
            })
            .unwrap();
        let uri = format!("/api/v1/libraries/{library}");
        let (status, _, _) = call(&app, authorized("DELETE", &uri, &admin, None)).await;
        assert_eq!(status, StatusCode::OK);

        // A GET proves the path exists without starting a scan of the real BOOK_DIRS.
        let (status, _, _) = call(&app, bare("GET", "/api/v1/scan")).await;
        assert_eq!(status, StatusCode::METHOD_NOT_ALLOWED);

        for (method, uri) in [
            ("GET", "/api/v1/get_libraries"),
            ("GET", "/api/v1/get_series_from_library/1"),
            ("GET", "/api/v1/get_series_children/1"),
            ("GET", "/api/v1/book_metadata/1"),
            ("GET", "/api/v1/book/1"),
            ("POST", "/api/v1/scan_all_directories"),
            ("POST", "/api/v1/setup_db"),
            ("DELETE", "/api/v1/delete_library/1"),
            ("PUT", "/api/v1/update_entry"),
            ("POST", "/api/v1/insert_entry"),
            ("POST", "/api/v1/scan_metadata"),
            ("GET", "/api/v1/reading_progress/1"),
            ("POST", "/api/v1/scan_directory/1"),
            ("GET", "/api/v1/get_cover/book/1"),
        ] {
            let (status, _, _) = call(&app, bare(method, uri)).await;
            assert_eq!(status, StatusCode::NOT_FOUND, "{method} {uri}");
        }
    }

    #[tokio::test]
    async fn every_unbuilt_route_answers_501_json() {
        let temp = TempDatabase::open();
        let member = logged_in(&temp, "bob", "member");
        let app = set_up_routes().with_state(temp.state(Mailer::Disabled));

        for (method, uri) in [
            ("POST", "/api/v1/libraries"),
            ("PATCH", "/api/v1/libraries/1"),
            ("GET", "/api/v1/books/1/progress"),
            ("PATCH", "/api/v1/books/1/progress"),
            ("GET", "/api/v1/covers/book/1"),
            ("GET", "/api/v1/scan/status"),
            ("POST", "/api/v1/directories/1/scan"),
            ("POST", "/api/v1/metadata/scan"),
            ("GET", "/api/v1/search"),
        ] {
            let (status, _, body) = call(&app, authorized(method, uri, &member, None)).await;
            assert_eq!(status, StatusCode::NOT_IMPLEMENTED, "{method} {uri}");
            assert_eq!(
                json(&body),
                serde_json::json!({ "error": "not_implemented" }),
                "{method} {uri}"
            );
        }
    }

    #[tokio::test]
    async fn an_unknown_path_answers_404_json_and_a_wrong_method_405_json() {
        let temp = TempDatabase::open();
        let app = set_up_routes().with_state(temp.state(Mailer::Disabled));

        for uri in ["/nope", "/api/v1/nope", "/api/v1/books/1/nope"] {
            let (status, _, body) = call(&app, bare("GET", uri)).await;
            assert_eq!(status, StatusCode::NOT_FOUND, "{uri}");
            assert_eq!(
                json(&body),
                serde_json::json!({ "error": "not_found" }),
                "{uri}"
            );
        }

        for (method, uri) in [
            ("GET", "/api/v1/scan"),
            ("DELETE", "/health"),
            ("PUT", "/api/v1/auth/login"),
        ] {
            let (status, headers, body) = call(&app, bare(method, uri)).await;
            assert_eq!(status, StatusCode::METHOD_NOT_ALLOWED, "{method} {uri}");
            assert_eq!(
                json(&body),
                serde_json::json!({ "error": "method_not_allowed" }),
                "{method} {uri}"
            );
            assert!(
                headers.contains_key("allow"),
                "a 405 names the allowed methods: {method} {uri}"
            );
        }
    }

    #[tokio::test]
    async fn the_library_and_series_lists_load_what_the_scanner_wrote() {
        let temp = TempDatabase::open();
        let (library, series) = temp
            .db
            .setup_new_transaction(|conn| {
                // Like the scanner: no covers, descriptions or release years.
                conn.execute(
                    "INSERT INTO library (library_name, library_type) VALUES ('/books', 'books')",
                    [],
                )
                .unwrap();
                let library = conn.last_insert_rowid();
                conn.execute("INSERT INTO series (name) VALUES ('the long road')", [])
                    .unwrap();
                let series = conn.last_insert_rowid();
                conn.execute(
                    "INSERT INTO library_elements (library_id, series_id) VALUES (?1, ?2)",
                    [library, series],
                )
                .unwrap();
                Ok((library, series))
            })
            .unwrap();
        let member = logged_in(&temp, "bob", "member");
        let app = set_up_routes().with_state(temp.state(Mailer::Disabled));

        let (status, _, body) =
            call(&app, authorized("GET", "/api/v1/libraries", &member, None)).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            json(&body),
            serde_json::json!([{
                "id": library,
                "name": "/books",
                "library_type": "Books",
                "cover_image": null,
                "description": null,
            }])
        );

        let uri = format!("/api/v1/libraries/{library}/series");
        let (status, _, body) = call(&app, authorized("GET", &uri, &member, None)).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            json(&body),
            serde_json::json!([{
                "id": series,
                "name": "the long road",
                "description": null,
                "cover_image": null,
                "start_release_year": null,
                "end_release_year": null,
            }])
        );
    }

    #[tokio::test]
    async fn the_books_of_a_series_load_what_the_scanner_wrote() {
        let temp = TempDatabase::open();
        let (series, book) = temp
            .db
            .setup_new_transaction(|conn| {
                conn.execute("INSERT INTO series (name) VALUES ('the long road')", [])
                    .unwrap();
                let series = conn.last_insert_rowid();
                // Like the scanner: no tags, and a file time it couldn't read.
                conn.execute(
                    "INSERT INTO books (title, author, format, language, tags, file_path, \
                     volume_number, series) \
                     VALUES ('The Long Road', 'Unknown Author', 'cbz', 'unknown', '', \
                     '/books/The Long Road v01.cbz', 1, ?1)",
                    [series],
                )
                .unwrap();
                Ok((series, conn.last_insert_rowid()))
            })
            .unwrap();
        let member = logged_in(&temp, "bob", "member");
        let app = set_up_routes().with_state(temp.state(Mailer::Disabled));

        let uri = format!("/api/v1/series/{series}/books");
        let (status, _, body) = call(&app, authorized("GET", &uri, &member, None)).await;
        assert_eq!(status, StatusCode::OK);
        let books = json(&body);
        assert_eq!(books.as_array().unwrap().len(), 1);
        assert_eq!(books[0]["id"], book);
        assert_eq!(books[0]["title"], "The Long Road");
        assert_eq!(books[0]["volume_number"], 1);
        assert_eq!(books[0]["tags"], serde_json::json!([]));
        assert_eq!(books[0]["last_modified"], serde_json::Value::Null);
    }

    #[tokio::test]
    async fn a_body_that_cannot_be_read_answers_400_json() {
        let temp = TempDatabase::open();
        let app = set_up_routes().with_state(temp.state(Mailer::Disabled));
        let login = |content_type: Option<&str>, body: &str| {
            let mut request = Request::post("/api/v1/auth/login");
            if let Some(content_type) = content_type {
                request = request.header("content-type", content_type);
            }
            request.body(Body::from(body.to_string())).unwrap()
        };
        let json_type = Some("application/json");

        for (label, request) in [
            (
                "malformed JSON",
                login(json_type, r#"{"username_or_email":"#),
            ),
            ("an empty body", login(json_type, "")),
            (
                "a missing field",
                login(json_type, r#"{"username_or_email":"alice"}"#),
            ),
            (
                "a wrong field type",
                login(
                    json_type,
                    r#"{"username_or_email":"alice","password":123456,"device_name":"phone","platform":"ios"}"#,
                ),
            ),
            (
                "no content type",
                login(
                    None,
                    r#"{"username_or_email":"alice","password":"correct horse battery","device_name":"phone","platform":"ios"}"#,
                ),
            ),
        ] {
            let (status, _, body) = call(&app, request).await;
            assert_eq!(status, StatusCode::BAD_REQUEST, "{label}");
            // Exactly the key: the parse error, which can quote the sent value, stays out.
            assert_eq!(
                json(&body),
                serde_json::json!({ "error": "invalid_request" }),
                "{label}"
            );
        }
    }

    #[tokio::test]
    async fn a_guarded_route_reads_its_body_only_after_the_session() {
        let temp = TempDatabase::open();
        let user = temp
            .db
            .setup_new_transaction(|conn| Ok(insert_user(conn, "alice")))
            .unwrap();
        add_session(
            &temp,
            user,
            "d1",
            "session-1",
            "refresh-1",
            unix_now() + 600,
        );
        let app = set_up_routes().with_state(temp.state(Mailer::Disabled));
        let broken_body = |token: Option<&str>| {
            let mut request = Request::post("/api/v1/account/totp/confirm")
                .header("content-type", "application/json");
            if let Some(token) = token {
                request = request.header("authorization", format!("Bearer {token}"));
            }
            request.body(Body::from(r#"{"code":"#)).unwrap()
        };

        let (status, _, _) = call(&app, broken_body(None)).await;
        assert_eq!(
            status,
            StatusCode::UNAUTHORIZED,
            "no session, no body reading"
        );

        let (status, _, body) = call(&app, broken_body(Some("session-1"))).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(
            json(&body),
            serde_json::json!({ "error": "invalid_request" })
        );
    }

    #[tokio::test]
    async fn an_id_that_is_not_a_number_answers_400_json() {
        let temp = TempDatabase::open();
        let admin = logged_in(&temp, "alice", "admin");
        let app = set_up_routes().with_state(temp.state(Mailer::Disabled));

        for (method, uri) in [
            ("GET", "/api/v1/books/abc"),
            ("GET", "/api/v1/books/abc/file"),
            ("GET", "/api/v1/libraries/abc/series"),
            ("GET", "/api/v1/series/abc/books"),
            ("DELETE", "/api/v1/libraries/abc"),
            ("GET", "/api/v1/books/99999999999999999999"),
        ] {
            let (status, _, body) = call(&app, authorized(method, uri, &admin, None)).await;
            assert_eq!(status, StatusCode::BAD_REQUEST, "{method} {uri}");
            assert_eq!(
                json(&body),
                serde_json::json!({ "error": "invalid_request" }),
                "{method} {uri}"
            );
        }
    }

    fn post_json(uri: &str, body: serde_json::Value) -> Request<Body> {
        Request::post(uri)
            .header("content-type", "application/json")
            .body(Body::from(body.to_string()))
            .unwrap()
    }

    #[tokio::test]
    async fn a_reset_request_without_email_configured_answers_503() {
        let (status, body) = json_answer(post_json(
            "/api/v1/auth/password_reset",
            serde_json::json!({ "email": "alice@example.com" }),
        ))
        .await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(body, serde_json::json!({ "error": "email_not_configured" }));
    }

    #[tokio::test]
    async fn known_and_unknown_addresses_get_the_same_answer_and_only_one_mail_goes_out() {
        let temp = TempDatabase::open();
        temp.db
            .setup_new_transaction(|conn| Ok(insert_user(conn, "alice")))
            .unwrap();
        let (mailer, sent) = Mailer::capture();
        let app = set_up_routes().with_state(temp.state(mailer));

        let mut answers = Vec::new();
        for email in ["alice@example.com", "nobody@example.com"] {
            let response = app
                .clone()
                .oneshot(post_json(
                    "/api/v1/auth/password_reset",
                    serde_json::json!({ "email": email }),
                ))
                .await
                .unwrap();
            let status = response.status();
            let body = to_bytes(response.into_body(), 1024).await.unwrap();
            answers.push((status, body));
        }
        assert_eq!(answers[0].0, StatusCode::ACCEPTED);
        assert_eq!(answers[0], answers[1], "the two answers must be identical");

        // The mail goes out in a background task; give it up to two seconds.
        for _ in 0..200 {
            if !sent.lock().unwrap().is_empty() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        let sent = sent.lock().unwrap();
        assert_eq!(sent.len(), 1);
        assert_eq!(sent[0].email, "alice@example.com");
    }

    #[tokio::test]
    async fn too_many_logins_answer_429_with_retry_after() {
        let temp = TempDatabase::open();
        let app = set_up_routes().with_state(temp.state(Mailer::Disabled));
        let wrong_login = || {
            post_json(
                "/api/v1/auth/login",
                serde_json::json!({
                    "username_or_email": "nobody",
                    "password": "wrong password 123",
                    "device_name": "phone",
                    "platform": "ios",
                }),
            )
        };
        for _ in 0..10 {
            let (status, _, _) = call(&app, wrong_login()).await;
            assert_eq!(status, StatusCode::UNAUTHORIZED);
        }

        let (status, headers, body) = call(&app, wrong_login()).await;

        assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(
            json(&body),
            serde_json::json!({ "error": "too_many_attempts" })
        );
        let seconds: u64 = headers["retry-after"].to_str().unwrap().parse().unwrap();
        assert!(
            (890..=900).contains(&seconds),
            "about the 15-minute window: {seconds}"
        );
    }

    #[tokio::test]
    async fn a_reset_with_a_bad_code_answers_400_json() {
        let (status, body) = json_answer(post_json(
            "/api/v1/auth/password_reset/confirm",
            serde_json::json!({
                "email": "nobody@example.com",
                "code": "ABCD2345",
                "new_password": "new password 456",
                "new_password_repeat": "new password 456",
            }),
        ))
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(
            body,
            serde_json::json!({ "error": "invalid_or_expired_code" })
        );
    }

    fn unix_now() -> i64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64
    }

    fn add_session(
        temp: &TempDatabase,
        user_id: i64,
        device: &str,
        session_token: &str,
        refresh_token: &str,
        session_expires_at: i64,
    ) {
        let device = device.to_string();
        let session_hash = hash_token(session_token);
        let refresh_hash = hash_token(refresh_token);
        temp.db
            .setup_new_transaction(move |conn| {
                conn.execute(
                    "INSERT INTO sessions (user_id, device_id, device_name, platform, \
                     refresh_token_hashed, refresh_token_valid_until, session_token_hashed, \
                     session_token_valid_until, last_used_at) \
                     VALUES (?1, ?2, 'phone', 'ios', ?3, ?4, ?5, ?6, 0)",
                    rusqlite::params![
                        user_id,
                        device,
                        refresh_hash,
                        session_expires_at + 3600,
                        session_hash,
                        session_expires_at
                    ],
                )
                .unwrap();
                Ok(())
            })
            .unwrap();
    }

    async fn call(app: &Router, request: Request<Body>) -> (StatusCode, HeaderMap, Vec<u8>) {
        let response = app.clone().oneshot(request).await.unwrap();
        let status = response.status();
        let headers = response.headers().clone();
        let body = to_bytes(response.into_body(), 4096).await.unwrap().to_vec();
        (status, headers, body)
    }

    /// A request to a route behind `AuthSession` that answers 200 for any live session.
    fn guarded(authorization: Option<&str>) -> Request<Body> {
        let mut request = Request::get("/api/v1/account/totp");
        if let Some(value) = authorization {
            request = request.header("authorization", value);
        }
        request.body(Body::empty()).unwrap()
    }

    fn post_with_bearer(uri: &str, token: &str) -> Request<Body> {
        Request::post(uri)
            .header("authorization", format!("Bearer {token}"))
            .body(Body::empty())
            .unwrap()
    }

    #[tokio::test]
    async fn logging_in_over_http_returns_tokens_that_open_guarded_routes() {
        let temp = TempDatabase::open();
        let hash = hash_secret("correct horse battery".to_string())
            .await
            .unwrap();
        temp.db
            .setup_new_transaction(move |conn| {
                conn.execute(
                    "INSERT INTO users (username, password_hash, email, email_verified) \
                     VALUES ('alice', ?1, 'alice@example.com', 1)",
                    [hash],
                )
                .unwrap();
                Ok(())
            })
            .unwrap();
        let app = set_up_routes().with_state(temp.state(Mailer::Disabled));

        let (status, headers, body) = call(
            &app,
            post_json(
                "/api/v1/auth/login",
                serde_json::json!({
                    "username_or_email": "alice",
                    "password": "correct horse battery",
                    "device_name": "phone",
                    "platform": "ios",
                }),
            ),
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(headers.get("cache-control").unwrap(), "no-store");
        let body: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(body["status"], "ok");
        assert!(body["session_expires_at"].is_i64());
        assert!(body["refresh_token"].is_string());
        assert!(body["device_id"].is_string());

        let session_token = body["session_token"].as_str().unwrap();
        let (status, _, _) = call(&app, guarded(Some(&format!("Bearer {session_token}")))).await;
        assert_eq!(status, StatusCode::OK);
    }

    #[tokio::test]
    async fn a_guarded_route_accepts_only_a_live_bearer_session_token() {
        let temp = TempDatabase::open();
        let user = temp
            .db
            .setup_new_transaction(|conn| Ok(insert_user(conn, "alice")))
            .unwrap();
        add_session(
            &temp,
            user,
            "d1",
            "live-token",
            "live-refresh",
            unix_now() + 600,
        );
        add_session(
            &temp,
            user,
            "d2",
            "old-token",
            "old-refresh",
            unix_now() - 1,
        );
        let app = set_up_routes().with_state(temp.state(Mailer::Disabled));

        for accepted in ["Bearer live-token", "bearer live-token"] {
            let (status, _, _) = call(&app, guarded(Some(accepted))).await;
            assert_eq!(status, StatusCode::OK, "{accepted}");
        }

        let mut refusals = Vec::new();
        for refused in [
            None,
            Some("Basic live-token"),
            Some("Bearer "),
            Some("Bearer unknown"),
            Some("Bearer old-token"),
            Some("Bearer live-refresh"),
        ] {
            let (status, _, body) = call(&app, guarded(refused)).await;
            assert_eq!(status, StatusCode::UNAUTHORIZED, "{refused:?}");
            refusals.push(body);
        }
        assert!(
            refusals.windows(2).all(|pair| pair[0] == pair[1]),
            "every refusal must look the same"
        );
    }

    #[tokio::test]
    async fn refresh_and_logout_over_http() {
        let temp = TempDatabase::open();
        let user = temp
            .db
            .setup_new_transaction(|conn| Ok(insert_user(conn, "alice")))
            .unwrap();
        add_session(
            &temp,
            user,
            "d1",
            "session-1",
            "refresh-1",
            unix_now() + 600,
        );
        let app = set_up_routes().with_state(temp.state(Mailer::Disabled));

        let (status, _, _) =
            call(&app, post_with_bearer("/api/v1/auth/refresh", "session-1")).await;
        assert_eq!(
            status,
            StatusCode::UNAUTHORIZED,
            "a session token is not a refresh token"
        );

        let (status, headers, body) =
            call(&app, post_with_bearer("/api/v1/auth/refresh", "refresh-1")).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(headers.get("cache-control").unwrap(), "no-store");
        let body: serde_json::Value = serde_json::from_slice(&body).unwrap();
        let new_session = body["session_token"].as_str().unwrap().to_string();
        let new_refresh = body["refresh_token"].as_str().unwrap().to_string();
        assert_eq!(body["device_id"], "d1");

        let (status, _, _) = call(&app, guarded(Some(&format!("Bearer {new_session}")))).await;
        assert_eq!(status, StatusCode::OK);

        let (status, _, _) =
            call(&app, post_with_bearer("/api/v1/auth/logout", &new_refresh)).await;
        assert_eq!(status, StatusCode::NO_CONTENT);
        let (status, _, _) = call(&app, guarded(Some(&format!("Bearer {new_session}")))).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "logged out");

        let without_token = Request::post("/api/v1/auth/logout")
            .body(Body::empty())
            .unwrap();
        let (status, _, _) = call(&app, without_token).await;
        assert_eq!(status, StatusCode::NO_CONTENT);
    }

    /// A sign-up body that carries the setup code; only the first account needs it.
    fn sign_up_body(username: &str, email: &str) -> serde_json::Value {
        serde_json::json!({
            "email": email,
            "username": username,
            "password": "correct horse battery",
            "password_repeat": "correct horse battery",
            "device_name": "phone",
            "platform": "ios",
            "setup_code": TEST_SETUP_CODE,
        })
    }

    fn json(body: &[u8]) -> serde_json::Value {
        serde_json::from_slice(body).unwrap()
    }

    #[tokio::test]
    async fn a_first_sign_up_without_the_setup_code_answers_403_json() {
        let mut body = sign_up_body("alice", "alice@example.com");
        body.as_object_mut().unwrap().remove("setup_code");

        let (status, body) = json_answer(post_json("/api/v1/auth/register", body)).await;

        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(body, serde_json::json!({ "error": "invalid_setup_code" }));
    }

    #[tokio::test]
    async fn signing_up_and_verifying_over_http() {
        let temp = TempDatabase::open();
        let (mailer, sent) = Mailer::capture();
        let app = set_up_routes().with_state(temp.state(mailer));
        let register = |username: &str| {
            post_json(
                "/api/v1/auth/register",
                sign_up_body(username, &format!("{username}@example.com")),
            )
        };

        let (status, headers, body) = call(&app, register("alice")).await;
        assert_eq!(
            status,
            StatusCode::OK,
            "the first account is verified on the spot"
        );
        assert_eq!(headers.get("cache-control").unwrap(), "no-store");
        assert_eq!(json(&body)["status"], "ok");

        let (status, _, body) = call(&app, register("bob")).await;
        assert_eq!(status, StatusCode::ACCEPTED);
        assert_eq!(
            json(&body),
            serde_json::json!({ "verification_required": true })
        );
        let code = code_in(&sent.lock().unwrap()[0]);

        let (status, headers, body) = call(
            &app,
            post_json(
                "/api/v1/auth/verify_email",
                serde_json::json!({
                    "username_or_email": "bob",
                    "password": "correct horse battery",
                    "code": code,
                    "device_name": "phone",
                    "platform": "ios",
                }),
            ),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(headers.get("cache-control").unwrap(), "no-store");
        let session_token = json(&body)["session_token"].as_str().unwrap().to_string();
        let (status, _, _) = call(&app, guarded(Some(&format!("Bearer {session_token}")))).await;
        assert_eq!(status, StatusCode::OK);
    }

    #[tokio::test]
    async fn a_role_or_verified_flag_in_a_sign_up_body_is_ignored() {
        let temp = TempDatabase::open();
        temp.db
            .setup_new_transaction(|conn| Ok(insert_user(conn, "alice")))
            .unwrap();
        let (mailer, _sent) = Mailer::capture();
        let app = set_up_routes().with_state(temp.state(mailer));
        let mut body = sign_up_body("bob", "bob@example.com");
        body["role"] = "admin".into();
        body["email_verified"] = true.into();

        let (status, _, _) = call(&app, post_json("/api/v1/auth/register", body)).await;

        assert_eq!(status, StatusCode::ACCEPTED);
        let stored: (String, bool) = temp
            .db
            .setup_new_transaction(|conn| {
                Ok(conn
                    .query_row(
                        "SELECT role, email_verified FROM users WHERE username = 'bob'",
                        [],
                        |row| Ok((row.get(0)?, row.get(1)?)),
                    )
                    .unwrap())
            })
            .unwrap();
        assert_eq!(stored, ("member".to_string(), false));
    }

    #[tokio::test]
    async fn sign_up_errors_answer_json() {
        let temp = TempDatabase::open();
        temp.db
            .setup_new_transaction(|conn| Ok(insert_user(conn, "alice")))
            .unwrap();
        let app = set_up_routes().with_state(temp.state(Mailer::Disabled));

        let cases = [
            (
                sign_up_body("bob", "ALICE@example.com"),
                StatusCode::CONFLICT,
                "email_taken",
            ),
            (
                sign_up_body("Alice", "bob@example.com"),
                StatusCode::CONFLICT,
                "username_taken",
            ),
            (
                sign_up_body("bob", "bob@example.com\r\nDATA"),
                StatusCode::BAD_REQUEST,
                "invalid_email",
            ),
            (
                sign_up_body("bo", "bob@example.com"),
                StatusCode::BAD_REQUEST,
                "invalid_username",
            ),
        ];
        for (body, expected_status, expected_key) in cases {
            let (status, _, body) = call(&app, post_json("/api/v1/auth/register", body)).await;
            assert_eq!(status, expected_status, "{expected_key}");
            assert_eq!(json(&body), serde_json::json!({ "error": expected_key }));
        }
    }

    #[tokio::test]
    async fn a_resend_answers_the_same_for_every_name_and_mails_only_an_unverified_account() {
        let temp = TempDatabase::open();
        temp.db
            .setup_new_transaction(|conn| Ok(insert_user(conn, "bob")))
            .unwrap();
        let (mailer, sent) = Mailer::capture();
        let app = set_up_routes().with_state(temp.state(mailer));

        let mut answers = Vec::new();
        for name in ["bob", "nobody"] {
            let (status, _, body) = call(
                &app,
                post_json(
                    "/api/v1/auth/verify_email/resend",
                    serde_json::json!({ "username_or_email": name }),
                ),
            )
            .await;
            answers.push((status, body));
        }
        assert_eq!(answers[0].0, StatusCode::ACCEPTED);
        assert_eq!(answers[0], answers[1], "the two answers must be identical");

        // The mail goes out in a background task; give it up to two seconds.
        for _ in 0..200 {
            if !sent.lock().unwrap().is_empty() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        let sent = sent.lock().unwrap();
        assert_eq!(sent.len(), 1);
        assert_eq!(sent[0].email, "bob@example.com");
    }

    /// A request with `Authorization: Bearer <token>` and, if given, a JSON body.
    fn authorized(
        method: &str,
        uri: &str,
        token: &str,
        body: Option<serde_json::Value>,
    ) -> Request<Body> {
        let request = Request::builder()
            .method(method)
            .uri(uri)
            .header("authorization", format!("Bearer {token}"));
        match body {
            Some(body) => request
                .header("content-type", "application/json")
                .body(Body::from(body.to_string())),
            None => request.body(Body::empty()),
        }
        .unwrap()
    }

    /// Every route except `/health` and the public `/api/v1/auth/…` routes. A route added to
    /// the table belongs here too, so this test proves it can't be reached without a login.
    const GUARDED_ROUTES: &[(&str, &str)] = &[
        ("GET", "/api/v1/libraries"),
        ("POST", "/api/v1/libraries"),
        ("PATCH", "/api/v1/libraries/1"),
        ("DELETE", "/api/v1/libraries/1"),
        ("GET", "/api/v1/libraries/1/series"),
        ("GET", "/api/v1/series/1/books"),
        ("GET", "/api/v1/books/1"),
        ("GET", "/api/v1/books/1/file"),
        ("GET", "/api/v1/books/1/progress"),
        ("PATCH", "/api/v1/books/1/progress"),
        ("GET", "/api/v1/covers/book/1"),
        ("POST", "/api/v1/scan"),
        ("GET", "/api/v1/scan/status"),
        ("POST", "/api/v1/directories/1/scan"),
        ("POST", "/api/v1/metadata/scan"),
        ("GET", "/api/v1/search"),
        ("PUT", "/api/v1/account/password"),
        ("GET", "/api/v1/account/totp"),
        ("POST", "/api/v1/account/totp"),
        ("POST", "/api/v1/account/totp/confirm"),
        ("POST", "/api/v1/account/totp/disable"),
        ("POST", "/api/v1/account/totp/recovery_codes"),
        ("GET", "/api/v1/users"),
        ("PATCH", "/api/v1/users/1"),
    ];

    #[tokio::test]
    async fn every_route_but_health_and_auth_needs_a_session() {
        let temp = TempDatabase::open();
        let user = temp
            .db
            .setup_new_transaction(|conn| Ok(insert_user(conn, "alice")))
            .unwrap();
        add_session(
            &temp,
            user,
            "d1",
            "expired",
            "expired-refresh",
            unix_now() - 1,
        );
        let app = set_up_routes().with_state(temp.state(Mailer::Disabled));

        for &(method, uri) in GUARDED_ROUTES {
            for token in [None, Some("expired")] {
                let request = match token {
                    Some(token) => authorized(method, uri, token, None),
                    None => bare(method, uri),
                };
                let (status, _, body) = call(&app, request).await;
                assert_eq!(status, StatusCode::UNAUTHORIZED, "{method} {uri} {token:?}");
                assert_eq!(
                    json(&body),
                    serde_json::json!({ "error": "unauthorized" }),
                    "{method} {uri}"
                );
            }
        }
    }

    #[tokio::test]
    async fn only_an_admin_scans_or_deletes_a_library() {
        let temp = TempDatabase::open();
        let member = logged_in(&temp, "bob", "member");
        let admin = logged_in(&temp, "alice", "admin");
        let library = temp
            .db
            .setup_new_transaction(|conn| {
                conn.execute(
                    "INSERT INTO library (library_name, library_type) VALUES ('books', 'books')",
                    [],
                )
                .unwrap();
                Ok(conn.last_insert_rowid())
            })
            .unwrap();
        let app = set_up_routes().with_state(temp.state(Mailer::Disabled));
        let delete = format!("/api/v1/libraries/{library}");
        let libraries = |temp: &TempDatabase| -> i64 {
            temp.db
                .setup_new_transaction(|conn| {
                    Ok(conn
                        .query_row("SELECT COUNT(*) FROM library", [], |row| row.get(0))
                        .unwrap())
                })
                .unwrap()
        };

        // Refused before the handler runs, so no scan of the real `BOOK_DIRS` starts.
        for (method, uri) in [("POST", "/api/v1/scan"), ("DELETE", delete.as_str())] {
            let (status, _, body) = call(&app, authorized(method, uri, &member, None)).await;
            assert_eq!(status, StatusCode::FORBIDDEN, "{method} {uri}");
            assert_eq!(json(&body), serde_json::json!({ "error": "forbidden" }));
        }
        assert_eq!(libraries(&temp), 1, "a member deletes nothing");

        let (status, _, _) = call(&app, authorized("DELETE", &delete, &admin, None)).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(libraries(&temp), 0);
    }

    #[tokio::test]
    async fn only_an_admin_manages_users_and_a_new_role_counts_at_once() {
        let temp = TempDatabase::open();
        let admin = logged_in(&temp, "alice", "admin");
        let member = logged_in(&temp, "bob", "member");
        let app = set_up_routes().with_state(temp.state(Mailer::Disabled));
        let list_as = |token: &str| authorized("GET", "/api/v1/users", token, None);
        let patch = |id: &str, body: serde_json::Value| {
            authorized("PATCH", &format!("/api/v1/users/{id}"), &admin, Some(body))
        };

        let (status, _, body) = call(&app, list_as(&member)).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(json(&body), serde_json::json!({ "error": "forbidden" }));

        let (status, _, body) = call(&app, list_as(&admin)).await;
        assert_eq!(status, StatusCode::OK);
        let text = String::from_utf8(body.clone()).unwrap();
        assert!(
            !text.contains("password") && !text.contains("not-a-hash"),
            "no password hash, not even its key: {text}"
        );
        let users = json(&body);
        let user = |name: &str| {
            users
                .as_array()
                .unwrap()
                .iter()
                .find(|user| user["username"] == name)
                .unwrap()
                .clone()
        };
        let (alice, bob) = (user("alice"), user("bob"));
        let mut keys: Vec<&String> = bob.as_object().unwrap().keys().collect();
        keys.sort();
        assert_eq!(
            keys,
            [
                "created_at",
                "email",
                "email_verified",
                "id",
                "last_login",
                "role",
                "username"
            ]
        );
        assert_eq!(bob["role"], "member");
        assert!(bob["created_at"].is_i64());
        let (alice, bob) = (alice["id"].to_string(), bob["id"].to_string());

        let (status, _, body) =
            call(&app, patch(&bob, serde_json::json!({ "role": "admin" }))).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(json(&body)["role"], "admin");
        let (status, _, _) = call(&app, list_as(&member)).await;
        assert_eq!(status, StatusCode::OK, "promoted on the very next request");

        let (status, _, body) =
            call(&app, patch(&bob, serde_json::json!({ "role": "member" }))).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(json(&body)["role"], "member");
        let (status, _, _) = call(&app, list_as(&member)).await;
        assert_eq!(
            status,
            StatusCode::FORBIDDEN,
            "demoted on the very next request"
        );

        let (status, _, body) =
            call(&app, patch(&alice, serde_json::json!({ "role": "member" }))).await;
        assert_eq!(status, StatusCode::CONFLICT);
        assert_eq!(json(&body), serde_json::json!({ "error": "last_admin" }));

        for (id, change, expected) in [
            (
                "abc",
                serde_json::json!({ "role": "admin" }),
                StatusCode::BAD_REQUEST,
            ),
            (
                bob.as_str(),
                serde_json::json!({ "role": "owner" }),
                StatusCode::BAD_REQUEST,
            ),
            (bob.as_str(), serde_json::json!({}), StatusCode::BAD_REQUEST),
            (
                bob.as_str(),
                serde_json::json!({ "email_verified": false }),
                StatusCode::BAD_REQUEST,
            ),
            (
                "999",
                serde_json::json!({ "role": "admin" }),
                StatusCode::NOT_FOUND,
            ),
        ] {
            let (status, _, _) = call(&app, patch(id, change.clone())).await;
            assert_eq!(status, expected, "{id} {change}");
        }
    }

    #[tokio::test]
    async fn the_code_steps_need_a_live_challenge() {
        let temp = TempDatabase::open();
        let user = temp
            .db
            .setup_new_transaction(|conn| Ok(insert_user(conn, "alice")))
            .unwrap();
        add_session(
            &temp,
            user,
            "d1",
            "session-1",
            "refresh-1",
            unix_now() + 600,
        );
        let app = set_up_routes().with_state(temp.state(Mailer::Disabled));
        let body = serde_json::json!({ "code": "123456" });

        for uri in [
            "/api/v1/auth/login/totp",
            "/api/v1/auth/login/recovery_code",
        ] {
            for token in [None, Some("unknown"), Some("session-1")] {
                let request = match token {
                    Some(token) => authorized("POST", uri, token, Some(body.clone())),
                    None => post_json(uri, body.clone()),
                };
                let (status, _, answer) = call(&app, request).await;
                assert_eq!(status, StatusCode::UNAUTHORIZED, "{uri} {token:?}");
                assert_eq!(
                    json(&answer),
                    serde_json::json!({ "error": "unauthorized" })
                );
            }
        }
    }

    async fn add_alice_with_password(temp: &TempDatabase) {
        let hash = hash_secret("correct horse battery".to_string())
            .await
            .unwrap();
        temp.db
            .setup_new_transaction(move |conn| {
                conn.execute(
                    "INSERT INTO users (username, password_hash, email, email_verified) \
                     VALUES ('alice', ?1, 'alice@example.com', 1)",
                    [hash],
                )
                .unwrap();
                Ok(())
            })
            .unwrap();
    }

    fn log_in_as_alice() -> Request<Body> {
        post_json(
            "/api/v1/auth/login",
            serde_json::json!({
                "username_or_email": "alice",
                "password": "correct horse battery",
                "device_name": "phone",
                "platform": "ios",
            }),
        )
    }

    /// Switches 2FA on over HTTP; answers the authenticator app and the recovery codes.
    async fn enroll_over_http(app: &Router, session: &str) -> (totp_rs::Totp, Vec<String>) {
        let (_, _, body) = call(
            app,
            authorized("POST", "/api/v1/account/totp", session, None),
        )
        .await;
        let authenticator = totp_rs::Builder::new()
            .with_secret(
                totp_rs::Secret::try_from_base32(json(&body)["secret_base32"].as_str().unwrap())
                    .unwrap(),
            )
            .build()
            .unwrap();
        let confirmation = serde_json::json!({
            "code": authenticator.generate_current().to_string(),
            "password": "correct horse battery",
        });
        let (status, _, body) = call(
            app,
            authorized(
                "POST",
                "/api/v1/account/totp/confirm",
                session,
                Some(confirmation),
            ),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let codes = serde_json::from_value(json(&body)["recovery_codes"].clone()).unwrap();
        (authenticator, codes)
    }

    /// Logs Alice in with her password; answers the challenge token of the code step.
    async fn challenge_token(app: &Router) -> String {
        let (status, _, body) = call(app, log_in_as_alice()).await;
        assert_eq!(status, StatusCode::OK);
        let body = json(&body);
        assert_eq!(body["status"], "totp_required");
        body["challenge_token"].as_str().unwrap().to_string()
    }

    #[tokio::test]
    async fn logging_in_with_2fa_and_turning_it_off_over_http() {
        let temp = TempDatabase::open();
        add_alice_with_password(&temp).await;
        let app = set_up_routes().with_state(temp.state(Mailer::Disabled));
        let (_, _, body) = call(&app, log_in_as_alice()).await;
        let setup_session = json(&body)["session_token"].as_str().unwrap().to_string();
        let (authenticator, recovery_codes) = enroll_over_http(&app, &setup_session).await;

        let challenge = challenge_token(&app).await;
        let (status, _, _) = call(&app, guarded(Some(&format!("Bearer {challenge}")))).await;
        assert_eq!(
            status,
            StatusCode::UNAUTHORIZED,
            "a challenge token is no session"
        );
        // The confirm above used the current time step, so this sends the next one; the check
        // accepts one step of clock drift.
        let next_step = authenticator.generate(unix_now() as u64 + 30).to_string();
        let (status, headers, body) = call(
            &app,
            authorized(
                "POST",
                "/api/v1/auth/login/totp",
                &challenge,
                Some(serde_json::json!({ "code": next_step })),
            ),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(headers.get("cache-control").unwrap(), "no-store");
        assert_eq!(json(&body)["status"], "ok");
        let session = json(&body)["session_token"].as_str().unwrap().to_string();
        let (status, _, _) = call(&app, guarded(Some(&format!("Bearer {session}")))).await;
        assert_eq!(status, StatusCode::OK);

        let challenge = challenge_token(&app).await;
        let (status, headers, body) = call(
            &app,
            authorized(
                "POST",
                "/api/v1/auth/login/recovery_code",
                &challenge,
                Some(serde_json::json!({ "code": recovery_codes[0] })),
            ),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(headers.get("cache-control").unwrap(), "no-store");
        assert_eq!(json(&body)["status"], "ok");

        let turn_off = serde_json::json!({
            "password": "correct horse battery",
            "code": recovery_codes[1],
        });
        let (status, _, _) = call(
            &app,
            authorized(
                "POST",
                "/api/v1/account/totp/disable",
                &session,
                Some(turn_off),
            ),
        )
        .await;
        assert_eq!(status, StatusCode::NO_CONTENT);
        let (status, _, _) = call(&app, guarded(Some(&format!("Bearer {session}")))).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "every session ended");
        let (_, _, body) = call(&app, log_in_as_alice()).await;
        assert_eq!(json(&body)["status"], "ok", "no code step any more");
    }

    #[tokio::test]
    async fn renewing_the_recovery_codes_over_http() {
        let temp = TempDatabase::open();
        add_alice_with_password(&temp).await;
        let app = set_up_routes().with_state(temp.state(Mailer::Disabled));
        let (_, _, body) = call(&app, log_in_as_alice()).await;
        let session = json(&body)["session_token"].as_str().unwrap().to_string();
        let (authenticator, old_codes) = enroll_over_http(&app, &session).await;

        // The confirm used the current time step, so this sends the next one.
        let renewal = serde_json::json!({
            "password": "correct horse battery",
            "code": authenticator.generate(unix_now() as u64 + 30).to_string(),
        });
        let (status, headers, body) = call(
            &app,
            authorized(
                "POST",
                "/api/v1/account/totp/recovery_codes",
                &session,
                Some(renewal),
            ),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(headers.get("cache-control").unwrap(), "no-store");
        let new_codes: Vec<String> =
            serde_json::from_value(json(&body)["recovery_codes"].clone()).unwrap();
        assert_eq!(new_codes.len(), 10);

        let challenge = challenge_token(&app).await;
        let with_code = |code: &str| {
            authorized(
                "POST",
                "/api/v1/auth/login/recovery_code",
                &challenge,
                Some(serde_json::json!({ "code": code })),
            )
        };
        let (status, _, body) = call(&app, with_code(&old_codes[0])).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "an old code");
        assert_eq!(
            json(&body),
            serde_json::json!({ "error": "invalid_or_expired_code" })
        );
        let (status, _, body) = call(&app, with_code(&new_codes[0])).await;
        assert_eq!(status, StatusCode::OK, "a new code");
        assert_eq!(json(&body)["status"], "ok");
    }

    #[tokio::test]
    async fn changing_the_password_over_http() {
        let temp = TempDatabase::open();
        add_alice_with_password(&temp).await;
        let app = set_up_routes().with_state(temp.state(Mailer::Disabled));
        let (_, _, body) = call(&app, log_in_as_alice()).await;
        let this_device = json(&body)["session_token"].as_str().unwrap().to_string();
        let (_, _, body) = call(&app, log_in_as_alice()).await;
        let other_device = json(&body)["session_token"].as_str().unwrap().to_string();

        let change = serde_json::json!({
            "current_password": "correct horse battery",
            "new_password": "battery staple horse",
            "new_password_repeat": "battery staple horse",
        });
        let (status, _, body) = call(
            &app,
            authorized(
                "PUT",
                "/api/v1/account/password",
                &this_device,
                Some(change),
            ),
        )
        .await;
        assert_eq!(status, StatusCode::NO_CONTENT);
        assert!(body.is_empty());

        let (status, _, _) = call(&app, guarded(Some(&format!("Bearer {this_device}")))).await;
        assert_eq!(status, StatusCode::OK, "this session stays");
        let (status, _, _) = call(&app, guarded(Some(&format!("Bearer {other_device}")))).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "the other device's ended");
        let (status, _, _) = call(&app, log_in_as_alice()).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "the old password");
        let with_the_new_password = post_json(
            "/api/v1/auth/login",
            serde_json::json!({
                "username_or_email": "alice",
                "password": "battery staple horse",
                "device_name": "phone",
                "platform": "ios",
            }),
        );
        let (status, _, _) = call(&app, with_the_new_password).await;
        assert_eq!(status, StatusCode::OK, "the new password");
    }

    #[tokio::test]
    async fn setting_up_2fa_over_http() {
        let temp = TempDatabase::open();
        add_alice_with_password(&temp).await;
        let app = set_up_routes().with_state(temp.state(Mailer::Disabled));
        let log_in = log_in_as_alice;
        let (_, _, body) = call(&app, log_in()).await;
        let session = json(&body)["session_token"].as_str().unwrap().to_string();
        let (_, _, body) = call(&app, log_in()).await;
        let other_device = json(&body)["session_token"].as_str().unwrap().to_string();
        let status_of_2fa = || authorized("GET", "/api/v1/account/totp", &session, None);

        let (status, _, body) = call(&app, status_of_2fa()).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            json(&body),
            serde_json::json!({ "enabled": false, "recovery_codes_left": 0 })
        );

        let (status, headers, body) = call(
            &app,
            authorized("POST", "/api/v1/account/totp", &session, None),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(headers.get("cache-control").unwrap(), "no-store");
        let setup = json(&body);
        assert!(setup["otpauth_url"]
            .as_str()
            .unwrap()
            .starts_with("otpauth://totp/Bookserver:alice?secret="));
        let authenticator = totp_rs::Builder::new()
            .with_secret(
                totp_rs::Secret::try_from_base32(setup["secret_base32"].as_str().unwrap()).unwrap(),
            )
            .build()
            .unwrap();

        let confirmation = serde_json::json!({
            "code": authenticator.generate_current().to_string(),
            "password": "correct horse battery",
        });
        let (status, headers, body) = call(
            &app,
            authorized(
                "POST",
                "/api/v1/account/totp/confirm",
                &session,
                Some(confirmation),
            ),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(headers.get("cache-control").unwrap(), "no-store");
        assert_eq!(json(&body)["recovery_codes"].as_array().unwrap().len(), 10);

        let (status, _, _) = call(&app, guarded(Some(&format!("Bearer {other_device}")))).await;
        assert_eq!(
            status,
            StatusCode::UNAUTHORIZED,
            "the other device's session ended"
        );
        let (_, _, body) = call(&app, status_of_2fa()).await;
        assert_eq!(
            json(&body),
            serde_json::json!({ "enabled": true, "recovery_codes_left": 10 })
        );
        let (status, _, body) = call(&app, log_in()).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(json(&body)["status"], "totp_required");
    }

    #[tokio::test]
    async fn a_resend_without_email_configured_answers_503() {
        let (status, body) = json_answer(post_json(
            "/api/v1/auth/verify_email/resend",
            serde_json::json!({ "username_or_email": "bob" }),
        ))
        .await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(body, serde_json::json!({ "error": "email_not_configured" }));
    }
}
