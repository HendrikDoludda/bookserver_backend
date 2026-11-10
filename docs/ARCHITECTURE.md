# Architecture

## Where things live
- `src/main.rs` — entry point: tracing, database, `start_server`.
- `src/lib.rs` — module declarations; re-exports `data_models::models` and `database_related_scripts::db`, so `crate::models::…` and `crate::db::…` resolve.
- `src/config.rs` — env-var readers and the generated `secrets.toml` (in the OS config directory).
- `src/app_state.rs` — `AppState`, the router state (database, mailer, setup code, TOTP cipher); handlers take their part through `FromRef` (`State<Arc<Database>>`, `State<Mailer>`, `State<SetupCode>`, `State<TotpCipher>`).
- `src/routes/api_routes.rs` — the route table (`set_up_routes`) and `start_server`.
- `src/routes/api_caller/` — the handlers, one file per area (`authentication.rs`, `totp.rs`, `library.rs`); `extractors.rs` holds the request extractors — the session ones (`AuthSession`, `AdminSession`, `AuthTOTPLoginSession`, `AuthRefreshSession`) and `JsonBody` / `PathParams`, and `helpers.rs` the shared response builders and `ping_server`, the liveness check and the placeholder for routes without a handler.
- `src/routes/auth.rs` — account, password and session building blocks, including the checks a new username, email or password passes; `login.rs` — login, refresh and logout; `registration.rs` — sign-up and email verification; `password_reset.rs` — the reset flow; `email_codes.rs` — the emailed codes of both: making them, mailing them within the limits, checking them; `totp_management.rs` — TOTP two-factor and recovery codes; `totp_cipher.rs` — `TotpCipher`, which encrypts TOTP secrets at rest; `email_helper.rs` — `Mailer`, the one sender every email goes through.
- `src/database_related_scripts/` — `db.rs` (`Database`: the r2d2 pool and every DB operation), one file per operation trait, `db_email_codes.rs` / `db_login_challenges.rs` / `db_login_throttle.rs` (one table each), `db_users.rs` (the user queries the traits can't express), `migrations.rs`.
- `src/data_models/` — `models.rs` (domain structs, `*DatabaseColumns` enums, `WithId<T>`) and the request/response bodies.
- `src/error_types.rs` — the typed error enums, one per domain.
- `src/scanner/` — `folder_scanner.rs` (walks the book folders, parses filenames), `cover_image_retriever.rs` (one cover per format), `metadata_fetcher.rs` (placeholder).
- `src/stream_reader.rs` — file serving with HTTP range support.
- `src/test_support.rs` — shared test fixtures, compiled only for tests.
- `migrations/` — numbered SQL files, embedded at compile time.
- Runtime data, git-ignored: `data/databases/app_data.sqlite`, `cover_images/` (except the `example-cover.png` fallback).

## API
- API-first: every change to data is an HTTP endpoint; the frontend and the admin view are both clients of that same API, no endpoint is reachable from only one of them, and business logic stays in the backend.
- Every route lives under `/api/v1/`, except `/health`.
- Routes are resource paths — plural nouns and `{id}` segments, never verbs: `GET /libraries/{id}/series`, not `/get_series_from_library/{id}`.
- A handler reads its body with `JsonBody<T>` and its path with `PathParams<T>`, never with axum's `Json` or `Path`; at most one body extractor, and it comes last.
- `user_id` always comes from the session (`AuthSession`), never from the request body or path.
- Session, refresh and challenge tokens travel as `Authorization: Bearer <token>`; every failed token check answers the same 401, and a response that carries tokens sends `Cache-Control: no-store`.
- Tokens are stored only as `hash_token` (BLAKE3) hashes and looked up by hash; passwords and emailed codes, which people choose or type, are Argon2-hashed; recovery codes, 80 random bits, are BLAKE3-hashed like tokens.
- TOTP secrets are stored encrypted with `TotpCipher` (the user id as associated data), not hashed — checking a code needs the secret itself.
- An account endpoint re-checks the password with `check_current_password` (`login.rs`): login's throttle, and 400 `incorrect_password` for a wrong one — never login's 401, which the app reads as an ended session.
- A permission check is an Axum extractor composed on `AuthSession` (as `AdminSession` is), not an `if` inside handlers.
- Auth answers never reveal whether an account exists: an unknown user and a wrong password get the same response, and so do password-reset and verification-resend requests for an unknown address. The one exception is sign-up, which answers 409 for a taken email or username.
- Every stored email address and username has passed `check_email` / `check_username` (`auth.rs`).
- Never return or `Debug`-log a struct that carries secrets — `UserMetadata`, `Sessions`, `TOTP` and `RecoveryCodes` derive `Serialize` / `Debug` over the password hash, the token hashes or the TOTP secret; build a response struct. Request bodies that carry a password or a code, `EmailConfig` (the SMTP password) and `EmailInformation` (a body carries codes) don't derive `Debug` at all.
- List endpoints return `WithId<T>`: the row id plus the metadata, flattened in JSON.

## Errors
- One `thiserror` enum per domain, in `error_types.rs`.
- Every error answer is JSON `{"error": "<key>"}` — a stable key the app translates; a 500 answers `internal_error` and logs the detail server-side.
- An error enum that reaches a handler implements `IntoResponse` through a `status_and_key` match with one arm per variant and no `_` arm, so a new variant can't silently become a 500.
- A handler returns `Result<_, AppErrors>` and uses `?`; an error type without `IntoResponse` (such as `anyhow`) is logged and answered with `internal_error_response()`.

## Email
- Mail is sent inline only at sign-up, whose caller just created the account and waits for its code; every other mail (reset and resent verification codes, upload notices) goes out from a background task after the commit, so an answer never shows whether a mail went out.
- Mail goes to the address stored on the account, never to an address taken from the request.

## Logging
- Log with `tracing`; the `log` crate is not a dependency.
- Never log a password, a token (plain or hashed), a TOTP secret, a verification, reset or recovery code, the signing key, or an email body — bodies carry codes.
- The one exception: the first-sign-up setup code, logged at start while no account exists.
- A rusqlite error is logged once, where it becomes a `DatabaseError` — `map_err(logged(action, what, variant))` (`db.rs`); callers don't log it again.

## Files
- Files are served by row id, never by a path the client supplies.
- Files are served through `tower_http::services::ServeFile` (Range, 206, `If-Range` and `HEAD` included) — never hand-rolled range handling.
- A filesystem path built from request input is sanitised, canonicalised and checked to lie inside a scannable root before it is read or written.
- Blocking work — PDFium, `zip`, `image`, hashing — runs in `spawn_blocking`, never directly in an async handler.

## Database
- One trait per operation — `Extract`, `Insert`, `Update`, `Search`, `Delete`, `FromRow` — implemented per model; `Database` methods wrap them with a pooled connection.
- Every `Database` method has a `*_with_connection` twin taking `&Connection`, so several operations can share one transaction (`setup_new_transaction`).
- A read-only lookup runs through `Database::read` — no transaction, so it never waits for the write lock.
- A transaction holds only SQL: it starts `IMMEDIATE`, an async handler runs it through `Database::transact` (on Tokio's blocking pool), Argon2 runs in `spawn_blocking` outside it, and mail goes out after the commit.
- Inside a transaction closure, use the `conn` it was given — a pooled `db.*` call opens a second connection that waits on the first one's write lock.
- Use the generic functions, also for a write that depends on what is stored: inside an `IMMEDIATE` transaction no other writer can step between a read, a compare in Rust and the write. Hand-written SQL only for what they can't express.
- A try at a secret (an emailed, TOTP or recovery code) is counted and committed before the secret is checked — counted in the same transaction as a failed check, it is rolled back with it; the final transaction re-checks what it uses up and changes exactly one row.
- Every connection is set up by `db::configure` — SQLite applies `foreign_keys` per connection, not per file.
- Values are always bound parameters; table and column names are interpolated only from trusted enums and consts, never from request input.
- Each table names its key column (`KEY` on `Extract` and `Update`, `DatabaseTypes::get_key_column`); nothing assumes it is `id`.
- Tables without a single key column (the `library_elements` junction, `email_codes`) don't implement `Extract`; dedicated functions query them — `Database` methods for the junction, `db_email_codes.rs` for the codes.
- List queries fetch the ids first, then every row in one `IN (…)` query — no N+1.
- Every reference to another table is a declared `FOREIGN KEY` — `PRAGMA foreign_keys = ON` enforces only declared ones.
- Times are stored as `INTEGER` unix seconds and read with `read_time` (must have a value — NULL is an error naming the column) or `read_optional_time` (NULL is `None`).
- A column is nullable exactly when its Rust field is an `Option`.
- Back up SQLite with `VACUUM INTO`, never by copying the file — under WAL a plain copy is not a valid backup.
- Schema changes: a new numbered `.sql` in `migrations/`, registered in `MIGRATIONS`; an applied migration is never edited. Early-development exception: migration 0001 is edited in place and the database wiped.

## Tests
- A test lives in a `#[cfg(test)] mod tests` at the bottom of the file it tests; shared fixtures live in `src/test_support.rs`.
- A database test runs on `memory_connection()`: in-memory, configured and migrated, fresh for every test.
- A test that needs the pool or the router uses `TempDatabase` — never a plain `:memory:` path, which gives every pooled connection its own empty database.
- Router tests send requests with `tower::ServiceExt::oneshot` and read bodies with `axum::body::to_bytes`.
- Fixtures insert rows with plain SQL, so they don't depend on the model code under test.
- Logic that depends on the time takes `now` as a parameter, so tests never sleep.

## Modules
- Core code — auth, users, sessions, settings, email, config, migrations — never imports book-specific types (`BookMetadata`, `BookSeriesMetadata`, `BookFormat`, the filename parser); book code may use core, never the reverse. Today's mixed files (`models.rs`, the per-model trait impls, `api_caller/library.rs`) get split up by [94](features/94-modular-core.md).
- Per-user data references `users.id` through a declared foreign key, in the one SQLite database.
- Format-specific behaviour — filename parsing, cover extraction, reading positions — is chosen by `LibraryType` / `BookFormat` in one place, never spread through core.

## Frontend
- Components read design tokens, never literal colours or sizes.
- Every visible string goes through `t("key")` against the backend's translation catalog.

## Hosting
- A home server on the LAN, reached remotely only through Tailscale — never public port-forwarding. Hardening that only matters on the open internet lives in [54](features/54-public-exposure-hardening.md).
