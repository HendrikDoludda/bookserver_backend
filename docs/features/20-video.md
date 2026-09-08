# Video Library

> This backend becomes the single media source for video, reached by two front doors: the custom Flutter app over the REST API, and Infuse over WebDAV.

**Status:** ⬜ not started · **Phase:** 2 · **Tier:** Parked — out of focus · **Tags:** [BE] [FE]
**Depends on:** [03 Authentication, Users & Sessions](03-auth-users-and-sessions.md) (the WebDAV front door needs credentials) · [01 Library & Directory Management](01-library-and-directory-management.md) (video libraries are typed libraries)
**Blocks:** Nothing
**Roadmap:** [Build order](../../ROADMAP.md#priority-order-build-sequence)

## Why

One machine, one copy of the files — the same server that already streams books should stream video, instead of running a second media server beside it. The complication is that the good Apple TV client (Infuse) cannot talk to a custom REST API, so a single API is not enough: the library needs a second door speaking a protocol Infuse understands. Getting both doors onto the same files, with no transcoding and no second copy of the library, is the whole feature. Parked deliberately: the entire Phase 1 backend ladder comes first.

## Current state

Nothing video-related exists in the code. A grep for `video`, `movie`, `episode`, `season`, `webdav`, `dav`, `infuse`, `mp4`, `mkv`, `tmdb`, `tvdb` across `src/` and `migrations/` returns zero hits.

What *does* exist is the pattern the direct-play door will copy, plus the places that will need to learn about video:

- **Live** — [stream_reader.rs:10-13](../../src/stream_reader.rs#L10) `streaming_file` hands the request to `tower_http::services::ServeFile` via `oneshot`, so HTTP Range, `If-Range`, `HEAD` and 206 responses come free from the crate. Three lines total.
- **Live, but wired to a route that cannot start today** — [api_routes.rs:55](../../src/routes/api_routes.rs#L55) registers `GET /api/v1/book/:id` → `request_file` ([api_caller.rs:279-288](../../src/routes/api_caller.rs#L279)), which looks up `BookMetadata` and streams `metadata.file_path` straight off disk. `Cargo.toml` pins `axum = "0.8.9"` ([Cargo.toml:7](../../Cargo.toml#L7)) and Axum 0.8 panics on the `:id` path syntax while `set_up_routes` is still being built ([api_routes.rs:48](../../src/routes/api_routes.rs#L48) is the first offender), so the process aborts before it ever binds the port — no streaming route runs at all until that is fixed. The crate does not even compile today either (five empty-bodied TOTP handlers at [api_caller.rs:617-625](../../src/routes/api_caller.rs#L617)). Both are tracked in [Cross-Cutting Backend Polish](07-cross-cutting-backend-polish.md); neither is video work.
- **No video in the domain model** — `LibraryType` ([models.rs:52-58](../../src/data_models/models.rs#L52)) is `Books | Comics | Magazines | Documents | Others`; `BookFormat` ([models.rs:83-88](../../src/data_models/models.rs#L83)) is `Pdf | Epub | Cbz | ImageComic | None`. No video variant in either, and no season/episode column anywhere in [0001_initial_schema.sql](../../migrations/0001_initial_schema.sql).
- **The scanner rejects video files** — `is_valid_file_type` ([folder_scanner.rs:278-283](../../src/scanner/folder_scanner.rs#L278)) accepts only `pdf`, `epub`, `cbz`, `jpg`, `jpeg`, `png`, and `get_book_format` ([folder_scanner.rs:285-293](../../src/scanner/folder_scanner.rs#L285)) errors on anything else. Both match extensions **case-sensitively**.
- **No WebDAV dependency** — `dav-server` (or any WebDAV crate) is absent from [Cargo.toml](../../Cargo.toml).
- **Empty file, relevant indirectly** — [metadata_fetcher.rs](../../src/scanner/metadata_fetcher.rs) is 0 bytes (not even comments), so the `MetadataProvider` abstraction that video metadata is meant to reuse does not exist yet either.
- **Inherited gap to carry over** — `request_file` serves whatever `file_path` the row holds, with no check that the path is inside a configured library dir and no auth on the route. The TODO is already written at [api_caller.rs:4](../../src/routes/api_caller.rs#L4). Video makes this sharper, because WebDAV exposes a directory *tree* rather than lookups by id.

Roadmap drift: none — every ⬜ marker in the Video section matches the code.

## Decisions

- **The custom backend serves video itself (DECIDED).** One machine, one copy of the files. No separate media server, no duplicated library.
- **Two front doors, both over HTTP (DECIDED).** Because one API cannot reach every client, the same files are published twice: once as REST, once as WebDAV.
- **Front door 1 — custom Flutter app over the normal REST API (DECIDED).** Phone and computer use **direct-play** file streaming with range support, exactly like `stream_reader` does for books. The app plays it with a client that bundles ffmpeg (e.g. `media_kit`) → client-side decode.
- **No server transcoding (DECIDED, out of scope).** Clients decode on-device. This is what makes the "just serve the file with ranges" approach sufficient — the server never touches the video bytes beyond reading them off disk.
- **Front door 2 — Infuse over WebDAV (DECIDED).** Infuse (Apple TV + Apple devices, video-only) **can't talk to a custom REST API**, so the backend also exposes the video library over a protocol Infuse understands. Practical choice: **WebDAV** — it is HTTP-based, and the Rust `dav-server` crate can mount alongside Axum. Infuse also supports SMB/NFS/DLNA if a plain share turns out to be easier.
- **WebDAV auth: basic-auth, protected by Tailscale when remote (DECIDED).** Fits the existing threat model (LAN-only, remote access via Tailscale rather than public port-forwarding) — see [Operations & Security](12-operations-and-security.md).
- **Per-device scope (DECIDED).** Apple TV (Infuse) needs **video only** — no document/book reader there. The WebDAV door does not need to expose the rest of the library.
- **No custom tvOS app (DECIDED, out of scope).** Use Infuse instead — Flutter doesn't target tvOS anyway.
- **Video metadata enhancement stays a separate item (DECIDED).** TMDB / TVDB-style enrichment (posters, synopsis, cast, episode/season data) is kept **distinct** from the Phase 1 metadata work in [Metadata Enrichment](09-metadata-enrichment.md) for two reasons: video needs **different providers** and a **movie/show data model**, *and* it **depends on the video feature existing first**. It is not a smaller sibling of the book/manga/comic providers — it is downstream of this doc. It should still **reuse** §8's `MetadataProvider` abstraction, routed by content type.
- **Phase 2, parked.** Video is explicitly "parked — out of focus" in the priority order; nothing here starts before the Phase 1 ladder is done.

## Open questions

- **WebDAV via `dav-server`, or a plain SMB/NFS/DLNA share?** WebDAV mounts inside the existing Axum process — one binary, one place where auth lives, and it inherits whatever TLS the server already terminates; the cost is a second Rust code path to write and secure. SMB/NFS is OS-level with zero Rust code, but its credentials and permissions live outside the application entirely, so "who can see what" stops being something the backend controls. *Suggested:* WebDAV via `dav-server`, as the roadmap already calls the practical choice — the share options are the fallback if mounting it proves annoying.
- **Where does video live in the data model?** `LibraryType` has no video/movies/series variant and `BookFormat` has no video formats ([models.rs:52-58](../../src/data_models/models.rs#L52), [models.rs:83-88](../../src/data_models/models.rs#L83)), yet the upload dialog spec in [Uploads & File Ingestion](05-uploads-and-ingestion.md) already assumes a typed `movie` / `series` / `original` kind plus season and episode fields. Options: extend `books`/`series` with nullable video columns (fewer tables, but `BookMetadata` grows fields that mean nothing for a PDF), or give video its own table and metadata struct (cleaner, but duplicates the `Extract`/`Insert`/`Update`/`Search` trait quartet and every list endpoint). *Suggested:* leave it open until video actually starts; when it does, design against the upload dialog's field set, since that is the concrete spec that already exists.
- **How does basic-auth over WebDAV interact with TOTP?** Basic-auth sends the account password on every request, and Infuse has no way to prompt for a 6-digit code — so an account with 2FA enabled would either be unusable through Infuse or would have its second factor bypassed by that door. *Suggested:* require TLS for the WebDAV mount and issue a **device-scoped WebDAV credential** rather than accepting the account password, so enabling TOTP ([Two-Factor Authentication](04-two-factor-auth-totp.md)) doesn't quietly open a password-only side entrance.
- **Does watch progress need to be shared between the two doors?** Per-user read state is planned as backend data in [Reading & Read State](02-reading-and-read-state.md), but Infuse tracks its own playback position locally and won't report it back over WebDAV — so "continue watching" would be accurate in the Flutter app and independently tracked on the Apple TV. *Suggested:* accept the split (the Flutter app is the door that reports progress) and don't try to reconcile the two; revisit only if it becomes annoying in practice.

## Tasks

- ⬜ Direct-play video streaming over the REST API (range support) **[BE]**
- ⬜ Capable in-app video player (e.g. Flutter `media_kit`, bundles ffmpeg) **[FE]**
- ⬜ WebDAV endpoint exposing the video library for Infuse **[BE]** (basic-auth; protected by Tailscale when remote)
- ⬜ Video metadata enhancement — TMDB / TVDB-style enrichment (posters, synopsis, cast, episode/season data) **[BE][FE]** — separate from §8 / [Metadata Enrichment](09-metadata-enrichment.md) (different providers, movie/show data model); reuse §8's `MetadataProvider` abstraction routed by content type

## Done when

- [ ] A request for a video file with `Range: bytes=1000-` comes back `206 Partial Content` with exactly that byte range, so seeking mid-file works.
- [ ] The Flutter app plays an MKV and an MP4 from the server end-to-end while the server spawns **no** transcode process (server CPU stays flat during playback).
- [ ] Infuse on an Apple TV, pointed at the server's WebDAV URL with basic-auth credentials, lists the video library and plays a file directly.
- [ ] The WebDAV mount exposes **only** the video library — no book, comic or document path is reachable through it, and no path outside the configured video root resolves.
- [ ] A video's TMDB/TVDB-fetched poster and synopsis are visible in the Flutter app, and the external source id is stored on the entry so it can be re-fetched later.

## Implementation notes

- **Don't hand-roll range handling.** `streaming_file` ([stream_reader.rs:10-13](../../src/stream_reader.rs#L10)) already gets 206 / `If-Range` / `HEAD` for free from `tower_http::services::ServeFile`, and its `.unwrap()` is safe because `ServeFile`'s error type is `Infallible` (a missing file yields a 404 response, not an `Err`). The video endpoint is the same shape. Note that `StreamReaderErrors` ([error_types.rs:171-190](../../src/error_types.rs#L171)) describes a hand-rolled range reader that no longer exists and is entirely dead code — don't resurrect it for video.
- **`media_kit` is a Flutter package, not a Rust crate.** It belongs in the app's `pubspec.yaml`; nothing goes into `Cargo.toml` for it. It bundles libmpv/ffmpeg, which is precisely why the server needs no transcoding.
- **`dav-server` is a new dependency** — nothing in [Cargo.toml](../../Cargo.toml) speaks WebDAV today.
- **The Rust shape for mounting WebDAV under Axum:** `dav-server` gives you a handler that is a *tower service*, not an `async fn` handler, so it goes into the router with `nest_service("/dav", ...)` (or `any_service`), **not** with `get(...)`/`post(...)`. That matters because WebDAV uses HTTP methods Axum's per-verb helpers don't cover at all (`PROPFIND`, `PROPPATCH`, `MKCOL`), and only a service-style mount passes them through untouched. Check the crate's current API when you get there — the point is "nest a service", not "write a handler".
- **The WebDAV filesystem root *is* the security boundary.** `dav-server` is configured with a filesystem backend (a local-filesystem one rooted at a directory). Root it at the video directory only — that single choice enforces both the per-device scope decision (video only on Apple TV) and the path-containment TODO at [api_caller.rs:4](../../src/routes/api_caller.rs#L4), without any per-request path checking of your own.
- **The scanner has to learn video extensions** — `is_valid_file_type` / `get_book_format` ([folder_scanner.rs:278-293](../../src/scanner/folder_scanner.rs#L278)) either grow video cases or video gets its own scan path. Gotcha carried over from books: those matches are **case-sensitive**, so `.MKV` would be silently skipped exactly as `.PDF` is today.
- **Whole-file hashing does not scale to video.** `hash_file` ([folder_scanner.rs:295-299](../../src/scanner/folder_scanner.rs#L295)) BLAKE3s the entire file through `std::io::copy` — fine for a 30 MB CBZ, painful for a multi-gigabyte remux, and it runs on every scan of a new file. *Suggestion:* for video, identify files by size + mtime, or hash only a prefix, rather than reading tens of gigabytes per scan.
- **Posters come from the provider, not the file.** `get_cover_image` ([cover_image_retriever.rs:16-28](../../src/scanner/cover_image_retriever.rs#L16)) dispatches on `BookFormat` and has no video branch; extracting a frame would mean decoding video server-side, which the no-transcoding decision rules out. TMDB/TVDB artwork is the intended source.
- **Warning to keep:** the WebDAV endpoint is basic-auth and relies on Tailscale for protection when accessed remotely — it is not hardened for the open internet, and neither is the rest of the server (see [Operations & Security](12-operations-and-security.md)).

## Related

- [Reading & Read State](02-reading-and-read-state.md) — direct-play reuses the same `ServeFile` range-streaming pattern; "continue watching" would extend the same per-user read-state tables.
- [Library & Directory Management](01-library-and-directory-management.md) — the video library needs a scannable directory and a typed library, and the scanner's format filter has to accept video extensions.
- [Authentication, Users & Sessions](03-auth-users-and-sessions.md) — WebDAV basic-auth verifies against the same users and Argon2 password hashes.
- [Two-Factor Authentication (TOTP)](04-two-factor-auth-totp.md) — a basic-auth door has no way to prompt for a TOTP code; see the open question.
- [Uploads & File Ingestion](05-uploads-and-ingestion.md) — its upload dialog already specifies the video field set (`movie` / `series` / `original` kind, plus series/season/episode) and library-type-driven defaults.
- [Metadata Enrichment (Online Sources)](09-metadata-enrichment.md) — the video-metadata item reuses its `MetadataProvider` abstraction routed by content type, with different providers.
- [Cross-Cutting Backend Polish](07-cross-cutting-backend-polish.md) — the Axum 0.8 `:id` → `{id}` path-syntax fix has to land before any streaming route can start.
- [Operations & Security](12-operations-and-security.md) — Tailscale threat model, in-app TLS, and path-traversal protection all cover the WebDAV door.
- [Interop & Client Apps](23-interop-and-clients.md) — WebDAV is a second "speak a standard protocol" door alongside the planned OPDS feed for book readers.
