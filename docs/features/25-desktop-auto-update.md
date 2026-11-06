# Desktop auto-update

State: blocked

**Blocked:** needs the `.dmg` / `.exe` packaging and a data directory outside the app bundle
([Packaging & deployment](52-packaging-and-deployment.md)), plus the
[release pipeline](76-release-pipeline.md).

Plex-style updates for the desktop installs: a native notification when a new version is out,
one click to install it, and the server comes back on the new version. Docker installs keep the
usual image pull — a container that replaced itself would need the host's Docker socket, which
is effectively root.

## Build
- [ ] ? h · Wire the updater framework, per the framework decision: check, download, install, relaunch [BE]
- [ ] ? h · Verify each update's authenticity before installing, per the authenticity decision — a package that fails is discarded and the running version keeps running [BE]
- [ ] ? h · Check for updates at launch and every few hours, and show a native notification with the new version and a changelog link [BE]
- [ ] ? h · Install and relaunch from the notification or the [tray menu](77-tray-app.md), after a graceful shutdown, per the install-timing decision [BE]
- [ ] ? h · Admin endpoint to install an update, behind `AdminSession`, that can only install a verified release from the configured host — per the admin-web-view decision [BE]
- [ ] ? h · "Update available — install" banner in the [admin web view](06-admin-web-view.md), per the admin-web-view decision [FE]
- [ ] ? h · Check by hand on a Mac, from the packaged app (macOS ties notifications to the bundle id; a bare `cargo run` binary shows none): publishing N+1 brings a notification within one check interval, and installing relaunches on N+1 with libraries, users, covers and `secrets.toml` intact
- [ ] ? h · Check by hand: the same flow on Windows, from the `.exe` install

## Open decisions
- [ ] Which updater framework: Velopack, Sparkle + WinSparkle, or `self_update` + `self-replace`? — blocks: wiring the updater. Suggested: Velopack — a Rust crate covering macOS and Windows that also builds the installers and replaces the whole app bundle, so a bundled pdfium updates with it; check that its macOS `.pkg` installer fits the `.dmg` plan, and that its authenticity model holds up.
- [ ] How does the app prove an update came from you? HTTPS plus a hash only proves the download wasn't corrupted — blocks: verifying authenticity. Suggested: a signature checked against a public key compiled into the app; if the framework only offers HTTPS plus a hash, add an ed25519 check on top (e.g. `minisign-verify`).
- [ ] Install on click, on a schedule, or when nothing is streaming? A restart drops open streams — blocks: install and relaunch. Suggested: click-to-install first; a scheduled option later if wanted.
- [ ] Should the admin web view show and trigger updates too? — blocks: the admin endpoint and banner. Suggested: yes, once the admin web view exists — everything should be changeable from both UIs.
