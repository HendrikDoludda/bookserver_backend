# Backend Admin Web View

> An admin web UI served by the backend itself, for managing libraries, series, directories, users and scans — built as just another client of the same HTTP API.

**Status:** ⬜ not started · **Phase:** 1 · **Tier:** Backend Tier 4 (second item, after uploads) · **Tags:** [BE] [FE]
**Depends on:** [03 Authentication, Users & Sessions](03-auth-users-and-sessions.md) (the admin UI is admin-only, so it needs roles) · [13 Internationalization (i18n)](13-internationalization.md) (its text must go through t("key") from the first screen, against the same backend-owned catalog)
**Blocks:** [08 Theming & Branding](08-theming-and-branding.md) (only for the backend-served branding option, which exists so the admin UI can edit it)
**Roadmap:** [Build order](../../ROADMAP.md#priority-order-build-sequence)

## Why

The roadmap's guiding principle is that **everything must be changeable from both the frontend and the backend web view**. The only way to actually guarantee that is API-first: every mutation is an HTTP endpoint, and both the frontend and the backend admin UI are just *clients* of that same API. The admin web view is the second half of that promise — the place an admin can run the server (scans, directories, users, translations, branding) without the main app. It also exists to keep the principle honest: if a behaviour only works in one UI, the principle is broken.

## Current state

Nothing exists yet. The backend serves no HTML at all.

- **No static/HTML serving anywhere.** A grep of `src/` for `ServeDir`, `nest_service`, `fallback_service`, `index.html` and `Html` returns zero hits. The only `tower-http` service in use is `ServeFile` for book streaming — [stream_reader.rs:8](../../src/stream_reader.rs#L8), [stream_reader.rs:10-13](../../src/stream_reader.rs#L10). There is no frontend or static-assets directory anywhere in the repo.
- **The router is API-only.** [api_routes.rs:45-70](../../src/routes/api_routes.rs#L45): 19 route registrations nested under `/api/v1` ([api_routes.rs:69](../../src/routes/api_routes.rs#L69)), plus `/health` at the root ([api_routes.rs:68](../../src/routes/api_routes.rs#L68)) — 20 in total. Router state is `Arc<Database>`, applied in `start_server` ([api_routes.rs:21](../../src/routes/api_routes.rs#L21)).
- **No middleware at all.** There is no `.layer(...)` call anywhere in `src/` — no CORS, no `CookieManagerLayer`, no trace layer, no body-size limit.
- **The admin gate is drafted but unwired.** `AdminSession` — [api_caller.rs:60-78](../../src/routes/api_caller.rs#L60) — delegates to `AuthSession`, loads the `UserMetadata` row, and returns `RequestErrors::Forbidden` when `!user.is_admin`; that maps to HTTP 403 in [error_types.rs:203-218](../../src/error_types.rs#L203). **Zero routes use it.** The backing column exists: `users.is_admin BOOLEAN NOT NULL DEFAULT FALSE` — [0001_initial_schema.sql:56](../../migrations/0001_initial_schema.sql#L56).
- **Most endpoints this UI would drive are stubs.** `update_entry` / `insert_entry` / `scan_metadata` are `AuthSession`-gated hollow handlers that return HTTP **200** with the body `"Failed to get response"` — [api_caller.rs:328-344](../../src/routes/api_caller.rs#L328). `/login`, `/register_user`, `/log_out`, `/scan_directory/:id`, `/get_cover/:kind/:id`, `/scan/status`, `/search` and `/reading_progress/:id` are route skeletons bound to the placeholder handler `ping_server` ([api_routes.rs:57-65](../../src/routes/api_routes.rs#L57), handler at [api_caller.rs:239-241](../../src/routes/api_caller.rs#L239)). Note the login path this UI needs is a *wiring* gap, not a missing implementation: `sign_up` ([api_caller.rs:346](../../src/routes/api_caller.rs#L346)), `sign_in` ([api_caller.rs:409](../../src/routes/api_caller.rs#L409)) and `log_out` ([api_caller.rs:480](../../src/routes/api_caller.rs#L480)) exist but are attached to no route.
- **Live endpoints an admin UI could already call today:** `get_libraries`, `get_series_from_library/:id`, `get_series_children/:id`, `book_metadata/:id`, `book/:id`, `setup_db`, `scan_all_directories`, `delete_library/:id`.
- **No i18n anything.** No `locales` / `translation_keys` / `translations` tables, no `GET /i18n/:locale`. The only trace in the codebase is a TODO at [auth.rs:171](../../src/routes/auth.rs#L171) ("Text must be replaced with the i18n text").
- **Two blockers sit in front of everything here:** the crate does not compile (5 empty-bodied TOTP handlers, [api_caller.rs:617-625](../../src/routes/api_caller.rs#L617)), and even once it does, `set_up_routes()` panics at startup because axum 0.8 rejects the 0.7 `:id` path syntax used on 8 routes. See [Cross-Cutting Backend Polish](07-cross-cutting-backend-polish.md).

**Roadmap drift:** §5's own three markers are correct — all ⬜, nothing built. But the *admin gate* this UI depends on is further along than §3's markers suggest: §3 lists "Roles enforced (admin vs member) on protected routes" as ⬜ while an `AdminSession` extractor and the `is_admin` column already exist — they are simply attached to no route.

## Decisions

- **API-first — the admin UI is just another client.** Every mutation is an HTTP endpoint; the frontend and the admin UI both call it. Rationale: this is the only way to actually *guarantee* the "changeable from both sides" principle. If any behaviour lives only inside one UI, the principle is broken. Keep business logic in the backend, not in either UI. Concretely for this feature: the admin UI must not get a private endpoint or a private code path — anything it can do, the main frontend could do too.
- **Built in Tier 4, in the order uploads → admin UI (§5) → theming → sensitive content.** The roadmap states the sequence; the only reason it gives is the Tier 1 dependency note, which says outright that the admin UI waits on auth & users. *Inferred, not stated in the roadmap:* it also exists to drive the Tier 2 CRUD, and it lands before theming and sensitive content because both are admin-editable surfaces that need somewhere to live.
- **Waits on auth (§3).** The Tier 1 dependency note names the admin UI alongside read state (§2), uploads (§4) and sensitive content (§9) as things blocked behind auth & users.
- **i18n discipline from day one — `t("key")` when this UI is first built.** A language select in the admin UI too, so it's localizable like the main app. Rationale (§12 timing note): retrofitting i18n into a frontend full of hardcoded strings is exactly the "complete rework" to avoid; wrap the text even while only English exists, and adding languages later is catalog edits, not a rework.
- **Resolved: same backend-owned catalog as the main app — no separate locale files.** The admin UI fetches from the same backend-owned catalog (§12) as the main app: one source of truth. Both clients just call the i18n API.
- **The admin UI *is* the catalog editor (§12).** Key ownership is split: **code owns keys, admins own values** — the admin panel adds *languages* and fills in *values*, it does not invent or delete keys, so the catalog can never drift out of sync with what the code references. The screen layout mirrors the data model: pick/add a **language** (display name + icon, enabled flag, default flag, sort order); **tabs = namespaces**; each tab lists its **keys as input boxes**. An untranslated box shows the default-language (English) text as placeholder for context, and the same `LEFT JOIN` that powers that gives a free "N/M translated" progress view per language. Every save bumps the catalog version. Full detail lives in [Internationalization (i18n)](13-internationalization.md).
- **CORS is scoped to browser clients only.** §11 defers CORS configuration until a browser client (web frontend *or* backend web view) actually calls the API — it is not needed before then.

## Open questions

- **How is the UI built and shipped?** Options: (a) a directory of static files served with `ServeDir` — easiest to iterate, but the files must ship next to the binary and the path is relative to the process CWD; (b) assets embedded into the binary (`rust-embed` / `include_dir`) — one self-contained executable, which fits §11's packaging goal (Windows `.exe`, macOS `.dmg`, Docker); (c) server-rendered HTML from Axum (`askama` / `maud`) — no build step, but it makes the "admin UI is just another API client" principle much easier to violate. *Suggested:* develop against (a) and switch to (b) before packaging — the mount point is the same either way; avoid (c) so the principle stays enforceable.
- **One frontend codebase or two?** §12 refers to "both frontends (main app + admin UI §5)" but never says whether they share a project. Sharing gives one i18n wiring, one design-token consumption (§7) and one auth flow; separate keeps the admin bundle out of the user-facing app. *Suggested:* keep them separate apps but share the conventions (same `t("key")` calls, same tokens, same auth mechanism), since the admin UI is a small management surface, not a second product.
- **Route prefix and SPA fallback vs `/api/v1`.** A client-routed UI needs a fallback so deep links reload correctly, and a careless global fallback will swallow unknown `/api/v1/...` paths and answer them with HTML instead of a 404. *Suggested:* mount the UI under an explicit prefix (e.g. `/admin`) with its own fallback, leaving `/api/v1` and `/health` untouched.
- **Does the admin UI need CORS at all?** Served from the backend's own origin it is same-origin, so only a separately hosted main frontend needs CORS headers. *Suggested:* scope CORS to the main frontend's origin and don't loosen it just because the admin UI exists — see [Operations & Security](12-operations-and-security.md).
- **Which auth mechanism does a browser admin UI use?** §3 decided "client sends the token in the `Authorization` header", but the code as written reads cookies: every extractor looks for `session_token` / `refresh_token` ([api_caller.rs:99](../../src/routes/api_caller.rs#L99)) while `create_auth_response` returns the tokens in a JSON body and sets no cookie ([api_caller.rs:643-658](../../src/routes/api_caller.rs#L643)) — and no `CookieManagerLayer` is installed, so the cookie path cannot work today at all. A same-origin browser UI is the strongest argument for HttpOnly cookies (no token handling in JS). *Suggested:* settle this in [Authentication, Users & Sessions](03-auth-users-and-sessions.md) before building this UI, and whichever wins, use the *same* mechanism in both clients.
- **What does "manage users" actually need?** §5 asks for user management, but §3 only defines admin-only registration — there are no list-users, edit-user, change-role or revoke-session endpoints specified. *Suggested:* track the missing endpoints (list users, admin-only create-user, role change, plus §11's "active sessions" list + revoke) in §3 rather than inventing them here; this doc's user screen is a client of whatever §3 lands.
- **Where does branding live?** §7's open decision — frontend-owned tokens file vs backend-served `GET /branding` — determines whether the admin UI gets a branding screen at all. The backend-served option is the one that fits "changeable from both sides". *Suggested:* decide it in [Theming & Branding](08-theming-and-branding.md); if it goes backend-served, a branding screen joins this UI's task list.

## Tasks

- ⬜ Serve an admin web UI from the backend **[BE][FE]**
- ⬜ Manage libraries / series / directories / users / scans from it **[FE]**
- ⬜ **Language select in the admin UI too** (§12) — same i18n discipline from the start: wrap text in `t("key")` when this UI is first built, so it's localizable like the main app **[FE]**
  - *Resolved:* the admin UI fetches from the **same backend-owned catalog** as the main app (§12) — one source of truth, no separate locale files. Both clients just call the i18n API.

## Done when

- [ ] Opening the admin UI's prefix (e.g. `/admin`) on the running server returns its index page, and a deep link like `/admin/libraries` reloads to the same app instead of 404ing — while `/api/v1/does_not_exist` still returns a 404 from the API, not HTML.
- [ ] A signed-in **non-admin** session calling an admin-only endpoint gets **403** (via `AdminSession`), and the admin UI renders a refused state rather than a blank screen.
- [ ] Creating a library from the admin UI and creating one from the main frontend hit the *same* endpoint and produce the same row — no endpoint exists that only the admin UI can call.
- [ ] Grepping the admin UI's source for user-facing string literals finds none: every visible string goes through `t("namespace.key")`.
- [ ] Switching the admin UI's language select to a second locale re-renders it from `GET /i18n/:locale`, with no locale file of its own anywhere in the admin UI source.
- [ ] Editing a translation value in the admin UI and then reloading the main frontend shows the new string — proving one catalog, two clients.

## Implementation notes

- **No new dependency needed for static serving.** `tower-http` is already pulled with `features = ["full"]` ([Cargo.toml:40](../../Cargo.toml#L40)), so `tower_http::services::ServeDir` is available. The SPA shape is `ServeDir::new(dir).fallback(ServeFile::new(index_html))`, mounted with `Router::nest_service("/admin", service)`.
- **Rust shape / router-state gotcha.** `set_up_routes()` returns `Router<Arc<Database>>` and the state is attached later in `start_server` ([api_routes.rs:21](../../src/routes/api_routes.rs#L21)). `nest_service` takes a tower `Service`, not a handler, so a static-file service carries no state and should be nested on the outer router (the one that already holds `/health` and `.nest("/api/v1", api)`) — you do not have to thread `Arc<Database>` through it.
- **Reuse `AdminSession`, don't re-check `is_admin` per handler.** It already exists at [api_caller.rs:60-78](../../src/routes/api_caller.rs#L60) and rejects with `RequestErrors::Forbidden` → 403. Adding it as the first argument of an admin-only handler is the whole gate.
- **Two hard blockers first.** The crate does not compile (5 errors: `-> Response<Body>` with empty bodies at [api_caller.rs:617-625](../../src/routes/api_caller.rs#L617)), and `Router::route` panics on the axum-0.8 path check for the 8 routes still written as `:id` — the correct form is `{id}` / `{kind}`. Nothing in this doc is reachable until both are fixed ([Cross-Cutting Backend Polish](07-cross-cutting-backend-polish.md)).
- **Cookie auth is currently non-functional** — no `CookieManagerLayer` is installed anywhere, so `Cookies::from_request_parts` fails and every session-gated route answers 400. If the admin UI is to authenticate with cookies, that layer has to be added first.
- **The i18n endpoint should already exist by the time this is built.** The frontend track's **F0** step builds the §12 catalog plus `GET /i18n/:locale` (with section scoping and version/ETag) right after backend Tier 1 — before the first frontend screen — so the admin UI can call a real endpoint from its own first screen too.
- **`ServeDir` paths are CWD-relative** — the same trap `COVER_DIR = "./cover_images/"` already has in the scanner. If the UI directory is configurable, read it the way the other runtime config is read (env var in [config.rs](../../src/config.rs)) rather than hard-coding a relative path.
- **Keep logic out of the UI.** When a screen needs something the API can't express, add the endpoint — do not add a shortcut inside the admin UI. That is the single rule that keeps the guiding principle true.

## Related

- [Authentication, Users & Sessions](03-auth-users-and-sessions.md) — supplies the admin role, the session extractor and the login flow this UI is gated behind.
- [Internationalization (i18n)](13-internationalization.md) — owns the catalog, `GET /i18n/:locale`, and the data model this UI's translation screen edits.
- [Library & Directory Management](01-library-and-directory-management.md) — the create/edit library & series and scannable-directory endpoints this UI drives.
- [Uploads & File Ingestion](05-uploads-and-ingestion.md) — the Tier 4 item built immediately before this one.
- [Theming & Branding](08-theming-and-branding.md) — its open decision (frontend file vs backend-served `GET /branding`) decides whether this UI gets a branding screen.
- [Sensitive Content Controls](10-sensitive-content-controls.md) — the admin-only sensitive toggles live on library & series screens in this UI.
- [Metadata Enrichment (Online Sources)](09-metadata-enrichment.md) — manual edit/override of fetched fields is an admin-facing screen ("auto-matching is fuzzy — let the admin fix it").
- [Operations & Security](12-operations-and-security.md) — CORS becomes relevant once a browser client calls the API; TLS and packaging affect how this UI is served.
- [Cross-Cutting Backend Polish](07-cross-cutting-backend-polish.md) — the compile + route-syntax blockers, and the sorting/pagination the management lists will want.
