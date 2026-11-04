# WebDAV for Infuse

State: blocked

**Blocked:** needs the video library from [20](20-video.md) — there is nothing to expose yet.

Infuse on the Apple TV can't talk to the REST API, so the video library is also published over
WebDAV — the same files, video only. It uses basic-auth and is reached remotely only through
Tailscale; it is not hardened for the open internet.

## Build
- [ ] ? h · Mount `dav-server` (not a dependency yet) as a service with `nest_service("/dav", …)` — WebDAV needs methods like `PROPFIND` and `MKCOL` that Axum's per-verb helpers don't route [BE]
- [ ] ? h · Root the WebDAV filesystem at the video directory only — that root is the security boundary: no book, comic or document path, and nothing outside the video root, is reachable through it [BE]
- [ ] ? h · WebDAV credentials, per the two-factor decision [BE]
- [ ] ? h · Check by hand: Infuse on an Apple TV, given the WebDAV URL and credentials, lists the video library and plays a file directly

## Open decisions
- [ ] WebDAV via `dav-server`, or a plain SMB / NFS / DLNA share? — blocks: mounting `dav-server`. Suggested: `dav-server` — one binary, auth stays in the backend, and it inherits the server's TLS; a share is the fallback if mounting proves annoying.
- [ ] How does a basic-auth door coexist with [two-factor auth](04-two-factor-auth-totp.md)? Infuse can't prompt for a code, so the account password alone would bypass it — blocks: WebDAV credentials. Suggested: require TLS and issue a device-scoped WebDAV credential instead of accepting the account password.
- [ ] Must watch progress sync between the app and Infuse? Infuse keeps its own position and won't report it over WebDAV — blocks: nothing yet; a yes would add work. Suggested: no — the app is the door that reports progress.
