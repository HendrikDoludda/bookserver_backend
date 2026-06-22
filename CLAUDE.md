# Bookserver Backend — Project Map

Personal **book / manga / comic home server** backend. Scans directories of books,
stores metadata in SQLite, extracts cover images, and exposes an HTTP API (Axum)
for a frontend to consume. See [ROADMAP.md](ROADMAP.md) for the feature checklist,
future plans, and design decisions (auth approach, security stance, etc.).

> **Maintenance note for Claude:** when a step from ROADMAP.md is completed,
> update its status marker there (⬜ → ✅, or 🟡 for partial) in the same change.
> Keep this map accurate too if module responsibilities move.

## Commands
- Type-check: `cargo check`
- Build: `cargo build`   ·   Run: `cargo run`
- Tests: `cargo test` (none yet — see ROADMAP §6)
- Runtime config via env vars: `PORT` (default 8080), `BOOK_DIRS` (`:`-separated), `API_KEY`

## Where to find what

### Entry & wiring
- `src/main.rs` — binary entry. Inits tracing, opens the DB, calls `start_server`. Returns `anyhow::Result`.
- `src/lib.rs` — module declarations + re-exports (`db`, `folder_scanner`, `cover_image_retriever`).
- `src/config.rs` — env-var config readers (`get_port`, `get_books_dirs`, `get_api_key`).

### HTTP layer — `src/routes/`
- `api_routes.rs` — the **route table** (`set_up_routes`) and `start_server`. Router state is `Arc<Database>`.
- `api_caller.rs` — the **handler functions** (what each endpoint does). Mix of implemented + stubbed; stubs note *why* they're blocked.
- `auth.rs` — placeholder, empty. Auth not built yet (ROADMAP §3).

### Database layer — `src/database_related_scripts/`
- `db.rs` — the **`Database` struct** (r2d2 SQLite pool) and all DB operations: `get_entry`, `get_entries_batch`, `insert`, `update_value`, `remove_entry`, library/series queries, `setup_new_transaction`.
- `extract.rs` — **`Extract` trait**: read a row → metadata struct. Each type supplies `TABLE` / `COLUMNS` / `from_row`; gets `extract` (by id) + `extract_batch` (IN-list → `Vec<WithId<T>>`) as default methods.
- `insert.rs` — **`Insert` trait**: metadata struct → `INSERT`.
- `db_update.rs` — **`Update` trait**: update one column by id.
- `migrations.rs` — versioned migration runner; loads `migrations/*.sql` via `include_str!`. To add schema: new numbered `.sql` + register in the `MIGRATIONS` array; never edit an applied migration.
- `mod.rs` — module declarations.

### Domain models & errors
- `src/models.rs` — all structs & enums: `BookMetadata`, `BookSeriesMetadata`, `LibraryMetadata`, `UserMetadata`, `SeriesLibraryConnection`; the `*DatabaseColumns` enums; `LibraryType` / `BookFormat` / `BookLanguage`; `DatabaseTypes`; `ColumnSelector`; **`WithId<T>`** (pairs row id + metadata, flattened in JSON); `ParsedName`, `FileExtractedMetadata`, `DatabaseEntry`.
- `src/error_types.rs` — typed error enums per domain (`DatabaseError`, `FolderScannerError`, `CoverImageError`, `StreamReaderErrors`, `RequestErrors`, `RoutingErrors`) + their Axum `IntoResponse` impls.

### Scanner — `src/scanner/`
- `folder_scanner.rs` — walks configured dirs, parses filenames (regex for volume/chapter/page), inserts entries. Entry point: `scan_all_folders`.
- `cover_image_retriever.rs` — extracts a cover per format (PDF / CBZ / EPUB / image). Entry point: `get_cover_image`. (EPUB path untested.)
- `metadata_fetcher.rs` — early/stub: online metadata fetching (ROADMAP §8).
- `mod.rs` — module declarations.

### Reading & misc
- `src/stream_reader.rs` — `streaming_file`: serves a file with HTTP range support via `tower-http` `ServeFile`.
- `src/utils.rs` — small helpers (`convert_file_path_to_blob`).

### Data & assets (runtime, not source)
- `migrations/0001_initial_schema.sql` — tables: `books`, `library`, `library_elements` (junction: composite PK `(library_id, series_id)`, **no `id`**), `series`, `users`.
- `data/databases/app_data.sqlite` — the SQLite database.
- `cover_images/` — extracted cover images.

## Key patterns & conventions
- **Three DB traits, one per operation:** `Extract` (read), `Insert` (write), `Update` (modify). Each metadata type implements the relevant ones; `Database` methods wrap them with a pooled connection.
- **`WithId<T>`** is how list endpoints return id + metadata together (the metadata structs don't store their own id).
- **Errors:** `thiserror` enums per domain. `DatabaseError` implements `IntoResponse` (currently maps everything to HTTP 500 — see ROADMAP §6 for the 404 fix).
- **SQL safety:** values are always bound parameters (`?1`, `params!`, `params_from_iter`). Table/column names are only ever interpolated from trusted enums/consts, never from request input.
- **Handlers** use Axum extractors: `State<Arc<Database>>`, `Path<i64>`, `Json<T>`, `Request<Body>`.
- **Junction table** `library_elements` deliberately does *not* implement `Extract` (no `id` column); it's queried by FK via dedicated `Database` methods.
