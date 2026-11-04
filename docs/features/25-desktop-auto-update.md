# Desktop Auto-Update

> Plex-style updates for the `.dmg` / `.exe` installs: a menu bar icon, a native notification when a new version is out, and one click to install it and relaunch.

**Status:** ⬜ not started · **Phase:** 2 · **Tier:** Parked — out of focus · **Tags:** [BE] [FE?]
**Depends on:** [12 Operations & Security](12-operations-and-security.md) (the `.dmg`/`.exe` packaging and the data-directory question both live there) · [06 Backend Admin Web View](06-admin-web-view.md) (only for the optional in-browser update banner)
**Blocks:** Nothing
**Roadmap:** [Build order](../../ROADMAP.md#priority-order-build-sequence)

## Why

On the machine hosting the server, an update should take one click: a notification says a new version is out, you click install, and the server comes back on the new version without opening a terminal or re-running an installer. That is the Plex Media Server experience on a Mac, and it is the target here. It matters more once someone other than the owner runs the server: an install that has to be updated by hand mostly stays on whatever version it was installed with, security fixes included. The scope is deliberately the desktop installs only — the `.dmg` and `.exe` targets from [Operations & Security](12-operations-and-security.md) — because those are the installs where the app owns its own files and can replace them.

## Current state

Nothing exists, and most of the prerequisites don't either.

- **No releases.** No `.github/`, no CI, no published builds. The version is `0.1.0` in [Cargo.toml:3](../../Cargo.toml#L3) and nothing reads it at runtime.
- **No packaging.** The `.exe` / `.dmg` / Docker task is ⬜ in [Operations & Security](12-operations-and-security.md#tasks).
- **The server is headless.** `#[tokio::main]` ([main.rs:7](../../src/main.rs#L7)) gives the async runtime the main thread, and no tray-icon or notification crate is in [Cargo.toml](../../Cargo.toml).
- **No graceful shutdown.** `axum::serve(listener, app)` ([api_routes.rs:34](../../src/routes/api_routes.rs#L34)) has no shutdown signal, so stopping the process drops in-flight requests.
- **Data paths are relative to the working directory.** The DB path is hard-coded as `./data/databases/app_data.sqlite` in two places ([db.rs:31](../../src/database_related_scripts/db.rs#L31), [models.rs:43-48](../../src/data_models/models.rs#L43)), and covers go to `COVER_DIR = "./cover_images/"` ([cover_image_retriever.rs:13](../../src/scanner/cover_image_retriever.rs#L13)). Only `secrets.toml` uses `ProjectDirs`, under a placeholder organization name ([config.rs:25](../../src/config.rs#L25)).
- **pdfium is found at runtime, not bundled.** Cover extraction calls `Pdfium::default()` ([cover_image_retriever.rs:84](../../src/scanner/cover_image_retriever.rs#L84)), with the bundling TODO at [:77-78](../../src/scanner/cover_image_retriever.rs#L77).
- **Migrations run on every startup and only look for missing versions.** `Database::new` calls `migrations::run` ([db.rs:54](../../src/database_related_scripts/db.rs#L54)), which walks the known `MIGRATIONS` and applies any not yet recorded ([migrations.rs:39-52](../../src/database_related_scripts/migrations.rs#L39)). Nothing backs the DB up first, and a DB that already carries a *newer* version than the binary knows is not detected — an older build would start against a schema it doesn't understand.

## Decisions

- **Desktop installs only (`.dmg`, `.exe`); Docker is out of scope.** Containers are meant to be immutable, and Docker users update by pulling a new image (`docker compose pull && docker compose up -d`). A container that replaced itself would need the host's Docker socket, which is effectively root on the host. Docker installs keep the usual Docker workflow.
- **The Plex model: a menu bar / tray app, a native notification, and install-and-relaunch.** The notification reaches the owner on the host machine without opening anything; an update button that only lives in the browser admin panel would not.
- **Parked in Phase 2.** It needs the packaging work first, and it is a convenience, not something the core reading flow waits on.

## Open questions

- **Which updater framework?** Options: **Velopack** — written in Rust with a Rust crate, one system for macOS and Windows, builds the installers too, and replaces the whole app bundle, so a bundled pdfium updates with it; its macOS installer is a `.pkg` rather than a `.dmg`, which needs checking against the packaging plan. **Sparkle (macOS) + WinSparkle (Windows)** — the most battle-tested updater on the Mac, but two separate systems, both reached through FFI from Rust. **`self_update` + `self-replace` crates** — pure Rust and simple, but they swap a bare binary, not an `.app` bundle, so pdfium, relaunching and signing are all left to you. *Suggested:* Velopack, provided its authenticity model (next question) holds up.
- **How does the app prove an update came from you?** An updater downloads code and runs it as the server, which can read every book and every user row. HTTPS plus a hash published on the same server only proves the download wasn't corrupted — whoever controls the release host controls every install. A signature checked against a public key compiled into the app (Sparkle's EdDSA model) proves who built it. *Suggested:* require a compiled-in-key signature check. Confirm what the chosen framework provides before committing to it; if it only offers HTTPS + hash, add a signature check on top (e.g. an ed25519 signature verified with the `minisign-verify` crate).
- **Install on click, on a schedule, or when idle?** Restarting the server drops any open reading stream. Options: install only when the owner clicks; install automatically at a set off-hours time (Plex offers this); install automatically once nothing is being streamed. *Suggested:* click-to-install first; add a scheduled option later if it's wanted.
- **Where are releases hosted?** GitHub Releases is free and the usual default, but the source is "all rights reserved" — if that repo stays private, its release files can't be downloaded without a token, and a token shipped inside the app can be extracted by anyone who installs it. Options: a separate public repo that holds only built releases; a static file host or bucket. *Suggested:* a separate public releases-only repo, so the source can stay private.
- **Should the admin web view show the update too, and trigger it?** The [guiding principle](../../ROADMAP.md#guiding-principle) is that everything is changeable from both the frontend and the backend web view, which argues for an admin-only "update available — install" banner backed by the same updater code. It also adds an HTTP endpoint that restarts the server. *Suggested:* yes, once [the admin web view](06-admin-web-view.md) exists — behind `AdminSession`, and only able to install a verified release from the configured host, so the worst a stolen admin session can do is install a genuine newer version.
- **What is the app's permanent identifier?** The data directory (`ProjectDirs::from("com", "placeholder_organization", "bookserver_backend")` at [config.rs:25](../../src/config.rs#L25)), the macOS bundle id and the updater's app id all derive from it, and the updater only matches releases that carry the same id. Changing it after the first release strands existing installs: their data sits in the old directory and their updater looks for the old id. *Suggested:* pick the final organization and app name before the first public build, together with the app name in [Theming & Branding](08-theming-and-branding.md).

## Tasks

- ⬜ Release pipeline: CI builds the macOS and Windows packages for each version tag and publishes them with release notes **[BE]**
- ⬜ Wire the chosen updater framework: check, download, install, relaunch **[BE]**
- ⬜ Verify update authenticity before installing (see the open question) — a package that fails the check is discarded **[BE]**
- ⬜ Menu bar / system tray icon: running status and version, "Open admin", "Check for updates", "Quit" **[BE]**
- ⬜ Background update check (at launch and every few hours) → native notification with the new version and a changelog link **[BE]**
- ⬜ Install-and-relaunch from the notification or tray menu, with a graceful server shutdown first **[BE]**
- ⬜ "Open at login" toggle in the tray menu, so the server comes back after a reboot **[BE]**
- ⬜ Back up the DB with `VACUUM INTO` before a new version applies pending migrations **[BE]**
- ⬜ Refuse to start against a DB whose schema is newer than the binary knows **[BE]**
- ⬜ Code signing: Apple Developer ID + notarization for macOS, a code-signing certificate for Windows — required before anyone other than the owner installs it **[BE]**
- ⬜ Admin web view "update available" banner + install trigger, if the open question lands on yes **[BE] [FE]**

## Done when

- [ ] On a Mac running version N, publishing N+1 produces a native notification within one check interval, with nothing opened by hand.
- [ ] Clicking install replaces the app and relaunches it; the tray menu then shows N+1 and the API answers requests again.
- [ ] After the update, libraries, users, covers and `secrets.toml` are all intact — nothing was stored inside the replaced bundle.
- [ ] When N+1 includes a migration, a backup taken before it exists and opens in `sqlite3` with the old schema version.
- [ ] An update package that fails the authenticity check is not installed, and version N keeps running.
- [ ] Starting version N against a DB already migrated by N+1 exits with a clear error instead of serving requests.
- [ ] The same flow works on Windows from the `.exe` install.

## Implementation notes

- **On macOS, UI code must run on the main thread.** `#[tokio::main]` ([main.rs:7](../../src/main.rs#L7)) currently gives the main thread to the async server. With a tray icon that flips: the tray's event loop (`tray-icon` plus an event loop such as `tao` or `winit`) owns the main thread, and the tokio runtime is built by hand on a thread of its own. A sketch, not checked against current crate versions:

  ```rust
  fn main() -> anyhow::Result<()> {
      VelopackApp::build().run(); // must run first: handles install/update hooks and may exit the process
      std::thread::spawn(|| {
          let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
          rt.block_on(run_server()) // today's main body: tracing, DB, secrets, start_server
      });
      run_tray_event_loop() // owns the main thread until Quit
  }
  ```

- **Velopack's update flow, roughly** (from its README — check current docs before relying on the exact API):

  ```rust
  let um = UpdateManager::new(source, None, None)?;
  if let UpdateCheck::UpdateAvailable(update) = um.check_for_updates()? {
      um.download_updates(&update, None)?;
      um.apply_updates_and_restart(&update)?;
  }
  ```

- **Shut the server down cleanly before the swap.** `axum::serve(listener, app)` ([api_routes.rs:34](../../src/routes/api_routes.rs#L34)) takes `.with_graceful_shutdown(signal)`. The tray's "Quit" and the updater's restart should both fire that signal, so in-flight requests finish and pooled SQLite connections close before the process exits.
- **Data has to live outside the app bundle.** An update replaces the whole `.app`, so anything inside it is lost; and an app launched from Finder starts with `/` as its working directory, so `./data/...` and `./cover_images/` fail outright. This is the data-directory open question in [Operations & Security](12-operations-and-security.md#open-questions): for desktop builds its suggested answer (`ProjectDirs` by default, env-var override for Docker) becomes required. The DB path is hard-coded twice ([db.rs:31](../../src/database_related_scripts/db.rs#L31), [models.rs:43](../../src/data_models/models.rs#L43)), so make it one value in the same change.
- **Bundle pdfium and load it by path.** Ship the library inside the app (`Contents/Frameworks` on macOS, next to the `.exe` on Windows) and bind to that path with `pdfium-render`'s `Pdfium::bind_to_library` instead of `Pdfium::default()` ([cover_image_retriever.rs:84](../../src/scanner/cover_image_retriever.rs#L84)). Each update then ships its matching pdfium, and the bundling TODO at [:77](../../src/scanner/cover_image_retriever.rs#L77) closes with it.
- **Back up inside the migration runner, with `VACUUM INTO`.** The natural place is `migrations::run` ([migrations.rs:26](../../src/database_related_scripts/migrations.rs#L26)): if `schema_migrations` already has rows (not a fresh DB) and at least one migration is pending, write a backup before applying anything. With `journal_mode = WAL`, copying the `.sqlite` file alone is not a valid backup — recent commits may only be in the `-wal` file. `VACUUM INTO ?1` with a timestamped path is one `execute` call and needs no new dependency. It is the same mechanism as idea 5 in the [Ideas Backlog](90-ideas-backlog.md#implementation-notes).
- **Downgrade guard.** Also in `migrations::run`: read `SELECT MAX(version) FROM schema_migrations` and, if it is higher than the last entry in `MIGRATIONS`, return an error instead of starting. Today's loop ([migrations.rs:39-52](../../src/database_related_scripts/migrations.rs#L39)) only asks "which known migrations are missing?", never "is there one I don't know?". Give it its own `DatabaseError` variant so the startup error says what happened.
- **The early-dev migration exception has to end before the first auto-updated release.** Migration 0001 is currently edited in place and the DB wiped. An auto-updated install can't wipe its DB, so from the first release onward every schema change is a new numbered migration, and applied migrations are never edited.
- **Test notifications from the packaged app, not `cargo run`.** macOS ties notifications to an app's bundle identifier; a bare binary started from a terminal has none, so its notifications are attributed to another app or not shown at all.
- **Signing costs.** The Apple Developer Program (about $99/year) covers both signing and notarization; without it Gatekeeper blocks the app for anyone who downloads it. Windows installs without a code-signing certificate trigger SmartScreen warnings. On the owner's own machine both can be skipped.

## Related

- [Operations & Security](12-operations-and-security.md) — owns the `.dmg`/`.exe` packaging, the data-directory question and pdfium bundling that this feature builds on.
- [Backend Admin Web View](06-admin-web-view.md) — home of the optional in-browser update banner, and the "Open admin" target of the tray menu.
- [Authentication, Users & Sessions](03-auth-users-and-sessions.md) — `AdminSession`, which an install-from-the-browser endpoint must sit behind.
- [Theming & Branding](08-theming-and-branding.md) — the app name and icon, which the tray icon and the app bundle should take from the same single source.
- [Ideas Backlog](90-ideas-backlog.md) — idea 5 (DB backup endpoint) uses the same `VACUUM INTO` mechanism as the pre-update backup.
