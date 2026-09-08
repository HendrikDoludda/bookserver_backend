# Bookserver Backend — Project Map

Personal **book / manga / comic home server** backend. Scans directories of books,
stores metadata in SQLite, extracts cover images, and exposes an HTTP API (Axum)
for a frontend to consume.

**Two places to look, and they have different jobs:**
- [ROADMAP.md](ROADMAP.md) — the **build order**: which tier is next, and the headline items.
- [docs/README.md](docs/README.md) — the index of [docs/features/](docs/features/), one
  document per feature holding its **full task list, decisions, open questions, and current
  state in the code**.

> **Maintenance note for Claude:** when a task is completed, update its status marker in the
> owning **feature doc** (⬜ → ✅, or 🟡 for partial) *and* the headline marker in ROADMAP.md
> if that item is one of the headlines. A settled open question moves up into that doc's
> Decisions section with its rationale — don't just delete it. Keep this map accurate too if
> module responsibilities move.

## Commands
- Type-check: `cargo check`
- Build: `cargo build`   ·   Run: `cargo run`
- Tests: `cargo test` (no tests yet — see [docs/features/07-cross-cutting-backend-polish.md](docs/features/07-cross-cutting-backend-polish.md))
- Runtime config via env vars: `PORT` (default 8080), `BOOK_DIRS` (`:`-separated), `API_KEY`,
  and `SMTP_HOST` / `SMTP_PORT` / `SMTP_USERNAME` / `SMTP_PASSWORD` / `SMTP_FROM` / `SMTP_FROM_NAME`

> ⚠️ The crate **does not currently compile** and the router **panics at startup** even when it
> does. Both blockers are described at the top of [ROADMAP.md](ROADMAP.md); fix them before
> trying to run anything.

## Where to find what

### Entry & wiring
- `src/main.rs` — binary entry. Inits tracing, opens the DB, calls `start_server`. Returns `anyhow::Result`.
- `src/lib.rs` — module declarations + re-exports. Note `pub use data_models::models` and
  `pub use database_related_scripts::db`, which is why `crate::models::…` and `crate::db::…`
  resolve despite the nested directories.
- `src/config.rs` — env-var config readers (`get_port`, `get_books_dirs`, `get_api_key`,
  `get_email_config`) plus `set_up_config_file`, which generates a 256-bit signing key into
  `secrets.toml` on first run. **Nothing consumes that key yet** — see
  [docs/features/12-operations-and-security.md](docs/features/12-operations-and-security.md).

### HTTP layer — `src/routes/`
- `api_routes.rs` — the **route table** (`set_up_routes`) and `start_server`. Router state is `Arc<Database>`.
- `api_caller.rs` — the **handler functions** (what each endpoint does). Mix of implemented + stubbed; stubs note *why* they're blocked (some of those notes are now stale). `ping_server` doubles as the liveness check and the placeholder for routes with no handler yet.
- `auth.rs` — real auth logic (~566 lines): user creation, email verification, login, password change/reset, Argon2 hashing, and the session + refresh token pair. **Not yet wired to routes.**
- `totp_management.rs` — comment-only stubs for the TOTP flows.
- `email_helper.rs` — `create_new_email`: builds and sends mail over SMTP via `mail-send`.

### Database layer — `src/database_related_scripts/`
- `db.rs` — the **`Database` struct** (r2d2 SQLite pool) and all DB operations: `get_entry`, `get_entries_batch`, `insert`, `update_value`, `remove_entry`, library/series queries, `setup_new_transaction`.
- `extract.rs` — **`Extract` trait**: read a row → metadata struct. Each type supplies `TABLE` / `COLUMNS` / `from_row`; gets `extract` (by id) + `extract_batch` (IN-list → `Vec<WithId<T>>`) as default methods.
- `insert.rs` — **`Insert` trait**: metadata struct → `INSERT`.
- `db_update.rs` — **`Update` trait**: update one column by id (plus a multi-column variant).
- `db_from_row.rs` — the **`FromRow` impls**: `rusqlite::Row` → metadata struct, by column name.
- `db_search.rs` — **`Search` trait**: find a row by arbitrary columns + values, `And`/`Or` separated. Also used to prove a value *doesn't* exist (uniqueness checks).
- `migrations.rs` — versioned migration runner; loads `migrations/*.sql` via `include_str!`. To add schema: new numbered `.sql` + register in the `MIGRATIONS` array; never edit an applied migration. *(Early dev exception: migration 0001 is still edited in place and the DB wiped.)*
- `mod.rs` — module declarations.

