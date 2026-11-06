# Modular core & media modules

State: not started

Split the server into a core — users, auth, permissions, libraries, the directory walk, the
migration runner, the route table — and media modules attached to it: books first,
[video](20-video.md) and [documents](21-documents.md) next, each owning its tables,
migrations, endpoints and format-specific code, and each switchable off. It comes first in
Phase 2, so the module interface is designed against a second real module; until then the
Modules rules in [ARCHITECTURE.md](../ARCHITECTURE.md) keep the boundary clean, and
[07](07-api-route-naming.md) settles the route shape.

## Build
- [ ] ? h · Cargo workspace: `crates/core`, `crates/books` and a thin `crates/server` binary — modules depend on `core`, never the other way round [BE]
- [ ] ? h · Each module builds its own `Router` with its own state (`.with_state(...)` turns it into a `Router<()>`) and the server mounts it with `.nest(...)`; dispatch over an `enum Module { Books, Video, Documents }` rather than `Box<dyn …>`, because native `async fn` in traits can't be called through `dyn` [BE]
- [ ] ? h · One SQLite database: each module owns its tables (prefixed `book_`, `video_`, …) and its own migration list, and the runner tracks `(module, version)` instead of one global number [DB]
- [ ] ? h · Split the scanner along `LibraryType`: core walks the directories and hands each file to the module that owns the library; scanning is optional per module, because documents don't scan [BE]
- [ ] ? h · Switch modules off per the switch-off decision [BE]

## Open decisions
- [ ] How is a module switched off — a Cargo feature, a runtime setting, or both? The `.dmg` / `.exe` installs ([25](25-desktop-auto-update.md)) and Docker images ship one prebuilt binary, so their users only have a runtime switch; dynamic plugins are out, since Rust has no stable ABI — blocks: switching modules off. Suggested: both — a Cargo feature per module for developers, and an admin-only setting stored in the database for users, with release builds compiling every module in.
- [ ] Where does per-user state that every module has live — progress, favourites, settings? — blocks: the read-state table in [02](02-read-state.md). Suggested: decide it with the [reading position](41-reading-position.md) shape. One core table keyed `(user_id, module, item_id)` gives one cross-media "continue" list but no foreign key on `item_id`, so deleting a book won't remove its progress; one table per module keeps real foreign keys — the safer default if cross-media "continue" isn't a goal.
