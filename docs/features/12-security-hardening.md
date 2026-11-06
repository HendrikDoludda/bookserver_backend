# Security hardening

State: not started

The hardening a LAN + Tailscale home server actually needs: secrets and the database readable
only by their owner, served files kept inside the library folders, and a clear answer for the
unused signing key. What only matters on the open internet lives in
[54](54-public-exposure-hardening.md).

## Build
- [ ] ? h · Write `secrets.toml` owner-only (`0600`), as the TODO in `set_up_config_file` asks: create the temp file with `OpenOptions::new().mode(0o600).create_new(true)` before the rename, so the real path is never readable by others; Unix only (`#[cfg(unix)]`) [BE]
- [ ] ? h · Create the SQLite file owner-only (`0600`) [BE]
- [ ] ? h · Load the signing key into `AppState` ([14](14-email-and-notifications.md)) and make the fields of `Secrets` readable — `main.rs` loads it into `_secrets` and nothing reads it; the TOTP encryption key is derived from it ([04](04-two-factor-auth-totp.md)) [BE]
- [ ] ? h · Serve only files inside the library folders: `request_file` hands the stored `file_path` straight to `ServeFile` — canonicalize the file and each allowed root, compare with `Path::starts_with` (it compares whole components, so `/books-secret` doesn't match `/books`), and answer 404 otherwise, as the TODO at the top of `api_caller/library.rs` asks. Required once uploads put user-influenced names on disk [BE]
- [ ] ? h · Run `cargo audit` (`cargo install cargo-audit`) as a development step, later in CI [BE]
- [ ] ? h · Upgrade a password hash on login when its stored Argon2 parameters differ from the current defaults — `validate_submitted_password` already parses the PHC string; re-hash while the plaintext is still in scope (low priority) [BE]
- [ ] ? h · Configure CORS (`tower_http::cors::CorsLayer`, no new crate) once a browser client on another origin calls the API [BE]
- [ ] ? h · Check by hand: `ls -l` shows `-rw-------` on `secrets.toml` and the SQLite file, and another local user can't read either

## Open decisions
- [ ] Check served paths against `BOOK_DIRS` now, or wait for the scannable-directories table? — blocks: serving only files inside the library folders. Suggested: check against `get_books_dirs()` plus `cover_images/` now, and point it at the table when that lands.
