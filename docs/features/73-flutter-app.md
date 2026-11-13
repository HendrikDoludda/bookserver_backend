# Flutter app

State: not started

The main client: one Flutter app for phone and desktop, built alongside the server so each
feature can be tested in it. The desktop version is a client only — the server's own desktop
install is [52](52-packaging-and-deployment.md).

## Build
- [ ] 8 h · App skeleton: enter the server's address, log in through [03](03-login-and-sessions.md), keep the session in the platform's secure storage and refresh it before it expires [FE]
- [ ] 2 h · Android build — an APK can be sideloaded freely [FE]
- [ ] 3 h · iOS build — outside the App Store it needs a signing identity, and a free Apple account's signing expires after 7 days, forcing re-installs [FE]
- [ ] 3 h · macOS and Windows desktop builds — Windows is added with `flutter create --platforms=windows .` and builds only on a Windows machine [FE]

## Open decisions
- [ ] When do the apps go into the App Store and Play Store? — blocks: nothing yet. Suggested: once someone other than you uses the server; the Apple Developer Program fee also covers the macOS signing in [76](76-release-pipeline.md).
- [ ] A web frontend as well, eventually? — blocks: nothing yet. Suggested: decide once the app covers reading — under the API-first rule it would be one more client of the same API.
