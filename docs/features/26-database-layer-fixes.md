# Database layer fixes

State: not started

The generic database layer (`Extract`, `Insert`, `Update`, `Search`, `FromRow`,
`DatabaseTypes`) has column-name, NULL-handling and primary-key bugs that compile fine and
fail at runtime — enough to break sign-up, login, logout, every read of a user, and the
library and series lists. Each fix comes with a round-trip test (write a row, read it back),
the kind of test that would have caught all of them.

## Build
- [ ] ? h · Test helper for `#[cfg(test)]` modules: `Connection::open_in_memory()` plus `migrations::run(&mut conn)` — the trait methods take a `&Connection`, so DB tests need no pool. A handler-level test that needs a pooled `Database` must not use a plain `:memory:` path (every pooled connection would get its own empty database) — use a temp file [BE]
- [ ] ? h · `LibraryMetadata::from_row` reads `"name"`; the column is `library_name` — every library read fails, so `GET /api/v1/get_libraries` can't return a row [BE]
- [ ] ? h · `BookSeriesMetadata::from_row` and `LibraryMetadata::from_row` read nullable columns as `Some(row.get(…)?)`, so a NULL `cover_image`, `description` or release year is an error instead of `None` — the scanner writes NULL covers and years, so every scanned series and library fails to load, `GET /api/v1/get_series_from_library/{id}` included [BE]
- [ ] ? h · `BookMetadata::insert` unwraps `last_modified`, so a file whose modification time can't be read panics the insert [BE]
- [ ] ? h · `BookMetadata::from_row` reads an empty `tags` column back as `[""]` — one empty tag instead of none [BE]
- [ ] ? h · Round-trip tests for books (`insert`, `extract`, `extract_batch`), which get no fix of their own [BE]
- [ ] ? h · Stop assuming the primary key is `id`: `remove_entry` (`db.rs`), `Update::update` / `update_multiple` (`db_update.rs`) and `Extract::extract` / `extract_batch` (`extract.rs`) all hard-code `WHERE id`, but `sessions`, `email_verification`, `reset_password_requests`, `recovery_codes` and `totp` key on other columns — today this makes every sign-up fail at the verification-code step [BE]
- [ ] ? h · `DatabaseTypes::Session` gives the table name `"session"`; the table is `sessions` [BE]
- [ ] ? h · Add `DatabaseTypes` variants for `totp` and `recovery_codes` [BE]
- [ ] ? h · `UserMetadata::insert` writes only `username`, `password_hash` and `email` — also write `is_admin` and `email_verified` (today the first user never becomes admin) [BE]
- [ ] ? h · Every read of a user fails: `UserMetadata::from_row` expects `created_at` and `last_login` as integer seconds, but `created_at` defaults to `CURRENT_TIMESTAMP` (text) and `last_login` starts out NULL. Store both as unix seconds (default `(strftime('%s', 'now'))`) and make `last_login` an `Option` [DB] [BE]
- [ ] ? h · `SelectionMethod::PassedInColumns` selects only the searched columns, so `from_row` fails whenever a row matches — in `validate_new_user` a taken email comes back as `DatabaseSearchFailure` instead of `EmailTaken`, and the admin check errors as soon as an admin exists [BE]
- [ ] ? h · `setup_new_transaction` replaces the closure's error with `DatabaseError::OperationFailure` — pass the original error through, so `EmailTaken`, `IncorrectCredentials` and the extractors' 401 branches can reach the client [BE]
- [ ] ? h · `UserDatabaseColumns::UserId` maps to `"user_id"`; the key column of `users` is `id` [BE]
- [ ] ? h · `Extract for ResetPasswordRequest`: `COLUMNS` lacks `cancelation_token`, `attempt_count` and `email_sent_at`, which its `FromRow` reads [BE]
- [ ] ? h · `Insert for ResetPasswordRequest` names 5 columns for 7 values, in a different order — list all seven, in the order of the `params!` [BE]
- [ ] ? h · `RecoveryCodes::from_row` reads `"rocovery_id"` instead of `"recovery_id"` [BE]
