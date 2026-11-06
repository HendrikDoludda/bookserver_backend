# Bookserver Backend

A self-hosted backend for a personal book, manga and comic library, written in Rust. It scans
your folders, catalogs the files in SQLite, extracts cover images, and serves everything over
an HTTP API for a reader frontend.

> **Status: work in progress.** Scanning, cataloging, cover extraction and file streaming
> work. Authentication is implemented but not yet wired to routes, and TOTP two-factor auth
> is planned. See [docs/project-state.md](docs/project-state.md) for what works today and the
> [build checklist](docs/build-checklist.md) for what comes next.

## Highlights

- **Directory scanner.** Walks the configured folders, processes each file on Tokio's
  blocking thread pool (with a semaphore limiting concurrency), and parses volume / chapter /
  page numbers out of filenames.
- **Cover extraction per format.** PDF (rendered with PDFium), CBZ (first image in the
  archive), EPUB, and plain folders of images. Each cover is named by the file's BLAKE3
  content hash.
- **File streaming with HTTP range requests**, so a reader can seek inside large files
  without downloading them whole.
- **SQLite behind a connection pool**, with versioned, transactional migrations embedded in
  the binary at compile time.
- **Authentication (in progress).** Argon2id password hashing, email verification over
  SMTP, and opaque random session tokens that are stored only as BLAKE3 hashes, with a
  15-minute session token and a rotating 30-day refresh token.

## Tech stack

Rust · [Axum](https://github.com/tokio-rs/axum) 0.8 · Tokio · rusqlite + r2d2 · thiserror /
anyhow · argon2 · blake3 · pdfium-render · zip · image · mail-send · tracing

## Architecture

```
src/
├── main.rs                     entry point: logging, database, server
├── config.rs                   env-var config + generated secrets file
├── routes/
│   ├── api_routes.rs           route table (/health, /api/v1/…)
│   ├── api_caller/             request handlers per area + auth extractors
│   ├── auth.rs                 account, session and token logic
│   └── email_helper.rs         SMTP delivery
├── database_related_scripts/
│   ├── db.rs                   Database: connection pool + queries
│   ├── extract.rs, insert.rs,
│   │   db_update.rs, db_search.rs,
│   │   db_delete.rs            one trait per database operation
│   ├── db_from_row.rs          row → model mapping
│   └── migrations.rs           versioned migration runner
├── data_models/                domain models, request/response bodies
├── scanner/                    folder scanner + cover extraction
├── stream_reader.rs            range-request file streaming
└── error_types.rs              typed error enums, one per domain
migrations/                     SQL migrations (embedded at compile time)
docs/                           one design doc per feature
```

A few design choices worth calling out:

- **One trait per database operation** (`Extract`, `Insert`, `Update`, `Search`, `Delete`).
  Each model declares its table and columns, and the trait's default methods build the
  queries. Supporting a new model takes a few lines rather than a new query layer.
- **Transactions as closures.** Every `Database` method has a `*_with_connection` twin, so
  a handler can compose several operations inside
  `db.setup_new_transaction(|conn| { … })`, which commits on `Ok` and rolls back on `Err`.
- **SQL safety.** Values are always bound parameters. Table and column names come only from
  trusted enums and constants, never from request input.
- **Typed errors.** Each domain has its own `thiserror` enum. Database errors map to HTTP
  status codes: 404 for a missing row, and 500 with the detail logged server-side while the
  client gets a generic message.
- **No N+1 queries.** List endpoints fetch the ids first, then load every row in a single
  `IN (…)` query. Results are returned as `WithId<T>` so the client gets each row's id
  alongside its metadata.

## Getting started

**Prerequisites**

- A recent stable Rust toolchain (developed on Rust 1.96).
- For PDF covers only: the PDFium shared library (`libpdfium.dylib`, `libpdfium.so` or
  `pdfium.dll`) in the working directory or on the system library path. Prebuilt binaries
  are available from [bblanchon/pdfium-binaries](https://github.com/bblanchon/pdfium-binaries).

**Run**

```sh
git clone https://github.com/HendrikDoludda/bookserver_backend.git
cd bookserver_backend
BOOK_DIRS=/path/to/books cargo run

curl http://localhost:8080/health                              # → pong
curl -X POST http://localhost:8080/api/v1/scan_all_directories
```

On first run the server creates its SQLite database at `./data/databases/app_data.sqlite`.
Extracted covers are written to `./cover_images/`.

**Configuration** (environment variables)

| Variable | Default | Purpose |
| --- | --- | --- |
| `PORT` | `8080` | HTTP port |
| `BOOK_DIRS` | `../Downloads` | `:`-separated list of directories to scan |
| `SMTP_HOST`, `SMTP_PORT`, `SMTP_USERNAME`, `SMTP_PASSWORD`, `SMTP_FROM`, `SMTP_FROM_NAME` | unset | Outgoing email for account verification and password resets |

## API

Implemented endpoints:

| Method | Path | Description |
| --- | --- | --- |
| `GET` | `/health` | Liveness check |
| `GET` | `/api/v1/get_libraries` | All libraries |
| `GET` | `/api/v1/get_series_from_library/{id}` | Series in a library |
| `GET` | `/api/v1/get_series_children/{id}` | Books in a series |
| `GET` | `/api/v1/book_metadata/{id}` | One book's metadata |
| `GET` | `/api/v1/book/{id}` | Stream the book file (supports `Range`) |
| `POST` | `/api/v1/scan_all_directories` | Scan `BOOK_DIRS` and catalog new files |
| `POST` | `/api/v1/setup_db` | Run any pending migrations |
| `DELETE` | `/api/v1/delete_library/{id}` | Delete a library |

The other registered routes are placeholders for upcoming features. Known issues that still
affect some of these endpoints are tracked in
[Database layer fixes](docs/features/26-database-layer-fixes.md).

> **Security note:** the API is not authenticated yet, and the server listens on all
> interfaces. Run it on a trusted network only until authentication is wired in.

## Project docs

- [Build checklist](docs/build-checklist.md): every feature, its tier in the build order, and its state.
- [Feature docs](docs/features/): one document per feature — what to build and what is still undecided.
- [Architecture](docs/ARCHITECTURE.md): the conventions the code follows.

## Development

```sh
cargo build
cargo clippy
cargo fmt
```

There are no automated tests yet. A database-layer test suite comes with the next item,
[Database layer fixes](docs/features/26-database-layer-fixes.md).

## License

Copyright © 2026 Hendrik Doludda. All rights reserved.

The source is published for viewing only. No license is granted to use, copy, modify or
distribute it.
