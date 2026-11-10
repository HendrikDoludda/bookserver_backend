# Packaging & deployment

State: not started

Ship the server as a Docker image, a macOS `.dmg` and a Windows `.exe`, with the database and
covers stored outside the app so updates and containers keep them.
[Desktop auto-update](25-desktop-auto-update.md) builds on the installers.

## Build
- [ ] ? h · One configurable data location, per the data-directory decision: the database path is hard-coded twice (`Database::new` and `DatabaseTypes::get_file_path`, which also `unwrap()`s the directory creation) and `COVER_DIR` is `./cover_images/` — all relative to the working directory [BE]
- [ ] ? h · Replace the placeholder `ProjectDirs` identifier (`"placeholder_organization"`) in `set_up_config_file` with the app's permanent identifier, per the identifier decision, before the first release — changing it later moves the config directory, and existing installs lose `secrets.toml` — and with it every stored TOTP secret ([04](04-two-factor-auth-totp.md)) [BE]
- [ ] ? h · Ship the PDFium library with every build and bind it by path (`Pdfium::bind_to_library`) — `Pdfium::default()` looks in the working directory, which is `/` for an app started from Finder (TODO in `cover_image_retriever.rs`) [BE]
- [ ] ? h · Graceful shutdown: `axum::serve` gets `.with_graceful_shutdown(signal)`, fired by `SIGTERM` and Ctrl-C, so in-flight requests finish and pooled SQLite connections close — `docker stop` sends `SIGTERM` and kills the process 10 seconds later; the tray's "Quit" ([77](77-tray-app.md)) and the updater's restart ([25](25-desktop-auto-update.md)) fire it too [BE]
- [ ] 3 h · Serve HTTPS without `tailscale serve` ([32](32-https-tls.md)) through `axum-server` 0.9 (feature `tls-rustls`): `RustlsConfig::from_pem_file(cert, key)` and `bind_rustls`, with a certificate per the LAN-HTTPS question [BE]
- [ ] 1 h · Settings for the certificate and key paths [BE]
- [ ] 2 h · Pick up a renewed certificate without a restart (`RustlsConfig::reload_from_pem_file`), triggered per the reload decision [BE]
- [ ] ? h · The desktop installs show the setup code the first sign-up needs ([27](27-registration-and-email-verification.md)) — an app started from Finder or the Start menu has no visible log [BE]
- [ ] ? h · Docker image with a persistent volume for the database and covers [BE]
- [ ] ? h · Publish the Docker image for each version, per the registry decision [BE]
- [ ] ? h · macOS `.dmg`, unsigned for now — macOS refuses the first launch until it is allowed in System Settings → Privacy & Security; signing comes with [76](76-release-pipeline.md) [BE]
- [ ] ? h · Windows `.exe`, unsigned for now — SmartScreen warns before the first launch; signing comes with [76](76-release-pipeline.md) [BE]
- [ ] ? h · Check the dependency licences with `cargo deny check licenses` before the first publish [BE]
- [ ] ? h · Ship the third-party licence notices — the crates' and PDFium's — with every published build [BE]

## Open decisions
- [ ] Where do the database and covers live once packaged? — blocks: one configurable data location. Suggested: `directories::ProjectDirs` by default (already used for `secrets.toml`), with an env-var override that Docker points at its volume.
- [ ] What is the app's permanent identifier? The data directory, the macOS bundle id and the updater's app id all derive from it, and changing it after the first release strands existing installs — blocks: replacing the placeholder identifier. Suggested: pick the final organization and app name before the first public build, together with the app name in [Theming & Branding](08-theming-and-branding.md).
- [ ] Where is the Docker image published? — blocks: publishing the image. Suggested: Docker Hub — a public image needs no public source repo.
- [ ] What triggers the certificate reload — a timer, a signal, or watching the files? — blocks: picking up a renewed certificate. Suggested: a daily timer — certificates last weeks, so polling is plenty, needs no extra crate, and works on Windows, which has no signals.
- [ ] Under which terms may people run the published builds? The repo is "All rights reserved", which on its own lets nobody use the image or the executables — blocks: publishing the image. Suggested: none — your call; worth a lawyer once money is involved.

## Missing information
- [ ] How does a LAN device without Tailscale get HTTPS — which name does it resolve, and who issues the certificate for that name (own domain with local DNS, a local CA)? A `*.ts.net` certificate only validates where MagicDNS resolves the name — answered by: you, before the in-server TLS tasks
