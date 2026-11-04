# Packaging & deployment

State: not started

Ship the server as a Docker image, a macOS `.dmg` and a Windows `.exe`, with the database and
covers stored outside the app so updates and containers keep them.
[Desktop auto-update](25-desktop-auto-update.md) builds on the installers.

## Build
- [ ] ? h · One configurable data location, per the data-directory decision: the database path is hard-coded twice (`Database::new` and `DatabaseTypes::get_file_path`, which also `unwrap()`s the directory creation) and `COVER_DIR` is `./cover_images/` — all relative to the working directory [BE]
- [ ] ? h · Replace the placeholder `ProjectDirs` identifier (`"placeholder_organization"`) in `set_up_config_file` with the app's permanent identifier, per the identifier decision, before the first release — changing it later moves the config directory, and existing installs lose `secrets.toml` [BE]
- [ ] ? h · Ship the PDFium library with every build and bind it by path (`Pdfium::bind_to_library`) — `Pdfium::default()` looks in the working directory, which is `/` for an app started from Finder (TODO in `cover_image_retriever.rs`) [BE]
- [ ] ? h · Docker image with a persistent volume for the database and covers [BE]
- [ ] ? h · macOS `.dmg` [BE]
- [ ] ? h · Windows `.exe` [BE]

## Open decisions
- [ ] Where do the database and covers live once packaged? — blocks: one configurable data location. Suggested: `directories::ProjectDirs` by default (already used for `secrets.toml`), with an env-var override that Docker points at its volume.
- [ ] What is the app's permanent identifier? The data directory, the macOS bundle id and the updater's app id all derive from it, and changing it after the first release strands existing installs — blocks: replacing the placeholder identifier. Suggested: pick the final organization and app name before the first public build, together with the app name in [Theming & Branding](08-theming-and-branding.md).
