# OPDS catalog feed

State: not started

A read-only OPDS feed, so standard reader apps can browse the library and download books
without the custom app — cheap interop, secondary to the app. Reader apps can't speak the REST
API, so the backend speaks a protocol they already know, the way
[WebDAV](70-webdav-for-infuse.md) does for Infuse.

## Build
- [ ] ? h · Navigation feeds for libraries → series, per the version decision, built from the existing queries `get_all_libraries` and `get_series_entries_in_library` — no new database work [BE]
- [ ] ? h · Acquisition feed listing a series' books (`get_books_in_series`, `get_entries_batch`), with download links pointing at the existing range-streaming route `GET /api/v1/book/{id}` (download-restricted users: see [24](24-download-permissions.md)) [BE]
- [ ] ? h · Serve the feeds with the OPDS content type — `application/atom+xml;profile=opds-catalog;kind=navigation` for browse levels, `kind=acquisition` for book lists — from a handler returning the header plus the body; hand-render the XML with `format!` or add `quick-xml` (no XML crate is a dependency yet) [BE]
- [ ] ? h · A cover link on every feed entry — cover serving itself (`GET /get_cover/{kind}/{id}`) is still a `ping_server` placeholder, owned by [01](01-library-and-series-management.md) [BE]
- [ ] ? h · Authenticate feed requests, per the auth decision; a request without valid credentials gets no catalog [BE]
- [ ] ? h · Leave sensitive items out of the feed, per the sensitive-items decision [BE]
- [ ] ? h · Check by hand: the feed parses in at least two reader apps (or an OPDS validator), and a book chosen in one of them downloads and opens

## Open decisions
- [ ] OPDS 1.2 (Atom XML) or 2.0 (JSON)? — blocks: the navigation feeds. Suggested: 1.2 — far wider client support, and working with apps people already have is the whole point.
- [ ] How do reader apps authenticate? — blocks: authenticating feed requests. Suggested: HTTP Basic against the same Argon2 password hashes, as its own small `FromRequestParts` extractor — today's session extractors read cookies only; don't widen them.
- [ ] Browse and download only, or also read-state shelves? — blocks: the navigation feeds. Suggested: browse and download only.
- [ ] What happens to sensitive items? A reader app bypasses every frontend control — blocks: leaving sensitive items out. Suggested: exclude `is_sensitive` items by default once that flag exists ([10](10-sensitive-content-controls.md)).
