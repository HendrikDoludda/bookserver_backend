# Download permission

State: not started

A per-user permission: users who have it may download the original, unmodified file and keep
it; everyone else can only view through the app. The server enforces it by never handing a
restricted user the file — it is policy, not copy protection: anything the app can show, the
person holding the device can capture.

## Build
- [ ] ? h · Schema for the permission on the access model, per the decisions below [DB]
- [ ] ? h · Model and `FromRow` / `Insert` / `Update` / `Search` impls for it [BE]
- [ ] ? h · A `DownloadPermitted` extractor composed on `AuthSession`, reading the permission from the user's row on every request — never from the request itself, and not cached in the session, so a change applies on the next request [BE]
- [ ] ? h · Put `DownloadPermitted` on the download endpoint from [Offline download](39-offline-download.md): a restricted user gets 403 with a generic body and no bytes, and can still stream and read the same title [BE]
- [ ] ? h · Include the permission in the current user's API response, so the app can hide the download button — a courtesy; the server still refuses [BE]
- [ ] ? h · Hide the download button for users without the permission [FE]
- [ ] ? h · Admin UI: set the permission per user (and per library, if the access model goes that way) [FE]
- [ ] ? h · Close the WebDAV door for restricted users, per the WebDAV decision [BE]
- [ ] ? h · Close the OPDS door for restricted users, per the OPDS decision [BE]
- [ ] ? h · Log downloads of the original file, per the logging decision [BE]
- [ ] ? h · Check by hand: point a WebDAV client at the mount with a download-restricted user's access and confirm it exposes nothing they are restricted from

## Open decisions
- [ ] What is the permission attached to: the user, the library, or the pair? — blocks: the schema. Suggested: per library on a user↔library table, with a default on `users` that a library row overrides — a movie and a book may well differ.
- [ ] One access model with per-library visibility ([10](10-sensitive-content-controls.md)) and per-library permissions ([92](92-per-library-permissions.md)), or a mechanism of its own? — blocks: the schema. Suggested: one `user_library_access` table with a column per capability (`can_view`, `can_download`, …), settled before any of the three is built.
- [ ] What is the default for a new user? — blocks: the schema. Suggested: denied — granting it is a deliberate act of trust.
- [ ] The WebDAV door ([70](70-webdav-for-infuse.md)) hands out raw files and can't express this permission — blocks: closing the WebDAV door. Suggested: don't issue WebDAV credentials to restricted users; "has WebDAV access" implies the permission, and the admin UI says so.
- [ ] OPDS acquisition links ([23](23-opds-feed.md)) are download links by definition — blocks: closing the OPDS door. Suggested: a browse-only feed (no acquisition links) for restricted users, or no OPDS access — decide when OPDS is built.
- [ ] Log downloads? It is a record of what household members read and watch — blocks: logging downloads. Suggested: original-file downloads only, not streams, kept short-lived, in the [security audit log](55-security-audit-log.md).
