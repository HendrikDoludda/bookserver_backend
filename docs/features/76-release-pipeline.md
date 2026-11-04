# Release pipeline

State: blocked

**Blocked:** needs the `.dmg` / `.exe` packaging and the app's permanent identifier from
[Packaging & deployment](52-packaging-and-deployment.md).

Every version tag produces signed macOS and Windows packages, published with release notes
where installs — and the [updater](25-desktop-auto-update.md) — can fetch them. Nothing exists
yet: no CI, no published builds, and the version in `Cargo.toml` (`0.1.0`) is read by nothing
at runtime.

## Build
- [ ] ? h · CI builds the macOS and Windows packages for each version tag and publishes them with release notes, per the hosting decision [BE]
- [ ] ? h · macOS signing: Apple Developer ID plus notarization (the Apple Developer Program, about $99 a year) — without it Gatekeeper blocks the app for anyone but you [BE]
- [ ] ? h · Windows signing with a code-signing certificate — unsigned installs trigger SmartScreen warnings [BE]

## Open decisions
- [ ] Where are releases hosted? If the source repo stays private, its release files can't be downloaded without a token, and a token shipped in the app can be extracted — blocks: publishing. Suggested: a separate public, releases-only repo.
