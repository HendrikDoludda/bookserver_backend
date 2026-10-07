# Interop & Client Apps

> Speak one standard protocol (OPDS) so third-party reader apps can browse the server, and keep native iOS/Android apps parked until they pay for themselves.

**Status:** ⬜ not started · **Phase:** 2 · **Tier:** Parked — out of focus · **Tags:** [BE] [FE]
**Depends on:** [03 Authentication, Users & Sessions](03-auth-users-and-sessions.md) (OPDS needs an auth scheme a third-party reader can speak) · [01 Library & Directory Management](01-library-and-directory-management.md) (the feed is generated from libraries and series)
**Blocks:** Nothing
**Roadmap:** [Build order](../../ROADMAP.md#priority-order-build-sequence)

## Why
An OPDS catalog feed is the cheapest possible interop: one read-only feed and *standard reader apps* can browse the library without waiting on the custom frontend app. It is explicitly **secondary** to the custom app — the point is that it costs very little for a whole class of clients you otherwise would not support. Native iOS/Android apps are the opposite trade: real, ongoing effort per platform, so they are deferred until there is revenue to justify them, with manual installs on devices in the meantime. Both items are the same theme — how clients other than the web frontend reach this server.

## Current state
Nothing exists for either item.

- No OPDS code anywhere: a case-insensitive search for `opds` / `atom` / `xml` / `dav` / `mobile` / `flutter` / `infuse` across `src/`, [Cargo.toml](../../Cargo.toml) and `migrations/` returns zero hits — no route, no feed handler, no XML serializer crate.
- No mobile client code — this repo is the backend only.
- The data an OPDS feed would re-expose is already live as JSON: [get_libraries](../../src/routes/api_caller.rs#L247), [get_series_in_library](../../src/routes/api_caller.rs#L257), [get_series_children](../../src/routes/api_caller.rs#L268), [retrieve_metadata](../../src/routes/api_caller.rs#L291), and file delivery with HTTP range support via [request_file](../../src/routes/api_caller.rs#L279) → [streaming_file](../../src/stream_reader.rs#L10).
- Cover serving — which feed entries need for thumbnails — is only a route skeleton wired to the placeholder handler: [api_routes.rs:63](../../src/routes/api_routes.rs#L63) points `GET /get_cover/:kind/:id` at `ping_server` ([api_caller.rs:239](../../src/routes/api_caller.rs#L239)).
- Session auth reads cookies only ([AuthSession](../../src/routes/api_caller.rs#L80)), and no cookie layer is installed on the router ([api_routes.rs:45-70](../../src/routes/api_routes.rs#L45)), so there is currently no auth path a third-party client could use.

## Decisions
- **OPDS is cheap interop, not the primary client.** The feed is secondary to the custom frontend app, but worth building because it is cheap: standard reader apps get to browse the server for the price of one read-only feed. It is tagged **[BE]** only — there is no frontend piece, because the "UI" is whatever reader app the user already has.
- **Third-party clients get a protocol, not the REST API.** This is the same reasoning the video feature uses for Infuse: a third-party client can't talk to a custom REST API, so the backend also exposes the library over a protocol the client already understands — WebDAV for Infuse (see [Video Library](20-video.md)), OPDS for reader apps. One media source, several front doors.
- **OPDS was promoted out of the ideas backlog.** It started as a proposal in the roadmap's "Ideas to run by you" list and was explicitly promoted to Phase 2 (alongside incremental scanning being promoted to Phase 1), so it is a committed item rather than a maybe. See [Ideas Backlog](90-ideas-backlog.md).
- **Native mobile apps are deferred until there's revenue to justify the effort.** Building and maintaining iOS + Android apps is real per-platform cost with no return on a personal home server; until there is revenue, **manual install on devices** is the accepted distribution path.

## Open questions
- **Which OPDS version?** OPDS 1.2 is Atom XML and has by far the widest client support; OPDS 2.0 is JSON (nicer to produce from `serde`) but fewer readers speak it. *Suggested:* OPDS 1.2 — the entire value of this item is "works with apps the user already has", so compatibility beats elegance.
- **How does an OPDS client authenticate?** Options: HTTP Basic against the existing `users` table (what most reader apps support, and what the video/WebDAV plan already assumes — basic-auth, protected by Tailscale when remote); a long-lived per-device token in the feed URL (works everywhere, but a URL that leaks is a permanent credential); or no app-level auth at all, relying on Tailscale/LAN. *Suggested:* HTTP Basic verified against the same argon2 password hashes, as a small separate extractor, since the current session extractors are cookie-only.
- **How much of the app does the feed expose?** Just browse + download, or also read state / "plan to read" as OPDS shelves? *Suggested:* browse + download only — read state stays in the custom app, where the reader actually is.
- **What happens to sensitive libraries in the feed?** A third-party client bypasses every frontend control, so anything marked sensitive would be plainly visible unless the feed filters it. *Suggested:* exclude `is_sensitive` items from the OPDS feed by default once that flag exists ([Sensitive Content Controls](10-sensitive-content-controls.md)).
- **What actually un-defers the mobile apps?** "Revenue" is the stated trigger but not defined, and "manual install" means different things per platform (an Android APK sideloads freely; iOS needs a signing identity). *Suggested:* leave both deferred and revisit only once the web frontend is complete and someone other than you is asking for an app.

## Tasks
- ⬜ OPDS catalog feed **[BE]** — lets standard reader apps browse the server (secondary to the custom frontend app, but cheap interop)
- ⬜ iOS / Android apps — deferred until there's revenue to justify the effort; manual install on devices until then **[FE]**

## Done when
- [ ] A standard OPDS reader app, pointed at the feed URL with nothing but a URL and credentials, can browse libraries → series → books.
- [ ] Selecting a book in that client actually downloads/opens the file, served through the same range-supporting path as `GET /api/v1/book/{id}`.
- [ ] The feed is well-formed OPDS: it parses in at least two different reader apps (or an OPDS validator) with no hand-editing.
- [ ] A request to the feed URL without valid credentials is rejected rather than returning the catalog.
- [ ] Each feed entry carries a cover link that resolves to a real image once cover serving is implemented.

## Implementation notes
- **No OPDS or XML crate is in [Cargo.toml](../../Cargo.toml) today.** For OPDS 1.2 you either hand-render the Atom XML with `format!` (a feed is small and the shape is fixed) or add an XML serializer (`quick-xml` has serde support). [`serde_json`](../../Cargo.toml#L14) is already a dependency but unused in `src/`, and `axum::Json` won't help here — the response must be XML.
- **Rust shape for an XML response:** OPDS clients key off the content type, so the handler returns the body *plus* a header rather than a bare `String` — e.g. `-> impl IntoResponse` returning `([(header::CONTENT_TYPE, "application/atom+xml;profile=opds-catalog;kind=navigation")], body)`. Browse levels use `kind=navigation`; the level that lists downloadable books uses `kind=acquisition`.
- **Reuse the existing queries, add no new DB work.** [`get_all_libraries`](../../src/database_related_scripts/db.rs#L274), [`get_series_entries_in_library`](../../src/database_related_scripts/db.rs#L327), [`get_books_in_series`](../../src/database_related_scripts/db.rs#L289) and [`get_entries_batch`](../../src/database_related_scripts/db.rs#L181) already return exactly what the feed needs — OPDS is a second *representation* of the same data, which is the API-first principle applied to a third client.
- **Point download links at the existing streaming path** ([stream_reader.rs:10](../../src/stream_reader.rs#L10)), so HTTP range and conditional handling come free from `tower-http`'s `ServeFile` instead of being re-implemented for the feed.
- **Router path syntax:** the current route table still uses the axum 0.7 `:id` form ([api_routes.rs:48](../../src/routes/api_routes.rs#L48)) while [Cargo.toml:7](../../Cargo.toml#L7) pins axum 0.8, which rejects it. Any OPDS routes you add should use the brace form (`/opds/library/{id}`) — see [Cross-Cutting Backend Polish](07-cross-cutting-backend-polish.md).
- **Basic auth is a separate extractor.** Today's session extractors read the `session_token` / `refresh_token` cookies ([api_caller.rs:80-235](../../src/routes/api_caller.rs#L80)), so a reader app that only speaks HTTP Basic needs its own small `FromRequestParts` impl that reads the `Authorization` header and verifies the password against `users.password_hash` — do not widen the existing extractors to accept both.
- **Manual installs, concretely:** Android sideloading an APK is free; distributing an iOS build to a device outside the App Store needs a signing identity (and free-tier signing expires, forcing re-installs). That cost is part of why the apps are deferred.

## Related
- [Video Library (Phase 2)](20-video.md) — Infuse over WebDAV is this exact idea applied to video: expose a protocol the third-party client already understands instead of expecting it to learn the REST API.
- [Reading & Read State](02-reading-and-read-state.md) — the range-streaming endpoint OPDS download links reuse.
- [Library & Directory Management](01-library-and-directory-management.md) — cover serving, which feed thumbnails depend on.
- [Authentication, Users & Sessions](03-auth-users-and-sessions.md) — the credentials an OPDS client would present; today's sessions are cookie-only.
- [Browsing, Search & Discovery](11-browsing-search-and-discovery.md) — the same catalog data the feed re-exposes, in the app's own representation.
- [Cross-Cutting Backend Polish](07-cross-cutting-backend-polish.md) — the axum 0.8 path-syntax fix any new OPDS route waits on.
- [Sensitive Content Controls](10-sensitive-content-controls.md) — a third-party client bypasses frontend controls, so sensitive items need filtering at the feed.
- [Operations & Security](12-operations-and-security.md) — the LAN + Tailscale threat model that decides how exposed the feed actually is.
- [Download Permissions](24-download-permissions.md) — OPDS acquisition links are download links by definition, so a download-restricted user either gets a browse-only feed or no feed access at all.
- [Ideas Backlog](90-ideas-backlog.md) — where OPDS was proposed before being promoted into Phase 2.
