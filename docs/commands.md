# Commands

| Command | What it does | Destructive |
|---|---|---|
| `cargo check` | Type-check without building | no |
| `cargo build` | Build the server | no |
| `cargo test` | Run the automated tests | no |
| `cargo clippy` | Lint | no |
| `cargo fmt` | Format the code | no |
| `rm -f data/databases/app_data.sqlite*` | Delete the database and its WAL files; the next start recreates it from the migrations — how edits to migration 0001 are applied during early development | yes |
