# Flutter app

State: in progress

The main client: one Flutter app for phone and desktop, built alongside the server so each
feature can be tested in it. The desktop version is a client only — the server's own desktop
install is [52](52-packaging-and-deployment.md).

## Build
- [x] 1 h · Texts for the server-address, login and home screens and for failed connections, in `translations/en/frontend/` and `translations/de/frontend/` [BE]
- [x] 2 h · Replace the template app: the folder layout, and design tokens for the first screens [FE]
- [ ] 2 h · Server address screen: an `https://` origin — `http://` in debug builds only — accepted when `GET /health` answers `pong` [FE]
- [x] 1 h · Network access: the INTERNET permission in Android's main manifest, the outgoing-network entitlement on macOS [FE]
- [x] 2 h · API client: Bearer tokens and JSON bodies to the server's origin, redirects never followed; `{"error": key}` answers become typed errors, a failed connection or a timeout a connection error [FE]
- [ ] 2 h · Keep the server's origin, the device id and the session tokens as one record in the platform's secure storage — macOS's legacy keychain, no Android backup — and open the app on the screen the record calls for; an unreadable record counts as none [FE]
- [ ] 1 h · Refresh the session token when it has less than a minute left, and once after a 401; a refused refresh ends the session, a network failure doesn't [FE]
- [ ] 1 h · Refresh runs one at a time: parallel requests wait for that single refresh instead of each starting their own [FE]
- [ ] 1 h · Placeholder home: the libraries' names from `GET /api/v1/libraries`, with a reload action [FE]
- [ ] 2 h · Android build — an APK can be sideloaded freely [FE]
- [ ] 3 h · iOS build — outside the App Store it needs a signing identity, and a free Apple account's signing expires after 7 days, forcing re-installs [FE]
- [ ] 3 h · macOS and Windows desktop builds — Windows is added with `flutter create --platforms=windows .` and builds only on a Windows machine [FE]

## Open decisions
- [ ] When do the apps go into the App Store and Play Store? — blocks: nothing yet. Suggested: once someone other than you uses the server; the Apple Developer Program fee also covers the macOS signing in [76](76-release-pipeline.md).
- [ ] A web frontend as well, eventually? — blocks: nothing yet. Suggested: decide once the app covers reading — under the API-first rule it would be one more client of the same API.
