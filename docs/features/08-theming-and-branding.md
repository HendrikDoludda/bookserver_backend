# Theming & Branding

> Keep the app visually re-skinnable from one place — corner rounding, color scheme(s), fonts, app name, logo, etc. — without editing components.

**Status:** ⬜ not started · **Phase:** 1 · **Tier:** Backend Tier 4 (third item, after the admin UI) · **Tags:** [FE] [BE?]
**Depends on:** [06 Backend Admin Web View](06-admin-web-view.md) (only for the backend-served branding option, which exists so the admin UI can edit it)
**Blocks:** Nothing
**Roadmap:** [Build order](../../ROADMAP.md#priority-order-build-sequence)

## Why
The whole point is one place to change the look. If a color, a corner radius, or the app name is typed into components, re-skinning means editing every screen — so the tokens have to exist before the UI grows, not after. The app name, logo and favicon belong in that same single source, because they are the other half of "what this install looks like". And because the project's guiding principle is that everything must be changeable from **both** the frontend and the backend web view, where that single source *lives* is a real decision, not a detail.

## Current state
Nothing for this feature exists — not live, not a route skeleton, not a comment-only stub. A repo-wide search for `branding`, `theme`, `favicon` or design tokens returns nothing in `src/`, `Cargo.toml` or `migrations/`.

- No frontend exists in this repository. [lib.rs](../../src/lib.rs) declares only backend modules (config, data models, DB scripts, error types, stream reader, utils, scanner, routes) — there is no CSS, no component tree, no static-asset directory, and no admin UI to serve one from.
- No `/branding` route. The route table at [api_routes.rs:45-70](../../src/routes/api_routes.rs#L45) has 20 route registrations; none is branding- or theme-related, and no handler serves configuration of any kind.
- No configurable app name / logo / favicon anywhere. [config.rs](../../src/config.rs) reads only `PORT` ([config.rs:61](../../src/config.rs#L61)), `BOOK_DIRS` ([config.rs:68](../../src/config.rs#L68)), `API_KEY` ([config.rs:76](../../src/config.rs#L76) — dead, never called) and the `SMTP_*` group ([config.rs:80](../../src/config.rs#L80)). The `secrets.toml` file it manages holds only signing keys ([config.rs:13-18](../../src/config.rs#L13)).
- No branding table or column. Migration [0001_initial_schema.sql](../../migrations/0001_initial_schema.sql) creates ten tables (books, library, library_elements, series, users, email_verification, reset_password_requests, sessions, recovery_codes, totp) — none for settings or branding — and `DatabaseTypes` ([models.rs:17-26](../../src/data_models/models.rs#L17)) has no variant that could address one.
- The nearest existing precedent for a server-owned image asset with a default fallback is the cover pipeline: `COVER_DIR = "./cover_images/"` and `DEFAULT_IMAGE = "example-cover.png"` ([cover_image_retriever.rs:13-14](../../src/scanner/cover_image_retriever.rs#L13)). The matching endpoint `GET /api/v1/get_cover/:kind/:id` ([api_routes.rs:63](../../src/routes/api_routes.rs#L63)) is a **route skeleton wired to the placeholder handler** `ping_server` ([api_caller.rs:239-241](../../src/routes/api_caller.rs#L239)), and its `kind` discriminator is not consumed by anything.
- The only real file-serving path today is `streaming_file`, which hands the request to `tower-http`'s `ServeFile` ([stream_reader.rs:10-13](../../src/stream_reader.rs#L10)) — a usable shape if a logo/favicon ever gets served from disk.

The ⬜ markers in [ROADMAP.md](../../ROADMAP.md) §7 are accurate: all four items are genuinely not started.

## Decisions
- **Single source of truth.** All visual values — border radius, colors / color schemes (light+dark), fonts, spacing — plus app name, logo and favicon live in one place, and components read from it. Rationale: the app must stay re-skinnable from one place *without editing components*; anything hard-coded into a component breaks that.
- **Components consume tokens, never literals.** Consumption is via CSS variables or a theme object. Rationale: it is the only mechanism that makes the single source actually authoritative — a token nobody reads is decoration.
- **Scheduled in Tier 4, after the admin UI.** The backend ladder puts theming third in Tier 4 (uploads → admin UI (§5) → theming → sensitive content), and the gating rule is to finish every item in a tier before moving to the next. Rationale: the backend-served variant of this feature only pays off once there is an admin UI to edit branding from.
- **The guiding principle bears on the open decision.** "Everything must be changeable from **both** the frontend and the backend web view", implemented API-first: every mutation is an HTTP endpoint and both UIs are clients of it. The roadmap notes explicitly that backend-served branding (`GET /branding`) is the option that **fits** that principle. Rationale: if branding lived only in a frontend file, the backend web view could not change it, and the principle would be broken for this feature.

## Open questions
- **Frontend-owned file vs backend-served branding (`GET /branding`).** This is the roadmap's own open decision, tagged **[BE?][FE]** — the question mark is on the backend half, i.e. whether the backend is involved at all.
  - *Frontend-owned file:* simplest — a tokens file in the frontend repo, no endpoint, no schema, works before first paint and offline. Trade-off: only editable by editing and redeploying the frontend, so the backend web view can never change it and the guiding principle is broken for theming.
  - *Backend-served (`GET /branding`):* one catalog of values both clients fetch, editable from the admin UI, fits "changeable from both sides". Trade-off: needs an endpoint, somewhere to store the values, and a story for "what do I render before the response arrives / when offline".
  - Suggested: backend-served `GET /branding`, mirroring the §12 i18n architecture (backend-owned data, both frontends are clients) — and, as a suggestion, ship a bundled default token set in the frontend so a cold or offline start still renders, exactly the "client-side cache + bundled fallback" must-have that the served-catalog approach forced in [Internationalization](13-internationalization.md).
- **If backend-served: where do the values live — a DB table or a config file?** A DB table is editable through the existing API/trait stack and needs no new router state; a config file (next to `secrets.toml`) needs no migration but does need the router to carry more than `Arc<Database>` ([api_routes.rs:21](../../src/routes/api_routes.rs#L21)) and is awkward to edit from an admin UI. Suggested: a DB table — the admin UI edits it through the same API as everything else, and per-install branding is data, not deployment config.
- **How do the logo and favicon get delivered — bundled asset, uploaded file, or served bytes?** `GET /branding` can carry names, colors and fonts as JSON, but a logo is a binary. Options: keep logo/favicon as bundled frontend assets and only make the *name* server-configurable (loses "logo is configurable"); or store an uploaded image and serve it, reusing the `cover_images/`-style path plus the unconsumed `kind` discriminator on `GET /get_cover/:kind/:id` ([api_routes.rs:63](../../src/routes/api_routes.rs#L63)). Suggested: have `GET /branding` return a *URL* for the logo/favicon and serve the bytes from a separate asset route, so the JSON payload stays small and the image can be cached by the browser normally.

## Tasks
The roadmap lists the open decision last, but resolving it determines where the values below live, so it is worth settling first.

- ⬜ **Open decision:** frontend-owned file vs backend-served branding (`GET /branding`) so the backend web view can edit it too — the latter fits the "changeable from both sides" principle **[BE?][FE]**
- ⬜ Design-tokens file: border radius, colors / color schemes (light+dark), fonts, spacing **[FE]**
- ⬜ App name + logo + favicon as configurable values **[FE]**
- ⬜ Components consume tokens (CSS variables / theme object), never hard-coded values **[FE]**

## Done when
- [ ] Changing one value in the token source (e.g. the border radius or the primary color) visibly changes every screen, with no component file edited.
- [ ] Switching between the light and dark scheme swaps the full palette, and a search of the component code for raw color literals (hex / `rgb(`) or raw radius/spacing values returns nothing outside the token source.
- [ ] Changing the app name in one place updates it everywhere it appears, including the browser tab title.
- [ ] Replacing the logo and favicon values changes them everywhere, and a missing/unset logo falls back to a defined default instead of a broken image.
- [ ] If the backend-served option is taken: editing branding from the backend web view changes what the frontend renders after a reload, without a frontend rebuild. (And, if the bundled-fallback suggestion below is adopted, a start with the backend unreachable still renders instead of showing an unstyled page.)

## Implementation notes
- **This is a frontend-track feature and there is no frontend yet.** The first three tasks change nothing in this repository. The frontend track only opens after backend Tier 1 (auth), so theming work waits on both that and the Tier 4 ordering above.
- **Warning: don't defer the "components consume tokens" habit.** The same lesson the roadmap records twice — for the `/api/v1` prefix and for i18n's `t("key")` — applies here: retrofitting tokens into a frontend full of hard-coded colors is the rework to avoid. Wire components to tokens from the first screen even while there is only one theme.
- **If a `GET /branding` endpoint is added (Rust shape).** With a DB table, it fits the existing stack without touching router state: add a struct in [models.rs](../../src/data_models/models.rs), implement `FromRow` ([db_from_row.rs:12](../../src/database_related_scripts/db_from_row.rs#L12)) plus `Insert` ([insert.rs:10](../../src/database_related_scripts/insert.rs#L10)), `Extract` and `Update` (and `Search` — [db_search.rs:12](../../src/database_related_scripts/db_search.rs#L12) — if you need to query by anything but the primary key), add a `DatabaseTypes` variant ([models.rs:17-26](../../src/data_models/models.rs#L17)), and register a new numbered migration in the `MIGRATIONS` array ([migrations.rs:16-20](../../src/database_related_scripts/migrations.rs#L16)). The handler is then a normal `State<Arc<Database>>` + `Json<T>` handler in [api_caller.rs](../../src/routes/api_caller.rs).
- **Gotcha — give that table an `id` column.** `Extract`'s and `Update`'s default methods hard-code `WHERE id = ?1` ([extract.rs:19-29](../../src/database_related_scripts/extract.rs#L19), [db_update.rs:16-32](../../src/database_related_scripts/db_update.rs#L16)), which is already broken for the auth tables keyed on `user_id` / `session_id`. A single-row branding table keyed on anything else would hit the same wall; name the key `id` (or reach it through `Search` instead).
- **Gotcha — a config-file-backed endpoint needs router state to grow.** Router state is `Arc<Database>` alone ([api_routes.rs:21](../../src/routes/api_routes.rs#L21)); this is the same wiring gap that leaves `secrets` loaded and dropped at [main.rs:12](../../src/main.rs#L12). Reading branding from a file per request would work, but sharing it properly means introducing an `AppState` struct that carries both.
- **Gotcha — new route paths must use Axum 0.8 syntax.** The crate pins `axum = "0.8.9"` and the existing `:id` paths are 0.7 syntax that Axum 0.8 rejects with a panic at router construction. A plain `/branding` route is unaffected, but any path parameter must be written `{param}`. See [Cross-Cutting Backend Polish](07-cross-cutting-backend-polish.md).
- **Serving a logo/favicon from disk** can reuse the existing pattern: `ServeFile` via `oneshot` ([stream_reader.rs:10-13](../../src/stream_reader.rs#L10)), with a default-image fallback like the cover pipeline's `example-cover.png` ([cover_image_retriever.rs:13-14](../../src/scanner/cover_image_retriever.rs#L13)). Note that `COVER_DIR` is relative to the process working directory, so an asset directory chosen the same way inherits that constraint. Any path that comes from user input (an uploaded logo name) needs the path-containment check tracked in [Operations & Security](12-operations-and-security.md).
- *Suggestion, not a roadmap item:* if branding is served, the version/ETag pattern already decided for the i18n catalog (clients re-download only when it changed) is the obvious fit here too — branding changes far less often than anything else the API serves.

## Related
- [Backend Admin Web View](06-admin-web-view.md) — the client that would edit branding; the backend-served option exists for it.
- [Internationalization (i18n)](13-internationalization.md) — the same architecture question, already decided the backend-owned way (catalog in the backend, both frontends are clients, admin UI edits the values); its cache + bundled-fallback and version/ETag must-haves are the template for a served branding payload.
- [Uploads & File Ingestion](05-uploads-and-ingestion.md) — precedes theming in the Tier 4 order; also the place an uploaded logo image would borrow its write-to-disk path from.
- [Cross-Cutting Backend Polish](07-cross-cutting-backend-polish.md) — the router/route-syntax and status-code work any new endpoint lands on top of.
- [Operations & Security](12-operations-and-security.md) — path-traversal validation for any served branding asset, and CORS once a browser client calls the API.
- [Library & Directory Management](01-library-and-directory-management.md) — owns `GET /get_cover/:kind/:id`, the cover-serving route a branding asset endpoint would sit next to (or reuse).
