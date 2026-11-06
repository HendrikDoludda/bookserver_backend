# Safe schema upgrades

State: not started

Upgrading the server must never cost data: back the database up before a new version migrates
it, and refuse to start against a schema newer than the binary knows. Today `migrations::run`
applies whatever is missing, with no backup and no check for versions it doesn't know.

## Build
- [ ] ? h · In `migrations::run`, when the database isn't fresh and a migration is pending, first write a backup with `VACUUM INTO` and a timestamped path — copying the `.sqlite` file is not a valid backup under `journal_mode = WAL` [BE]
- [ ] ? h · Refuse to start when `MAX(version)` in `schema_migrations` is higher than the last known migration, with its own `DatabaseError` variant and a clear startup error [BE]
- [ ] ? h · Check by hand: when a new version brings a migration, the backup taken before it exists and opens in `sqlite3` with the old schema version
