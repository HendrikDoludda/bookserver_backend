# Metadata Enrichment (Online Sources)

> Pull extra info (descriptions, covers, tags, release years, authors) from online databases, through one `MetadataProvider` abstraction whose adapter is chosen by the library's type.

**Status:** ⬜ not started · **Phase:** 1 · **Tier:** Backend Tier 3 (item #11, last in the tier) · **Tags:** [BE] [DB] [FE]
**Depends on:** [01 Library & Directory Management](01-library-and-directory-management.md) (providers are routed by library type, so libraries must be typed and manageable)
**Blocks:** [10 Sensitive Content Controls](10-sensitive-content-controls.md) (the "sensitive items skip online lookups" rule needs provider dispatch to exist)
**Roadmap:** [Build order](../../ROADMAP.md#priority-order-build-sequence)

## Why

The scanner only ever learns what a filename and the filesystem can tell it — size and modified time are real, and everything else is hardcoded placeholder text. Books land in the DB with `author: "Unknown Author"` and `language: Other("Unkown")` ([folder_scanner.rs:192-193](../../src/scanner/folder_scanner.rs#L192)), and scanner-created series get `description: None`, `cover_image: None` and no release years at all ([folder_scanner.rs:321-327](../../src/scanner/folder_scanner.rs#L321)). Online databases fill exactly those gaps, and doing it per content type matters because a manga, a comic and a novel are catalogued by completely different services. Enrichment also feeds the things built on top of metadata — tag/author search and filters ([Browsing, Search & Discovery](11-browsing-search-and-discovery.md)) are only as good as the tags and authors in the rows.

## Current state

- `POST /api/v1/scan_metadata` is registered at [api_routes.rs:52](../../src/routes/api_routes.rs#L52) and points at a real, named handler — but the handler is hollow: `scan_for_metadata` takes an `AuthSession` and immediately `return failed_getting_response()` ([api_caller.rs:338-340](../../src/routes/api_caller.rs#L338)). `failed_getting_response()` is `Response::new("Failed to get response".into())` ([api_caller.rs:342-344](../../src/routes/api_caller.rs#L342)) — i.e. **HTTP 200** with a failure string in the body. Unlike its two siblings (`update_database_entry`, `insert_database_entry`) it carries no comment saying what blocks it.
- [src/scanner/metadata_fetcher.rs](../../src/scanner/metadata_fetcher.rs) is a **0-byte file** — not a comment-only stub, literally empty. It is declared as a module at [scanner/mod.rs:2](../../src/scanner/mod.rs#L2) and nothing imports from it.
- The metadata-agent link is a live TODO in the code, at the top of the handler file: `//TODO: The table still needs a link for the metadata agent result` ([api_caller.rs:3](../../src/routes/api_caller.rs#L3)).
- No storage exists for an external source id. `books` ([0001_initial_schema.sql:2-20](../../migrations/0001_initial_schema.sql#L2)) and `series` ([0001_initial_schema.sql:41-48](../../migrations/0001_initial_schema.sql#L41)) have no provider/external-id column, and neither do `BookDatabaseColumns` ([models.rs:182](../../src/data_models/models.rs#L182)) or `SeriesDatabaseColumns` ([models.rs:235](../../src/data_models/models.rs#L235)). There is no provider table, no API-key storage and no cache table.
- The routing key is only half there: `LibraryType` has `Books | Comics | Magazines | Documents | Others` ([models.rs:51-58](../../src/data_models/models.rs#L51)) — **no `Manga` variant** — and the scanner hardcodes `LibraryType::Books` for every library it auto-creates ([folder_scanner.rs:363](../../src/scanner/folder_scanner.rs#L363)).
- No HTTP client crate is declared. [Cargo.toml](../../Cargo.toml) has no `reqwest` (or any other client); the only outbound network code today is SMTP via `mail-send`.
- Nothing here is reachable at runtime today for reasons unrelated to this feature: the crate does not compile ([api_caller.rs:617-625](../../src/routes/api_caller.rs#L617)) and `set_up_routes` would then panic on Axum 0.8's rejection of `:id` paths ([api_routes.rs:48](../../src/routes/api_routes.rs#L48)) — both tracked in [Cross-Cutting Backend Polish](07-cross-cutting-backend-polish.md).

**Roadmap drift:** the 🟡 on "Scaffolding exists: `scan_for_metadata` endpoint stub + `scanner/metadata_fetcher.rs`" is generous. The scaffolding is a route name, a handler that returns 200 with the string "Failed to get response", and a zero-byte module file — there is no code for this feature at all, so this doc records the feature as ⬜ not started with that scaffolding listed above. The Tier 3 build-order note ("scaffolding … already exists") reads the same way and is true only as a route name plus an empty module file.

## Decisions

- **One `MetadataProvider` abstraction, one adapter per source, routed by library type.** Different content types need different providers, so the router picks the adapter from the library's `library_type` rather than from a per-request parameter. This keeps the provider choice a property of the library (data), not of the caller.
- **Provider list per content type (settled):**
  - **Books** → Google Books / Open Library
  - **Manga** → AniList / MangaDex / MangaUpdates
  - **Comics** → Comic Vine / Metron
- **Search → candidates → confirm / auto-match → merge.** Enrichment is a search by title that returns candidate matches, then either a confirmation step or an auto-match, and only then a merge into the entry. The candidate step exists because title matching is fuzzy and a silent wrong match is worse than no match.
- **Store the external source id on the entry.** Keeping the provider's id means an entry can be re-fetched or updated later without repeating the fuzzy search. This is the item the existing code TODO is asking for ("the table still needs a link for the metadata agent result").
- **Manual edit/override of any fetched field is required, not optional.** Auto-matching is fuzzy, so the admin must be able to fix any field the provider filled in.
- **API keys, rate limiting and result caching are part of the feature, not an afterthought.** External services are metered and throttled; caching results and respecting rate limits is what makes repeated scans viable.
- **Sensitive items skip all online lookups.** A library or series marked sensitive must never have its title sent to a third-party API (privacy). The flag and toggle belong to [Sensitive Content Controls](10-sensitive-content-controls.md); the exclusion check belongs here.
- **Video metadata enrichment is a deliberately separate Phase 2 item.** TMDB / TVDB-style enrichment (posters, synopsis, cast, episode/season data) is kept distinct from this feature because video needs *different providers* and a *movie/show data model*, **and** because it depends on the video feature (Phase 2 → Video) existing first — but it should **reuse this `MetadataProvider` abstraction, routed by content type**. See [Video Library (Phase 2)](20-video.md).
- **Position in the build order: Tier 3, item 11.** It sits after auth (Tier 1) and core CRUD (Tier 2) and is the last item of the "makes it feel like a product" tier. The gating rule applies: finish every item in a tier before moving on.

## Open questions

- **`LibraryType` has no `Manga` variant, yet manga has its own provider list.** Options: add `Manga` to the enum (`library_type` is nullable TEXT and `LibraryType::from_str` maps anything unknown to `Others`, [models.rs:61-69](../../src/data_models/models.rs#L61), so no migration is strictly needed — but existing rows created by the scanner all say `books`); or route manga off something else (a per-library provider override column). *Suggested:* add a `Manga` variant to `LibraryType`, since the whole routing decision keys off library type and the provider split is already decided along that line.
- **Where does the external source id live — `books`, `series`, or both?** Series-level is where most provider data actually sits (AniList/MangaDex/Comic Vine describe a series, not a single volume file), but per-book ids would let a single volume be matched individually. *Suggested:* put it on `series` first (that is where descriptions, covers and release years land), and add a book-level column only if per-volume matching turns out to be needed.
- **One provider per library type, or a fallback chain?** The decided lists have two to three providers per content type. Options: a single configured provider per library type (simplest); an ordered chain where the next provider is tried if the first returns no candidates; or merging candidates from several providers into one list. *Suggested:* one configured provider per library type to start, with the abstraction shaped so a chain can be added without changing call sites.
- **When is auto-match allowed instead of a confirmation?** The roadmap says "confirm / auto-match" without a threshold. Options: always require confirmation; auto-match only on an exact normalised title match; auto-match above a similarity score. *Suggested:* require confirmation for anything but an exact normalised-title single hit, and log every auto-match.
- **Where do API keys live?** Options: env vars (matches the existing `SMTP_*` / `BOOK_DIRS` pattern, [config.rs](../../src/config.rs)); `secrets.toml` (the existing secret store, `set_up_config_file`); or a DB table the admin UI can edit — which is what the guiding principle ("changeable from both frontend and backend web view") points at. *Suggested:* DB-backed so the admin UI can manage them, with the file/env path kept as the bootstrap fallback.
- **Cache location and lifetime.** Options: a DB table keyed by (provider, query/external id) which survives restarts and is inspectable; or an in-memory map, which is simpler but re-hits the provider on every restart. *Suggested:* a DB table — a scan re-runs often and restarts are frequent in development.
- **What does "merge" overwrite?** The scanner writes real placeholder values (`"Unknown Author"`, `Other("Unkown")`), so "only fill empty fields" would never overwrite them, while "always overwrite" would clobber a manual override. *Suggested:* treat the known scanner placeholders as empty, and never overwrite a field the admin has manually edited — which implies tracking per-field provenance (or at least a "manually edited" marker) when the override task is built.
- **`books.tags` is a comma-joined TEXT blob, not a relation** ([insert.rs:42](../../src/database_related_scripts/insert.rs#L42)). Merging provider tags means splitting, de-duplicating and re-joining a string. *Suggested:* keep the blob for now (a tags relation is a bigger change that also touches the FTS5 plan in [Browsing, Search & Discovery](11-browsing-search-and-discovery.md)), and put the split/merge/join logic in one helper.
- **Ordering conflict: the sensitive-content skip rule lands in Tier 4, but enrichment is Tier 3.** Enrichment could therefore ship before an `is_sensitive` flag exists, meaning every title is eligible to be sent to a third-party API in the meantime. Options: pull just the `is_sensitive` column forward into this feature's migration; or accept the gap until Tier 4. *Suggested:* add the column with this feature's migration and honour it from the first request, even if the admin toggle UI comes later.
- **Is `POST /scan_metadata` a bulk scan or a single-entry lookup?** Today it takes no path parameter and no body ([api_routes.rs:52](../../src/routes/api_routes.rs#L52), [api_caller.rs:338](../../src/routes/api_caller.rs#L338)), so the shape is undecided. The confirm-then-merge flow needs at least a search call and a confirm/merge call. *Suggested:* keep `POST /scan_metadata` as the bulk/background pass and add explicit per-entry search and confirm endpoints for the interactive flow.
- **The stub currently answers HTTP 200 with "Failed to get response".** Whatever replaces it should return a real status; this is the same class of problem as the generic-error-body work in [Cross-Cutting Backend Polish](07-cross-cutting-backend-polish.md). *Suggested:* until the handler is real, make it return 501 the way `reset_password` does.

## Tasks

### Foundation — schema and abstraction

- ⬜ Scaffolding exists: `scan_for_metadata` endpoint stub + `scanner/metadata_fetcher.rs` (see Current state — the module file is empty and the stub returns 200)
- ⬜ Store the external source id on the entry so it can be re-fetched/updated later **[DB]** (code already TODOs "a link for the metadata agent result")
- ⬜ `MetadataProvider` abstraction — one adapter per source, routed by library type **[BE]**
- ⬜ API keys + rate limiting + result caching for external services **[BE]**

### Provider adapters (per content type)

- ⬜ Books adapter(s) — Google Books / Open Library **[BE]**
- ⬜ Manga adapter(s) — AniList / MangaDex / MangaUpdates **[BE]**
- ⬜ Comics adapter(s) — Comic Vine / Metron **[BE]**

### Match, merge and override

- ⬜ Search-by-title → candidate matches → confirm / auto-match → merge into the entry **[BE][FE]**
- ⬜ Replace the `scan_for_metadata` stub with a real handler over the above (and give it a real status code) **[BE]**
- ⬜ Manual edit/override of any fetched field **[FE]** (auto-matching is fuzzy — let the admin fix it)

## Done when

- [ ] A library typed as manga and a library typed as books resolve to different provider adapters through the same `MetadataProvider` call site, with no branching in the caller.
- [ ] Searching an existing series by title returns a list of candidate matches from the provider for that library's type, with no write to the database.
- [ ] Confirming a candidate writes the fetched description / cover / tags / release years / author onto the entry **and** stores the provider's external id, so a second run re-fetches by id instead of searching by title.
- [ ] A field edited by hand after a merge is not overwritten by a subsequent enrichment run.
- [ ] Two identical enrichment requests in a row make only one outbound HTTP call (the second is served from the cache), and exceeding a provider's rate limit produces a logged, non-500 outcome rather than a hammered API.
- [ ] A series or library marked sensitive produces zero outbound requests — verifiable from the logs — while enrichment still works for a non-sensitive sibling.

## Implementation notes

- **Adding an HTTP client is the first concrete step** (see Current state). `serde_json` (1.0.151) is already declared in [Cargo.toml](../../Cargo.toml) but unused anywhere in `src/`, so JSON response parsing needs no dependency beyond the client itself. *Suggestion:* pick the client's `rustls` features rather than native-tls, to stay consistent with the `axum-server`/`tls-rustls` choice already made for HTTPS in [Authentication, Users & Sessions](03-auth-users-and-sessions.md).
- **The Rust shape for the abstraction** is a trait with `async` methods plus a small factory. Because `async fn` in traits does not give you an object-safe trait, the usual pattern in a codebase like this is either `#[async_trait]` (an extra crate) or making the trait's methods return `Pin<Box<dyn Future<...>>>`; the simplest alternative that avoids both is an `enum Provider { GoogleBooks(..), AniList(..), ComicVine(..) }` with a match in one `search`/`fetch` method — the same style the codebase already uses for `LibraryType` / `BookFormat` ([models.rs:51](../../src/data_models/models.rs#L51), [models.rs:83](../../src/data_models/models.rs#L83)). Start with the enum; convert to a trait only if the number of adapters makes the match unwieldy.
- **Follow the existing DB conventions for anything new.** A new column means a `*DatabaseColumns` enum variant plus its `AsRef<str>` arm; a new table means a struct with `FromRow`, and the `Insert` / `Update` / `Search` impls it actually needs. Note the existing trap: `Extract` and `Update` hard-code `WHERE id = ?`, so any new table whose primary key is not literally named `id` cannot use them — name the PK `id` unless you have a reason not to.
- **Migration practice during early dev** is to edit `0001_initial_schema.sql` in place and wipe the database, rather than adding `0002`. Migrations are baked in with `include_str!` and registered in the `MIGRATIONS` array in [migrations.rs](../../src/database_related_scripts/migrations.rs); never edit a migration that has already been applied to a database you intend to keep.
- **A merge touches several columns (and possibly a series plus its books), so it wants a transaction.** *Suggestion:* one transaction per merged entry. `Database::setup_new_transaction` ([db.rs:388](../../src/database_related_scripts/db.rs#L388)) gives a closure over a `&Connection` and pairs with the `*_with_connection` variants of each DB method. Gotcha: it swallows the closure's specific error and returns `DatabaseError::OperationFailure`, so the real reason only survives in the log — that flattening is tracked in [Cross-Cutting Backend Polish](07-cross-cutting-backend-polish.md).
- **Don't do network I/O inline in the request path** the way email sending currently does ([api_caller.rs:366](../../src/routes/api_caller.rs#L366)) — an unresponsive provider would stall the HTTP response. A bulk `scan_metadata` pass in particular wants the same background-job + status treatment that `scan_all_directories` still lacks (`GET /scan/status` is a placeholder handler).
- **Rate limiting here is per outbound provider, not per inbound client** — a different thing from the login/TOTP throttles in [Operations & Security](12-operations-and-security.md). Providers publish their own limits (Comic Vine in particular is strict); respect the documented limit per provider rather than one global number.
- **API keys are secrets.** If they end up in `secrets.toml`, note that the file is still written with default permissions — the owner-only `0600` hardening is an open task in [Operations & Security](12-operations-and-security.md). Never log a key.
- **Fetched covers need somewhere to live.** Extracted covers go to `./cover_images/` (`COVER_DIR`, [cover_image_retriever.rs:13](../../src/scanner/cover_image_retriever.rs#L13)), named by file hash. *Suggestion:* write provider covers to the same directory and reference them the same way, so the (still stubbed) `GET /get_cover/:kind/:id` endpoint serves both without special-casing.
- **`series.description` is `NOT NULL`** in the schema while `BookSeriesMetadata.description` is `Option<String>` (the `Insert` impl compensates with `.unwrap_or_default()`, [insert.rs:73](../../src/database_related_scripts/insert.rs#L73)) — worth knowing before you write a fetched description through that path.
- **The route is already auth-gated.** `scan_for_metadata` takes an `AuthSession` ([api_caller.rs:338](../../src/routes/api_caller.rs#L338)), which cannot succeed today because no `CookieManagerLayer` is installed on the router. Enrichment is an admin-shaped action, so `AdminSession` ([api_caller.rs:60](../../src/routes/api_caller.rs#L60)) is likely the right extractor once roles are enforced.

## Related

- [Library & Directory Management](01-library-and-directory-management.md) — library types are the routing key, and library/series create-edit is what lets a library be typed correctly in the first place; series & library covers overlap with fetched covers.
- [Authentication, Users & Sessions](03-auth-users-and-sessions.md) — Tier 1 gate; the endpoint is session-gated and the "admin fixes a bad match" flow needs roles.
- [Sensitive Content Controls](10-sensitive-content-controls.md) — owns the `is_sensitive` flag and toggle; this feature owns honouring it so titles never leak to third-party APIs.
- [Browsing, Search & Discovery](11-browsing-search-and-discovery.md) — consumes what enrichment writes: tags, authors and descriptions are what FTS5 search and the tag/author filters run over.
- [Uploads & File Ingestion](05-uploads-and-ingestion.md) — the post-upload pipeline reuses the scanner pipeline, so it is the natural second trigger point for enrichment.
- [Backend Admin Web View](06-admin-web-view.md) — where the confirm-a-candidate and manual-override screens live on the backend side.
- [Cross-Cutting Backend Polish](07-cross-cutting-backend-polish.md) — the stub's wrong status code, the generic-error-body rule, and the transaction error flattening.
- [Operations & Security](12-operations-and-security.md) — secrets-file permissions for API keys, and the separate inbound rate-limiting work.
- [Video Library (Phase 2)](20-video.md) — video metadata enhancement (TMDB / TVDB) is a deliberately separate item that reuses this `MetadataProvider` abstraction routed by content type.
