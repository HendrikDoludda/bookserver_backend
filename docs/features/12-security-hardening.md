# Security hardening

State: in progress

The hardening a LAN + Tailscale home server actually needs: secrets and the database readable
only by their owner, and served files kept inside the library folders. What only matters on the
open internet lives in [54](54-public-exposure-hardening.md).

## Build
- [x] 1 h · Write `secrets.toml` owner-only (`0600`): the temp file is created with `create_new` and mode `0o600` before the rename — after removing one a crash left behind — so the real path is never readable by others; an existing file with wider permissions is narrowed at every start, with a warning; Unix only (`#[cfg(unix)]`) [BE]
- [ ] ? h · Create the SQLite file owner-only (`0600`) [BE]
- [x] 1 h · Use the signing key: `Secrets::signing_key()` decodes it (anything but 32 bytes stops the server at start), and `main.rs` derives the TOTP encryption key from it into `AppState` (`TotpCipher`, [04](04-two-factor-auth-totp.md)); the key itself joins `AppState` once [74](74-signed-streaming-urls.md) needs it [BE]
- [ ] ? h · Serve only files inside the library folders: `request_file` hands the stored `file_path` straight to `ServeFile` — canonicalize the file and each allowed root, compare with `Path::starts_with` (it compares whole components, so `/books-secret` doesn't match `/books`), and answer 404 otherwise, as the TODO at the top of `api_caller/library.rs` asks. Required once uploads put user-influenced names on disk [BE]
- [ ] ? h · Run `cargo audit` (`cargo install cargo-audit`) as a development step, later in CI [BE]
- [ ] ? h · Upgrade a password hash on login when its stored Argon2 parameters differ from the current defaults — `validate_submitted_password` already parses the PHC string; re-hash while the plaintext is still in scope (low priority) [BE]
- [ ] ? h · Configure CORS (`tower_http::cors::CorsLayer`, no new crate) once a browser client on another origin calls the API [BE]
- [ ] ? h · Check by hand: `ls -l` shows `-rw-------` on the SQLite file, and another local user can't read it

## Open decisions
- [ ] Check served paths against `BOOK_DIRS` now, or wait for the scannable-directories table? — blocks: serving only files inside the library folders. Suggested: check against `get_books_dirs()` plus `cover_images/` now, and point it at the table when that lands.
