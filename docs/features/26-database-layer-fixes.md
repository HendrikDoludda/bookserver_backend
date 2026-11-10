# Database layer fixes

State: in progress

The generic database layer (`Extract`, `Insert`, `Update`, `Search`, `FromRow`,
`DatabaseTypes`, `setup_new_transaction`) has column-name, NULL-handling, primary-key and
transaction bugs that compile fine and fail at runtime — enough to break sign-up, login, logout,
every read of a user, and the library and series lists. Each fix comes with a round-trip test
(write a row, read it back), the kind of test that would have caught all of them.

## Build
- [x] 3 h · Test helper for `#[cfg(test)]` modules: `Connection::open_in_memory()` plus `migrations::run(&mut conn)` — the trait methods take a `&Connection`, so DB tests need no pool. A handler-level test that needs a pooled `Database` must not use a plain `:memory:` path (every pooled connection would get its own empty database) — use a temp file, through a new `Database::open(path)` that `new()` calls with today's path [BE]
- [x] 1 h · Move the PRAGMAs into one `configure(conn)` that the pool and the test helper both call — `foreign_keys` is off on any connection that skips it, so cascade tests would pass for the wrong reason [BE]
- [x] 0.5 h · `[profile.dev.package.argon2] opt-level = 3` — unoptimised Argon2 makes every test that hashes take seconds [BE]
- [ ] 1 h · `LibraryMetadata::from_row` reads `"name"`; the column is `library_name` — every library read fails, so `GET /api/v1/libraries` can't return a row [BE]
- [ ] 2 h · `BookSeriesMetadata::from_row` and `LibraryMetadata::from_row` read nullable columns as `Some(row.get(…)?)`, so a NULL `cover_image`, `description` or release year is an error instead of `None` — the scanner writes NULL covers and years, so every scanned series and library fails to load, `GET /api/v1/libraries/{id}/series` included [BE]
- [ ] 1 h · `BookMetadata::insert` unwraps `last_modified`, so a file whose modification time can't be read panics the insert [BE]
- [ ] 1 h · `BookMetadata::from_row` reads an empty `tags` column back as `[""]` — one empty tag instead of none [BE]
- [ ] 2 h · Round-trip tests for books (`insert`, `extract`, `extract_batch`), which get no fix of their own [BE]
- [x] 4 h · Stop assuming the primary key is `id`: `remove_entry` (`db.rs`), `Update::update` / `update_multiple` (`db_update.rs`) and `Extract::extract` / `extract_batch` (`extract.rs`) all hard-code `WHERE id`, but `sessions`, `email_verification`, `reset_password_requests`, `recovery_codes` and `totp` key on other columns — today this makes every sign-up fail at the verification-code step. A `const KEY` on `Extract` and `Update`, and `DatabaseTypes::get_key_column()` for `remove_entry` [BE]
- [x] 1 h · `Update`, `Delete` and `remove_entry` discard the changed-row count — return it, so a conditional write can tell "nothing matched" from success [BE]
- [x] 1 h · `DatabaseTypes::Session` gives the table name `"session"`; the table is `sessions` [BE]
- [x] 1 h · Add `DatabaseTypes` variants for `totp` and `recovery_codes` [BE]
- [x] 1 h · `UserMetadata::insert` writes only `username`, `password_hash` and `email` — also write `is_admin` and `email_verified` (today the first user never becomes admin) [BE]
- [x] 2 h · Every read of a user fails: `UserMetadata::from_row` expects `created_at` and `last_login` as integer seconds, but `created_at` defaults to `CURRENT_TIMESTAMP` (text) and `last_login` starts out NULL. Store both as unix seconds (default `(strftime('%s', 'now'))`) and make `last_login` an `Option`; the time columns of the other auth tables become `INTEGER` too [DB] [BE]
- [x] 2 h · Remove `SelectionMethod`: `PassedInColumns` selects only the searched columns, so `from_row` fails whenever a row matches — in `validate_new_user` a taken email comes back as `DatabaseSearchFailure` instead of `EmailTaken`, and the admin check errors as soon as an admin exists. Every search selects `*` [BE]
- [x] 1 h · `search_for_row` turns every SQL and `FromRow` error into `EntryNotFound` — a 404 once errors reach clients, and never logged; log it and return `QueryFailure` [BE]
- [ ] 1 h · `extract`, `extract_batch` and `search_for_multiple_rows` — and on the write side `remove_entry_with_connection`, `Update`, `Delete` and the `db_login_challenges` functions — replace the rusqlite error with a generic variant without logging it, so a row that fails to load or write leaves no trace of why — log it once, as `search_for_row` does [BE]
- [ ] 1 h · Move `db_sessions.rs`'s hand-written queries onto the generic functions, per ARCHITECTURE: `delete_for_device` (`Delete` with two columns), `delete_by_token_hash` (`Delete` with `Or`) and `rotate_tokens` (a read and `update_multiple` in the transaction) [BE]
- [x] 3 h · `setup_new_transaction` replaces the closure's error with `DatabaseError::OperationFailure` — pass the original error through, so `EmailTaken`, `IncorrectCredentials` and the extractors' 401 branches can reach the client [BE]
- [x] 2 h · `setup_new_transaction` opens a deferred transaction, and one that reads and then writes fails at once with `SQLITE_BUSY_SNAPSHOT` when another connection wrote in between (WAL) — the busy timeout doesn't help. Use `IMMEDIATE`, plus an async helper that runs the transaction on Tokio's blocking pool [BE]
- [x] 1 h · `UserDatabaseColumns::UserId` maps to `"user_id"`; the key column of `users` is `id` [BE]
- [x] 1 h · `RecoveryCodes::from_row` reads `"rocovery_id"` instead of `"recovery_id"`, and `recovery_codes.used` is nullable but read as `bool` — make it `NOT NULL` [DB] [BE]
