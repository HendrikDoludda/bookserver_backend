# Bookserver Backend

A self-hosted backend for a personal book, manga and comic library, written in Rust. It scans
your folders, catalogs the files in SQLite, extracts cover images, and serves everything over
an HTTP API for a reader frontend.

> **Status: work in progress.** Scanning, cataloging, cover extraction and file streaming
> work. Accounts work end to end — sign-up with email verification, login with optional TOTP
> two-factor auth and recovery codes, refresh, logout and password reset — and the library
> routes need a login. See [docs/project-state.md](docs/project-state.md) for
> what works today and the [build checklist](docs/build-checklist.md) for what comes next.

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
  15-minute session token and a rotating 30-day refresh token. Optional TOTP two-factor
  login: secrets encrypted at rest with XChaCha20-Poly1305, single-use recovery codes, replay
  protection, and a lock on wrong codes that doubles up to 24 hours.

## Tech stack

Rust · [Axum](https://github.com/tokio-rs/axum) 0.8 · Tokio · rusqlite + r2d2 · thiserror /
anyhow · argon2 · blake3 · pdfium-render · zip · image · mail-send · tracing

## Architecture

```
src/
├── main.rs                     entry point: logging, database, server
├── config.rs                   env-var config + generated secrets file
├── app_state.rs                router state: database, mailer, setup code, TOTP cipher
├── routes/
│   ├── api_routes.rs           route table (/health, /api/v1/…)
│   ├── api_caller/             request handlers per area + auth extractors
│   ├── auth.rs                 account, password and session building blocks
│   ├── registration.rs         sign-up and email verification
│   ├── login.rs                login, refresh and logout
│   ├── password_reset.rs       password reset by emailed code
│   ├── email_codes.rs          emailed codes: issuing, mail limits, checking
│   ├── totp_management.rs      TOTP two-factor auth and recovery codes
│   ├── totp_cipher.rs          TOTP secrets encrypted at rest (XChaCha20-Poly1305)
│   ├── translations.rs         the translation catalog as the apps read it
│   └── email_helper.rs         Mailer: SMTP delivery
├── database_related_scripts/
│   ├── db.rs                   Database: connection pool + queries
│   ├── extract.rs, insert.rs,
│   │   db_update.rs, db_search.rs,
│   │   db_delete.rs            one trait per database operation
│   ├── db_from_row.rs          row → model mapping
│   ├── migrations.rs           versioned migration runner
│   └── translation_seed.rs     checks the shipped translations, syncs them at start
├── data_models/                domain models, request/response bodies
├── scanner/                    folder scanner + cover extraction
├── stream_reader.rs            range-request file streaming
└── error_types.rs              typed error enums, one per domain
migrations/                     SQL migrations (embedded at compile time)
translations/                   UI and mail texts, one folder per language (embedded at compile time)
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
curl -X POST -H "Authorization: Bearer <session_token>" http://localhost:8080/api/v1/scan
```

On first run the server creates its SQLite database at `./data/databases/app_data.sqlite`.
Extracted covers are written to `./cover_images/`. While no account exists, the server logs a
setup code at start; the first sign-up must send it and becomes the admin account. Its answer
carries the `session_token` that scanning needs (valid 15 minutes; log in again for a new one).

**Configuration** (environment variables)

| Variable | Default | Purpose |
| --- | --- | --- |
| `PORT` | `8080` | The port to listen on, 1–65535 |
| `BIND_ADDRESS` | `0.0.0.0` | The IP address to listen on — every interface by default; `127.0.0.1` behind `tailscale serve` |
| `BOOK_DIRS` | `../Downloads` | `:`-separated list of directories to scan |
| `SMTP_HOST` | unset | Mail server for verification and password-reset codes; email is off while unset |
| `SMTP_PORT`, `SMTP_FROM`, `SMTP_FROM_NAME` | unset | Required once `SMTP_HOST` is set |
| `SMTP_USERNAME`, `SMTP_PASSWORD` | unset | The mail server's login, set together; leave both out for a server without one |
| `SMTP_TLS` | `starttls` | `starttls`, `implicit` (usually port 465) or `none` (a local catcher such as Mailpit only; refused together with a password) |

A bad or incomplete setting stops the server at start, naming it.

## API

Every endpoint except `/health`, `/api/v1/auth/…` and the two `/api/v1/locales…` reads needs
`Authorization: Bearer <session_token>` and answers 401 without it; scanning and deleting a library also need an admin
(403 for a member). Implemented endpoints:

| Method | Path | Description |
| --- | --- | --- |
| `GET` | `/health` | Liveness check |
| `GET` | `/api/v1/libraries` | All libraries |
| `GET` | `/api/v1/libraries/{id}/series` | Series in a library |
| `GET` | `/api/v1/series/{id}/books` | Books in a series |
| `GET` | `/api/v1/books/{id}` | One book's metadata |
| `GET` | `/api/v1/books/{id}/file` | Stream the book file (supports `Range`) |
| `POST` | `/api/v1/scan` | Scan `BOOK_DIRS` and catalog new files |
| `DELETE` | `/api/v1/libraries/{id}` | Delete a library |
| `POST` | `/api/v1/auth/register` | Sign up; the first account sends the setup code from the server log and becomes admin. Answers the login tokens, or `202` when the account must first confirm its email — within 48 hours, or a later sign-up deletes it |
| `POST` | `/api/v1/auth/verify_email` | Confirm the email with the mailed code and the password; answers like login |
| `POST` | `/api/v1/auth/verify_email/resend` | Mail a new verification code; always `202` |
| `POST` | `/api/v1/auth/login` | Log in; answers the session and refresh tokens, or a challenge when two-factor auth is on |
| `POST` | `/api/v1/auth/login/totp` | The code step of a two-factor login: the challenge token as Bearer and the authenticator code; answers like login |
| `POST` | `/api/v1/auth/login/recovery_code` | The same with a recovery code instead; each code works once |
| `POST` | `/api/v1/auth/refresh` | New tokens for a refresh token (`Authorization: Bearer`) |
| `POST` | `/api/v1/auth/logout` | End the session behind either token; always `204` |
| `POST` | `/api/v1/auth/password_reset` | Mail a password-reset code; always `202`, so it never reveals whether an address has an account |
| `POST` | `/api/v1/auth/password_reset/confirm` | Set a new password with the emailed code; confirms the email and ends every session |
| `PUT` | `/api/v1/account/password` | Change the password with the current one; `204`, and every other session ends |
| `PUT` | `/api/v1/account/username` | Rename the account with `{"new_username"}`; `204`, or `409 username_taken` for a name another account has, in any case |
| `PUT` | `/api/v1/account/email` | Change the email with `{"new_email", "password"}`: `202` and a code mailed to the new address, or `204` at once when email is off; `409 email_taken` for an address another account has |
| `POST` | `/api/v1/account/email/confirm` | Confirm the change with the mailed code; `204`, and codes mailed to the old address stop working |
| `GET` | `/api/v1/account/settings` | The user's settings, `{"locale": "de" \| null}`; `null` is not chosen |
| `PUT` | `/api/v1/account/settings` | Replace them with the same shape and answer what is stored; a locale such as `de` or `pt-BR`, else `400 invalid_locale` |
| `GET` | `/api/v1/account/totp` | Whether two-factor auth is on, and how many recovery codes are left |
| `POST` | `/api/v1/account/totp` | Start setting up two-factor auth; answers the authenticator secret and its `otpauth://` URL |
| `POST` | `/api/v1/account/totp/confirm` | Confirm the setup with a code and the password; answers 10 recovery codes, shown once, and ends the other sessions |
| `POST` | `/api/v1/account/totp/disable` | Turn two-factor auth off with the password and an authenticator or recovery code; deletes the recovery codes and ends every session |
| `POST` | `/api/v1/account/totp/recovery_codes` | New recovery codes with the password and an authenticator or recovery code; answers 10 codes, shown once, and the old ones stop working |
| `GET` | `/api/v1/users` | Admin only: every account — username, email, verified, role, created and last login |
| `PATCH` | `/api/v1/users/{id}` | Admin only: `{"role": "admin" \| "member"}` and/or `{"email_verified": true}`; answers the changed account, `409 last_admin` for demoting the last admin |
| `GET` | `/api/v1/locales` | No session needed: the languages the server offers, in their order — `code`, `display_name` (in the language itself), `icon`, `is_default` |
| `GET` | `/api/v1/locales/{code}/translations` | No session needed: a language's texts for the apps, `{"<namespace>": {"<name>": "<text>"}}`; a text the language lacks comes in English; `?sections=errors,settings` limits the namespaces; `404 not_found` for a language the server doesn't offer. Sent with an `ETag`; `If-None-Match` with it answers `304` without the texts |

Errors answer with JSON `{"error": "<key>"}` and a matching status code.

The other registered routes answer 501 `not_implemented` until their features are built.

> **Security note:** by default the server listens on all interfaces over plain HTTP. Run it on
> a trusted network only, or behind `tailscale serve` for HTTPS
> ([docs/run-deploy/tailscale.md](docs/run-deploy/tailscale.md)).

## Project docs

- [Build checklist](docs/build-checklist.md): every feature, its tier in the build order, and its state.
- [Feature docs](docs/features/): one document per feature — what to build and what is still undecided.
- [Architecture](docs/ARCHITECTURE.md): the conventions the code follows.

## Development

```sh
cargo build
cargo test
cargo clippy
cargo fmt
```

`cargo test` needs no setup: database tests run against a fresh in-memory SQLite, and router
tests against a temporary database file that is deleted afterwards.

## License

Copyright © 2026 Hendrik Doludda. All rights reserved.

The source is published for viewing only. No license is granted to use, copy, modify or
distribute it.
