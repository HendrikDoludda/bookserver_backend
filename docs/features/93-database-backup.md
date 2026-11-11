# Database backup

State: not started

One click to back up the SQLite catalog — cheap insurance for a home server that holds a
whole library and its users. Optional — not scheduled.

## Build
- [ ] ? h · Admin-only backup endpoint (`AdminSession`) that writes a consistent copy with `VACUUM INTO 'path'` — under `journal_mode = WAL`, copying the `.sqlite` file alone is not a valid backup, because recent commits can still sit in the `-wal` file. The backup includes `secrets.toml`: restored next to another one, the database's TOTP secrets can't be decrypted ([04](04-two-factor-auth-totp.md)) [BE]
- [ ] ? h · Read the database path from one place — `Database::new` hard-codes it [BE]

## Open decisions
- [ ] Back up the database only, or the database plus `./cover_images/`? — blocks: the backup endpoint. Suggested: the database only — covers can be re-extracted from the source files; the catalog and the user accounts can't.
