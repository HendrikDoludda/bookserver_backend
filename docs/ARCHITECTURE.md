# Architecture

## Where things live
- `src/main.rs` — entry point: tracing, database, `start_server`.
- `src/lib.rs` — module declarations; re-exports `data_models::models` and `database_related_scripts::db`, so `crate::models::…` and `crate::db::…` resolve.
- `src/config.rs` — env-var readers and the generated `secrets.toml` (in the OS config directory).
- `src/routes/api_routes.rs` — the route table (`set_up_routes`) and `start_server`.
- `src/routes/api_caller.rs` — the handlers and the session extractors (`AuthSession`, `AdminSession`, `AuthTOTPLoginSession`, `AuthRefreshSession`); `ping_server` is the liveness check and the placeholder for routes without a handler.
- `src/routes/auth.rs` — account, password and session logic; `totp_management.rs` — TOTP flows; `email_helper.rs` — SMTP sending.
- `src/database_related_scripts/` — `db.rs` (`Database`: the r2d2 pool and every DB operation), one file per operation trait, `migrations.rs`.
- `src/data_models/` — `models.rs` (domain structs, `*DatabaseColumns` enums, `WithId<T>`) and the request/response bodies.
- `src/error_types.rs` — the typed error enums, one per domain.
- `src/scanner/` — `folder_scanner.rs` (walks the book folders, parses filenames), `cover_image_retriever.rs` (one cover per format), `metadata_fetcher.rs` (placeholder).
- `src/stream_reader.rs` — file serving with HTTP range support.
- `migrations/` — numbered SQL files, embedded at compile time.
- Runtime data, git-ignored: `data/databases/app_data.sqlite`, `cover_images/` (except the `example-cover.png` fallback).

## API
- API-first: every change to data is an HTTP endpoint; the frontend and the admin view are both clients of that same API, no endpoint is reachable from only one of them, and business logic stays in the backend.
- Every route lives under `/api/v1/`, except `/health`.
- A handler takes at most one body extractor (`Json<T>`), and it comes last.
- `user_id` always comes from the session (`AuthSession`), never from the request body or path.
- A permission check is an Axum extractor composed on `AuthSession` (as `AdminSession` is), not an `if` inside handlers.
- Auth answers never reveal whether an account exists: an unknown user and a wrong password get the same response, and so does a password-reset request for an unknown address.
- Never return or `Debug`-log a struct that carries secrets — `UserMetadata`, `Sessions`, `TOTP`, `RecoveryCodes` and `EmailConfig` derive `Serialize` / `Debug` over the password hash, the token hashes, the TOTP secret or the SMTP password; build a response struct.
- List endpoints return `WithId<T>`: the row id plus the metadata, flattened in JSON.

## Errors
- One `thiserror` enum per domain, in `error_types.rs`.
- An error enum that reaches a handler implements `IntoResponse` in the shape of `DatabaseError::into_response`: a status per variant (`EntryNotFound` → 404), and for a 5xx the detail logged server-side and a generic body to the client.

## Logging
- Never log a password, a token (plain or hashed), a TOTP secret, a verification, reset or recovery code, the signing key, or an email body — bodies carry codes.

## Files
- Files are served by row id, never by a path the client supplies.
- Files are served through `tower_http::services::ServeFile` (Range, 206, `If-Range` and `HEAD` included) — never hand-rolled range handling.
- A filesystem path built from request input is sanitised, canonicalised and checked to lie inside a scannable root before it is read or written.
- Blocking work — PDFium, `zip`, `image`, hashing — runs in `spawn_blocking`, never directly in an async handler.

## Database
- One trait per operation — `Extract`, `Insert`, `Update`, `Search`, `Delete`, `FromRow` — implemented per model; `Database` methods wrap them with a pooled connection.
- Every `Database` method has a `*_with_connection` twin taking `&Connection`, so several operations can share one transaction (`setup_new_transaction`).
- Values are always bound parameters; table and column names are interpolated only from trusted enums and consts, never from request input.
- Tables without an `id` column (the `library_elements` junction) don't implement `Extract`; dedicated `Database` methods query them by foreign key.
- List queries fetch the ids first, then every row in one `IN (…)` query — no N+1.
- Every reference to another table is a declared `FOREIGN KEY` — `PRAGMA foreign_keys = ON` enforces only declared ones.
- Times are stored as unix seconds; a nullable time is read as `Option<i64>` — `convert_to_system_time` fails on NULL.
- Back up SQLite with `VACUUM INTO`, never by copying the file — under WAL a plain copy is not a valid backup.
- Schema changes: a new numbered `.sql` in `migrations/`, registered in `MIGRATIONS`; an applied migration is never edited. Early-development exception: migration 0001 is edited in place and the database wiped.

## Modules
- Core code — auth, users, sessions, settings, email, config, migrations — never imports book-specific types (`BookMetadata`, `BookSeriesMetadata`, `BookFormat`, the filename parser); book code may use core, never the reverse. Today's mixed files (`models.rs`, the per-model trait impls, `api_caller.rs`) get split up by [94](features/94-modular-core.md).
- Per-user data references `users.id` through a declared foreign key, in the one SQLite database.
- Format-specific behaviour — filename parsing, cover extraction, reading positions — is chosen by `LibraryType` / `BookFormat` in one place, never spread through core.

## Frontend
- Components read design tokens, never literal colours or sizes.
- Every visible string goes through `t("key")` against the backend's translation catalog.

## Hosting
- A home server on the LAN, reached remotely only through Tailscale — never public port-forwarding. Hardening that only matters on the open internet lives in [54](features/54-public-exposure-hardening.md).
