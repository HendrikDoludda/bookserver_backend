# Menu bar / tray app

State: not started

On a desktop install the server runs as a menu bar (macOS) / system tray (Windows) app, the
Plex Media Server way: running status and version, "Open admin", "Check for updates", "Quit",
and an "Open at login" toggle so the server comes back after a reboot.

## Build
- [ ] ? h · Restructure `main` so the tray's event loop owns the main thread (macOS requires UI code there) and the tokio runtime runs on a thread of its own — today `#[tokio::main]` gives the main thread to the server [BE]
- [ ] ? h · Tray icon (`tray-icon` plus an event loop such as `tao` or `winit`) with status, version, "Open admin", "Check for updates" and "Quit" [BE]
- [ ] ? h · Graceful shutdown: `axum::serve` gets `.with_graceful_shutdown(signal)`, fired by "Quit" and by the [updater](25-desktop-auto-update.md)'s restart, so in-flight requests finish and pooled SQLite connections close — today stopping the process drops them [BE]
- [ ] ? h · "Open at login" toggle in the tray menu [BE]
