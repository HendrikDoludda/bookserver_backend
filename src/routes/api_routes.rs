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
        authentication::{
            confirm_password_reset, log_out, refresh_session, request_password_reset,
            resend_verification_code, sign_in, sign_up, verify_email,
        },
        helpers::ping_server,
        library::{
            delete_library, get_libraries, get_series_children, get_series_in_library,
            initiate_database, insert_database_entry, request_file, retrieve_metadata,
            scan_all_directories, scan_for_metadata, update_database_entry,
        },
        totp::{finalize_totp_set_up, totp_start_set_up, totp_status},
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
/// Routes mapped to `ping_server` are placeholders whose handlers don't exist yet.
pub fn set_up_routes() -> Router<AppState> {
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
        .route("/scan_directory/{id}", post(ping_server))
        .route("/get_cover/{kind}/{id}", get(ping_server))
        .route("/scan/status", get(ping_server))
        .route("/search", get(ping_server))
        .route("/auth/register", post(sign_up))
        .route("/auth/verify_email", post(verify_email))
        .route("/auth/verify_email/resend", post(resend_verification_code))
        .route("/auth/login", post(sign_in))
        .route("/auth/logout", post(log_out))
        .route("/auth/refresh", post(refresh_session))
        .route("/auth/password_reset", post(request_password_reset))
        .route("/auth/password_reset/confirm", post(confirm_password_reset))
        .route("/account/totp", get(totp_status).post(totp_start_set_up))
        .route("/account/totp/confirm", post(finalize_totp_set_up));

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
        let (status, body) = json_answer(
            Request::put("/api/v1/update_entry")
                .body(Body::empty())
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(body, serde_json::json!({ "error": "unauthorized" }));
    }

    #[tokio::test]
    async fn a_missing_book_answers_404_json() {
        let (status, body) = json_answer(
            Request::get("/api/v1/book_metadata/999")
                .body(Body::empty())
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body, serde_json::json!({ "error": "not_found" }));
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

    fn guarded(authorization: Option<&str>) -> Request<Body> {
        let mut request = Request::put("/api/v1/update_entry");
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
    async fn admin_and_verified_flags_in_a_sign_up_body_are_ignored() {
        let temp = TempDatabase::open();
        temp.db
            .setup_new_transaction(|conn| Ok(insert_user(conn, "alice")))
            .unwrap();
        let (mailer, _sent) = Mailer::capture();
        let app = set_up_routes().with_state(temp.state(mailer));
        let mut body = sign_up_body("bob", "bob@example.com");
        body["is_admin"] = true.into();
        body["email_verified"] = true.into();

        let (status, _, _) = call(&app, post_json("/api/v1/auth/register", body)).await;

        assert_eq!(status, StatusCode::ACCEPTED);
        let flags: (bool, bool) = temp
            .db
            .setup_new_transaction(|conn| {
                Ok(conn
                    .query_row(
                        "SELECT is_admin, email_verified FROM users WHERE username = 'bob'",
                        [],
                        |row| Ok((row.get(0)?, row.get(1)?)),
                    )
                    .unwrap())
            })
            .unwrap();
        assert_eq!(flags, (false, false));
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

    #[tokio::test]
    async fn the_totp_account_routes_need_a_session() {
        for request in [
            Request::get("/api/v1/account/totp")
                .body(Body::empty())
                .unwrap(),
            Request::post("/api/v1/account/totp")
                .body(Body::empty())
                .unwrap(),
            post_json(
                "/api/v1/account/totp/confirm",
                serde_json::json!({ "code": "123456", "password": "correct horse battery" }),
            ),
        ] {
            let (status, body) = json_answer(request).await;
            assert_eq!(status, StatusCode::UNAUTHORIZED);
            assert_eq!(body, serde_json::json!({ "error": "unauthorized" }));
        }
    }

    #[tokio::test]
    async fn setting_up_2fa_over_http() {
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
        let log_in = || {
            post_json(
                "/api/v1/auth/login",
                serde_json::json!({
                    "username_or_email": "alice",
                    "password": "correct horse battery",
                    "device_name": "phone",
                    "platform": "ios",
                }),
            )
        };
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