### Domain models & errors — `src/data_models/`
- `models.rs` — the domain structs & enums: `BookMetadata`, `BookSeriesMetadata`, `LibraryMetadata`, `UserMetadata`, `SeriesLibraryConnection`, `Sessions`, `TOTP`, `EmailVerification`, `RecoveryCodes`, `EmailConfig`; the `*DatabaseColumns` enums; `LibraryType` / `BookFormat` / `BookLanguage`; `DatabaseTypes`; `ColumnSelector`; `QuerySeparator` / `SelectionMethod`; **`WithId<T>`** (pairs row id + metadata, flattened in JSON); `ParsedName`, `FileExtractedMetadata`, `DatabaseEntry`.
- `authentication_model.rs` — the auth **request/response bodies** (`UserCreationRequest`, `LoginRequest`, `VerifyEmailRequest`, `ChangePasswordRequest`, `DeviceInformation`, `AuthResponse`, …).
- `totp_model.rs` / `information_retrieval_models.rs` — TOTP and library-request bodies. *(These don't derive `Deserialize` yet, so they can't back a `Json<T>` extractor.)*
- `src/error_types.rs` — 10 typed error enums per domain (`DatabaseError`, `FolderScannerError`, `CoverImageError`, `StreamReaderErrors`, `RequestErrors`, `RoutingErrors`, `AuthenticationError`, `EmailErrors`, `AppErrors`, `ApplicationSetUpErrors`). Only `DatabaseError` and `RequestErrors` implement Axum's `IntoResponse`.

### Scanner — `src/scanner/`
- `folder_scanner.rs` — walks configured dirs, parses filenames (regex for volume/chapter/page), inserts entries. Entry point: `scan_all_folders`.
- `cover_image_retriever.rs` — extracts a cover per format (PDF / CBZ / EPUB / image). Entry point: `get_cover_image`. (EPUB path untested.)
- `metadata_fetcher.rs` — **empty file**; online metadata fetching lives in [docs/features/09-metadata-enrichment.md](docs/features/09-metadata-enrichment.md).
- `mod.rs` — module declarations.

### Reading & misc
- `src/stream_reader.rs` — `streaming_file`: serves a file with HTTP range support via `tower-http` `ServeFile`.
- `src/utils.rs` — small helpers (`convert_file_path_to_blob`).

### Data & assets (runtime, not source)
- `migrations/0001_initial_schema.sql` — 10 tables: `books`, `library`, `library_elements`
  (junction: composite PK `(library_id, series_id)`, **no `id`**), `series`, `users`,
  `email_verification`, `reset_password_requests`, `sessions`, `recovery_codes`, `totp`.
- `data/databases/app_data.sqlite` — the SQLite database.
- `cover_images/` — extracted cover images.

### Docs — `docs/`
- `docs/README.md` — index of the feature docs + the doc conventions.
- `docs/features/*.md` — one doc per feature. Numbering: `01`–`19` Phase 1, `20`–`29` Phase 2,
  `90`+ backlog, with gaps left so new features slot in without renumbering.

## Key patterns & conventions
- **DB traits, one per operation:** `Extract` (read), `Insert` (write), `Update` (modify), `Search` (find by column), `FromRow` (row → struct). Each metadata type implements the relevant ones; `Database` methods wrap them with a pooled connection.
- **Two flavours of every `Database` method:** the plain one takes a connection from the pool; the `*_with_connection` variant takes a `&Connection` so several operations can share one transaction (`setup_new_transaction`).
- **`WithId<T>`** is how list endpoints return id + metadata together (the metadata structs don't store their own id).
- **Errors:** `thiserror` enums per domain. `DatabaseError::into_response` is the reference shape: `EntryNotFound` → 404, everything else → 500 with the detail logged server-side and a generic body to the client.
- **Column names are matched by string in two places** — `Extract::COLUMNS` (the `SELECT` list) and `FromRow` (`row.get("…")`). A mismatch compiles fine and fails at runtime; three such bugs exist today. This is the main argument for the DB unit tests.
- **SQL safety:** values are always bound parameters (`?1`, `params!`, `params_from_iter`). Table/column names are only ever interpolated from trusted enums/consts, never from request input.
- **Handlers** use Axum extractors: `State<Arc<Database>>`, `Path<i64>`, `Json<T>`, `Request<Body>`.
- **Junction table** `library_elements` deliberately does *not* implement `Extract` (no `id` column); it's queried by FK via dedicated `Database` methods.
