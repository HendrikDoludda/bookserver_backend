# Bookserver — Roadmap & Feature Checklist

Tracks everything needed before the **backend + frontend** are considered complete.

**Status:** ✅ done · 🟡 partial · ⬜ not started
**Tags:** **[BE]** backend · **[FE]** frontend · **[DB]** schema/migration

---

## Guiding principle

> Everything must be changeable from **both** the frontend and the backend web view.

The way to actually guarantee this is **API-first**: every mutation is an HTTP
endpoint, and both the frontend and the backend admin UI are just *clients* of
that same API. If any behaviour lives only inside one UI, the principle is
broken. Keep business logic in the backend, not in either UI.

---

## Priority order (build sequence)

Ordered by **dependency**, not feature count — most of the list is blocked behind a
few foundational pieces.

**Gating rule:** finish *every* item in a tier before moving to the next. One thing in
front of you at a time; no skipping ahead.

Two parallel tracks, each gated internally:
- **Backend ladder** — Tiers 0 → 4, in order.
- **Frontend track** — *opens* once backend Tier 1 is done, then grows as each later
  backend tier lands (a frontend screen can't precede the API it calls).

### Backend ladder

**Tier 0 — tiny unblockers (each small, high leverage):** ✅ complete
1. Add `Deserialize` to models (§6) — one derive per struct; unblocks the stubbed
   `insert_entry` / `update_entry` handlers → unblocks create/edit (§1).
2. API prefix `/api/v1/...` (§6) — trivial now, painful after the frontend couples to it.
3. Correct HTTP status codes — `EntryNotFound` → 404 (§6).

**Tier 1 — foundation that gates everything per-user:**
4. Auth & users (§3) — the keystone. Biggest single piece; unlocks the most.
   *Dependency note (not tasks here):* read state (§2), uploads (§4), sensitive
   content (§9) and the admin UI (§5, built in Tier 4) all wait on this.
5. 2FA (§3a) — *only after #4 works*; it extends the login flow that #4 builds.
   Learning project; **TOTP** (authenticator app) + recovery codes.

**Tier 2 — complete core CRUD (fulfils the guiding principle):**
6. Create/update libraries & series (§1) — unblocked by Tier 0 #1.
7. Scannable-directories table replacing `BOOK_DIRS` (§1).

**Tier 3 — makes it feel like a product:**
8. Per-user read state + "continue reading" (§2) — needs auth.
9. Search / FTS5 (§10) — route stub exists; mostly self-contained.
10. Sorting + pagination (§6) — backend support §10 filters depend on.
11. Metadata enrichment for books / manga / comics (§8) — online providers routed by
    library type; scaffolding (`scan_metadata` stub + `metadata_fetcher.rs`) already exists.

**Tier 4 — later:** uploads → admin UI (§5) → theming → sensitive content.

**Phase 2 (parked — out of focus):** video, documents, PDF tooling — and, as a *separate*
item, **video metadata enhancement** (TMDB / TVDB-style enrichment for the video library).
Kept distinct from #11 because video needs different providers *and* depends on the
video feature (Phase 2 → Video) existing first.

### Frontend track (opens after backend Tier 1)

- **F0 — i18n backend (do right after Tier 1, before F1):** build the §12 catalog +
  `GET /i18n/:locale` (+ section scoping + version/ETag) so the frontend can wire
  `t("key")` against a *real* endpoint from its first screen. Backend-only; small. **[BE]**
- **F1 — Browse & read** (start here): library/series/book browsing + the reader,
  built against the already-live `get_libraries` / `get_series_*` / `request_file`
  endpoints, plus the login flow from Tier 1. Read-only — no management screens yet.
  *Wire i18n from the first screen (§12) — `t("key")` everywhere, even English-only;
  adding languages later is then just catalog edits, not a rework.*
- **F2 — Management** (after backend Tier 2): create/edit libraries & series, manage
  scannable directories.
- **F3 — Read-state & discovery** (after backend Tier 3): "continue reading",
  read/unread indicators, search, sort/filter.

---

## Phase 1 — Core (v1 "complete")

### 1. Library & directory management
- 🟡 Scan a directory for supported files **[BE]** — `scan_all_folders` works; `POST /scan_all_directories` live
- ⬜ Incremental scanning — skip unchanged files using stored `file_hash` + `last_modified` **[BE]**
- ⬜ Scannable-directories table to replace the `BOOK_DIRS` env var **[DB][BE]** (code already has a TODO for this)
- ⬜ Add / remove scannable directories via API **[BE]** — `POST /scan_directory/:id` + `GET /scan/status` route stubs exist (placeholder handlers)
- ⬜ Add / remove directories from the frontend **[FE]**
- ⬜ Create a library and assign specific series to it **[BE][FE]** (`insert_entry`/`update_entry` currently stubbed)
- 🟡 Edit / delete libraries & series from both UIs **[BE][FE]** — `DELETE /delete_library/:id` done; create/update pending
- 🟡 Per-format support (PDF / EPUB / CBZ / images) **[BE]** — scan + cover extraction implemented; **EPUB covers untested**
- ⬜ Series & library covers user-definable; default to the first entry's image **[BE][FE]**
- ⬜ Serve cover images via API **[BE]** — `GET /get_cover/:kind/:id` route stub exists (placeholder handler)

### 2. Reading
- 🟡 Stream / serve files with HTTP range requests **[BE]** — `stream_reader` + `GET /book/:id` live
- ✅ Single-book metadata endpoint — `GET /book_metadata/:id` live **[BE]**
- ⬜ Per-user read state — current page/locator + finished flag **[DB][BE][FE]**
- ⬜ "Continue reading" surface **[BE][FE]**
- ⬜ Per-user "plan to read" list **[DB][BE][FE]**
- ⬜ Read/unread indicators — checkmark (top-right corner) on finished items; unread-count badge on series/libraries for what's not yet viewed **[BE][FE]**
- ⬜ Offline download — let the app download a file for offline reading (distinct from streaming) **[BE][FE]**
- ⬜ Reader UI per format **[FE]** — PDF, EPUB, and CBZ/image readers differ

### 3. Users, auth & settings  *(gates everything per-user below — likely the next thing to build)*

**Decided approach** (agreed, not yet implemented):
- **Sessions:** opaque random tokens stored in a `sessions` table; client sends the token in the `Authorization` header. Logout = delete the row.
- **Registration:** admin-only. First admin bootstrapped (e.g. from env on first run); only admins create further users → needs an admin/member role.
- **Hashing:** argon2 (no hand-rolled crypto).

Tasks:
- ⬜ Migration: add `role` to `users`; add `sessions` table **[DB]**
- ⬜ Password hashing helpers (argon2) **[BE]**
- ⬜ Session create / lookup / delete + token generation **[BE]**
- ⬜ `AuthUser` request extractor (reads token → loads user, or 401) **[BE]**
- ⬜ Endpoints: login, logout, admin-only create-user **[BE][FE]** — route skeletons (`POST /login`, `/log_out`, `/register_user`) already wired to a placeholder handler
- ⬜ Bootstrap the first admin on startup **[BE]**
- ⬜ Roles enforced (admin vs member) on protected routes **[BE]**
- ⬜ Per-user settings **[DB][BE][FE]**
- ⬜ **HTTPS / TLS in-app (DECIDED — part of the auth step)** — serve over TLS with
  `axum-server` (`tls-rustls`): `RustlsConfig::from_pem_file(cert, key)` + `bind_rustls`.
  Rationale: login sends password **and** TOTP code, so encrypt the transport even on LAN
  (defense-in-depth) and unlock browser secure-context features. **[BE]**
  - *Cert source:* `tailscale cert` (real, trusted for the tailnet, fits the network model)
    or long-lived self-signed (simplest; trust warnings). **Not** Let's Encrypt unless public.
  - *Only ongoing piece is renewal* — `axum-server` supports hot cert reload
    (`RustlsConfig::reload_from_pem_file`), so no restart needed. Serving itself is trivial.

#### 3a. Two-factor auth (2FA) — *learning project; build AFTER core auth above works*

**What kind (DECIDED):** **TOTP** (authenticator-app) on top of normal password auth —
*not* email/SMS delivery. Chosen because there's no delivery service to run (no SMTP, no
Twilio, no cost), it works offline, and it's more secure than SMS. The shared secret +
the current time produce the same 6-digit code on both phone and server independently.
Rust crate: `totp-rs` (secret generation, `otpauth://` URI + QR, skew-tolerant verify).

**Enrollment flow:**
1. In settings, user starts enrollment. Backend generates a random secret and returns it
   as a QR code (`otpauth://` URI); 2FA stays **disabled / pending**.
2. User scans it into their authenticator app and types back the code it currently shows.
3. Backend verifies that live code against the secret → on success, store the secret
   (`authentication` table), set `users.totp_enabled = true`, and issue **recovery codes**.
4. If they never confirm → the pending secret is discarded; 2FA stays disabled.

**Login flow:** password verified **and** current TOTP code verified in the *same* step
(no second round-trip — nothing to send). On success, issue the session + refresh tokens.
A valid **recovery code** can stand in for the TOTP code if the device is lost.

**Data model (schema already drafted in `0001_initial_schema.sql`):**
- 🟡 `users.totp_enabled` flag **[DB]**
- 🟡 `authentication` table — holds the per-user TOTP `authentication_secret` **[DB]**
  *(consider `UNIQUE(user_id)` — one secret per user)*
- 🟡 `recovery_codes` table — `code_hash`, `used` **[DB]**
- 🟡 `sessions` table — **access + refresh token split**: `session_token_hashed` (short-lived)
  + `refresh_token_hashed` (longer-lived, sliding expiry, refreshed on use) **[DB]**

**Tasks:**
- ⬜ Secret generation + QR/`otpauth` URI (via `totp-rs`) **[BE]**
- ⬜ Skew-tolerant code verification (±1 time step) **[BE]**
- ⬜ Recovery-code generation + hashed storage + single-use redemption **[BE]**
- ⬜ Enrollment endpoints: start (returns QR) + confirm (verify live code) **[BE][FE]**
- ⬜ Login: verify password + TOTP (or recovery code) together → issue tokens **[BE][FE]**
- ⬜ Refresh-token endpoint: exchange a valid refresh token for a new session token **[BE]**
- ⬜ Disable-2FA endpoint (re-auth required) **[BE][FE]**
- ⬜ Settings UI: enable (QR + confirm) / disable; show recovery codes once **[FE]**

**Security must-haves (the real learning content):**
- ⬜ **Protect the secret at rest** — a leaked `authentication_secret` lets an attacker
  generate valid codes forever; it's as sensitive as a password hash. Decide plaintext
  vs encrypted-at-rest **[BE]**
- ⬜ **Verify attempt rate-limit** — a 6-digit code is only 1,000,000 combos; throttle/lock
  after N wrong tries or it's brute-forceable **[BE]**
- ⬜ **Replay protection (optional)** — track the last-used time step so the same code
  can't be reused within its 30s window **[BE]**
- ⬜ **Recovery path** — lose your phone → recovery codes (or admin reset) or you *will*
  lock yourself out **[BE][FE]**

**Open decisions:**
- TOTP secret stored plaintext vs encrypted at rest (see above).
- `authentication` table: add `UNIQUE(user_id)` if it's strictly one secret per user.

### 4. Uploads / file ingestion
- ⬜ Upload endpoint that writes the file to the correct series folder on disk **[BE]**
- ⬜ Drag a file into a series view → triggers upload **[FE]**
- ⬜ "Where to store this + metadata" dialog on upload **[FE]**
- ⬜ Post-upload pipeline: create DB entry + extract cover + hash **[BE]** (reuse the scanner pipeline)

**Upload dialog — fields by content type:**
- **Video** — a *kind* dropdown: `movie` / `series` / `original`.
  - `movie` and `original`: no extra fields.
  - `series`: **series**, **season**, **episode** fields.
- **Books / comics** — **series** field, plus **volume** and **chapter** (chapter only
  matters when the thing being uploaded is a chapter, not a whole volume).

**Bulk upload (DECIDED — global defaults + per-file override):**
- ⬜ Accept multiple files and ingest them **sequentially** (one after the other) **[BE][FE]**
- ⬜ **Global fields at the top** of the batch — series/title, season, starting-episode
  (books: series, volume, starting-chapter). Each is **optional**. **[FE]**
- ⬜ **Per-file rows**, each individually editable to override the globals **[FE]**
- ⬜ **Per-field resolution cascade** (highest wins): **per-file manual edit → global field
  (only if filled) → value parsed from the filename → empty** **[BE][FE]**
  - Leaving a global field **empty** = fall through to the filename for that field. So a
    well-named season folder just needs the series title set (or nothing); its season/episode
    come straight from the filenames.
  - Setting a global season + starting-episode **overrides** and **auto-increments** across
    the batch — files ordered by **natural sort** (`ep2` before `ep10`).
- ⬜ **Mandatory preview/confirm step** before anything touches disk — show each file's
  *resolved* season/episode/title **and target path/name** so a wrong global increment or a
  naming mismatch is caught and fixed inline **[FE]**
- *"title" by context:* series batch → global title = **series name**, per-file title =
  **episode name**; movies batch → each file's title = the **movie name**.
- ⬜ **Collision policy (DECIDED): auto-rename** when a resolved name already exists on
  disk, and **notify the uploader by email** that the file was renamed **[BE]**
  - *Reintroduces an email dependency* (SMTP, e.g. Rust `lettre`) — not needed for TOTP
    auth, but needed here for the rename notification. Sends to the user's `users.email`.
  - **Rename scheme (DECIDED): timestamp** suffix. Must still parse cleanly back through
    the scanner's filename regex (see round-trip constraint below).
- ⬜ **Insufficient-storage policy (DECIDED): pre-flight free-space check** — if the drive
  can't hold the file, **drop the upload** (don't write a partial file) and **notify the
  user by email** **[BE]**
  - Check *before* writing so nothing half-lands. In a bulk batch, drop the offending file
    and report it; the rest of the batch is unaffected (sequential ingest).

**Library-type drives the default mode:** libraries are typed (series vs movies, books vs
comics), so the dialog defaults to the right field set. A *movies* library doesn't auto-fill
series/episode at all — "clear series info" is just an escape hatch, not the common path.

**Series field behaviour (applies to both the video-series and book/comic-series fields):**
- ⬜ **Auto-complete** against existing series **[BE][FE]** (needs a series-lookup/search endpoint)
- ⬜ **Create a new series inline** — if the typed name isn't found, offer to create it **[BE][FE]**
- ⬜ On new-series creation, **ask where it should live** (which scannable root) **[FE]** —
  depends on the scannable-directories table (§1, Tier 2)
- ⬜ Backend then **creates the folder in the correct structure + location and names
  everything per the convention** **[BE]**

> ⚠️ **Round-trip constraint:** the names/paths the uploader generates MUST match what the
> scanner's filename parser (`folder_scanner.rs` volume/chapter/page regex) expects to read
> back — otherwise a re-scan won't recognise uploaded files. Treat the naming convention as a
> single shared spec between upload (write) and scan (read).

### 5. Backend web view (admin UI)
- ⬜ Serve an admin web UI from the backend **[BE][FE]**
- ⬜ Manage libraries / series / directories / users / scans from it **[FE]**
- ⬜ **Language select in the admin UI too** (§12) — same i18n discipline from the start:
  wrap text in `t("key")` when this UI is first built, so it's localizable like the main
  app **[FE]**
  - *Resolved:* the admin UI fetches from the **same backend-owned catalog** as the main app
    (§12) — one source of truth, no separate locale files. Both clients just call the i18n API.

### 6. Cross-cutting backend polish
- ✅ Add `Deserialize` to models so write endpoints can parse request bodies **[BE]**
- ✅ Correct HTTP status codes — `EntryNotFound` → 404, not 500 **[BE]** (5xx details logged, generic body to client)
- ⬜ Sorting + pagination on list endpoints **[BE]**
- ✅ API prefix / versioning (`/api/v1/...`) before the frontend couples to it **[BE]** — all routes nested under `/api/v1`; `/health` left at root
- ⬜ Testing strategy — unit-test the DB layer (extract / insert / batch), validate EPUB covers **[BE]** *(new territory — plan to learn Rust testing together, walking through the first few)*
- ✅ Batched list queries (no N+1) **[BE]**
- ✅ `main` wired to router with clean error handling **[BE]**

### 7. Theming & branding (single source of truth)
Goal: keep the app visually re-skinnable from one place — corner rounding, color
scheme(s), fonts, app name, logo, etc. — without editing components.

- ⬜ Design-tokens file: border radius, colors / color schemes (light+dark), fonts, spacing **[FE]**
- ⬜ App name + logo + favicon as configurable values **[FE]**
- ⬜ Components consume tokens (CSS variables / theme object), never hard-coded values **[FE]**
- ⬜ **Open decision:** frontend-owned file vs backend-served branding (`GET /branding`) so the backend web view can edit it too — the latter fits the "changeable from both sides" principle **[BE?][FE]**

### 8. Metadata enrichment (online sources)
Pull extra info (descriptions, covers, tags, release years, authors) from online
databases. Different content types need different providers:
- Books → Google Books / Open Library
- Manga → AniList / MangaDex / MangaUpdates
- Comics → Comic Vine / Metron

- ⬜ `MetadataProvider` abstraction — one adapter per source, routed by library type **[BE]**
- ⬜ Search-by-title → candidate matches → confirm / auto-match → merge into the entry **[BE][FE]**
- ⬜ Store the external source id on the entry so it can be re-fetched/updated later **[DB]** (code already TODOs "a link for the metadata agent result")
- ⬜ API keys + rate limiting + result caching for external services **[BE]**
- ⬜ Manual edit/override of any fetched field **[FE]** (auto-matching is fuzzy — let the admin fix it)
- 🟡 Scaffolding exists: `scan_for_metadata` endpoint stub + `scanner/metadata_fetcher.rs`

### 9. Sensitive content controls  *(depends on roles from §3, couples with §8)*
Admins can mark a library or series as sensitive, which changes its behaviour.

- ⬜ `is_sensitive` flag on library and series **[DB]**
- ⬜ Admin-only toggle on the library & series screens **[BE][FE]**
- ⬜ Sensitive items skip all online metadata lookups — don't leak titles to third-party APIs (privacy) **[BE]** (ties into §8)
- ⬜ Extra protections (pick later): hidden from default views / explicit opt-in, optional per-user visibility, blurred covers, optional PIN or re-auth to open **[BE][FE]**

### 10. Browsing, search & discovery
- ⬜ Search across titles / authors / tags (SQLite FTS5) **[BE][FE]** — `GET /search` route stub already exists (placeholder handler)
- ⬜ A–Z fast-scroll bar — clickable letters/numbers down the side jump to series starting with that character; letters with no matches are greyed out and non-interactable **[FE]** (greying is computable client-side from the series list)
- ⬜ Filters: completed / not-completed (uses read state §2), by tag, by author **[BE][FE]**
- ⬜ Sorting controls — title, recently added, volume number, etc. **[BE][FE]** (backend support tracked in §6)

### 11. Operations & security

**Threat model & approach:** LAN-only for now; remote access via **Tailscale**
(WireGuard mesh VPN), *not* public port-forwarding. Tailscale keeps the server off
the public internet and already encrypts traffic in transit, so for the LAN +
Tailscale case the network-facing hardening below is low-urgency. App-level auth
(§3) is still required regardless — Tailscale controls *which devices* can reach
the box; auth controls *who you are* and what you can do once in (needed for
multi-user separation). Items marked *(public only)* become urgent only if the
server is ever exposed directly to the open internet.

- ⬜ Path-traversal protection — validate served file paths stay inside known library/cover dirs **[BE]** (safe today since serving is by id; becomes real with uploads §4)
- 🟡 TLS / HTTPS **[BE]** — **moved into the auth step (§3)** and done **in-app** via
  `axum-server` (`tls-rustls`), *not* deferred to "public only" anymore: it protects the
  login credential + TOTP handshake even on the LAN and enables browser secure-context
  features. Reverse proxy (Caddy) remains a valid alternative if in-app renewal ever annoys.
- ⬜ Login rate-limiting (brute-force protection) **[BE]** *(public only)*
- ⬜ CORS configuration **[BE]** — only once a browser client (web frontend / backend web view) calls the API
- ⬜ File reconciliation — a scan prunes/flags DB rows whose files were moved or deleted on disk **[BE]** (data integrity, not security)
- ⬜ Deployment / packaging: Windows `.exe`, macOS `.dmg`, Docker image; persistent volume for the SQLite DB + covers **[BE]**


### 12. Language options (i18n / localization)
Goal: users pick a display language; adding a new language never forces a rework.

**Architecture (DECIDED): backend-owned catalog, served over the API.** One catalog lives in
the backend (single source of truth); both frontends (main app + admin UI §5) fetch strings
via an API call rather than bundling their own locale files. Fits "changeable from both
sides" — and since it's *data in the backend*, translations can be edited from the admin UI,
and the §4 notification **emails localize for free** (backend already has the catalog + the
user's locale; no separate email catalog needed).

- ⬜ Catalog stored backend-side, keyed by **key** (never by English string), per locale,
  and grouped into **namespaces/sections** (e.g. `settings`, `library`, `options`, `reader`) **[BE]**
- ⬜ `GET /i18n/:locale` endpoint returning that locale's strings **[BE]**
- ⬜ **Section-scoped retrieval** — a screen can request just the namespaces it needs in the
  chosen language in one call (e.g. `GET /i18n/:locale?sections=settings,library,options`),
  instead of pulling unrelated strings **[BE]**

**Retrieval strategy (DECIDED):** the **default is to fetch the whole catalog for the chosen
locale once, cache it locally, and re-check periodically** (e.g. daily, or on app start) via
the version/ETag below — so normal use is a local lookup with no per-screen round-trips. The
section-scoped call above is the fallback for the rare case a screen needs strings not yet
cached.
- ⬜ A `t("some.key")` lookup used everywhere instead of literal UI text **[FE]**
- ⬜ **Fallback locale** — a missing key falls back to the default (English), so a
  half-translated language still works **[BE][FE]**
- ⬜ **Pluralization + interpolation** handled (`"{count} unread"`; plural rules differ per
  language) — the two classic i18n gotchas **[FE]**
- ⬜ **Per-user language preference** stored on the user (ties into §3 "Per-user settings");
  fall back to browser/OS locale, then default, when not logged in **[DB][BE][FE]**

**Must-haves for the served-catalog approach (the cost of not bundling files):**
- ⬜ **Client-side cache + bundled fallback** — the app needs strings before first paint and
  must work offline (§2 offline reading). Cache the fetched catalog locally and ship at least
  the default language baked in, so a cold/offline start still renders **[FE]**
- ⬜ **Versioned fetch (ETag / version number)** so clients only re-download when the catalog
  changed, not on every load **[BE][FE]**
- ⬜ **Runtime-loadable lookup** — most i18n libs expect files at *build* time; serving at
  runtime means loading JSON into a map + lookup (or a package that supports remote loading,
  esp. on Flutter) **[FE]**

> **Timing — start the *structure* at F1, not the translations.** Retrofitting i18n into a
> frontend full of hardcoded strings is exactly the "complete rework" to avoid. So wrap UI
> text in `t("key")` from the first screen even while only English exists; adding languages
> then becomes catalog edits. The *architecture* is a day-one decision; the *translations*
> are low-priority polish for whenever. (Same lesson as the `/api/v1` prefix.)

> *Scope:* this system translates **UI chrome** ("Continue reading"). Library/series names and
> descriptions are **user data**, not UI text, so they aren't translated by it.

---

## Phase 2 — Future ("complete home-server" ambitions)

### Video — this backend is the single media source, reached by multiple clients
**Decided:** the custom backend serves video itself (one machine, one copy of the
files). Two front doors over HTTP:
1. **Custom Flutter app** (phone/computer) → the normal REST API, **direct-play**
   file streaming with range support (like `stream_reader` does for books). The app
   plays it with a client that bundles ffmpeg (e.g. `media_kit`) → client-side
   decode, **no server transcoding**.
2. **Infuse** (Apple TV + Apple devices, video-only) → Infuse can't talk to a custom
   REST API, so the backend also exposes the video library over a protocol Infuse
   understands. Practical choice: **WebDAV** (HTTP-based; Rust `dav-server` crate can
   mount alongside Axum). Infuse also supports SMB/NFS/DLNA if a plain share is easier.

- ⬜ Direct-play video streaming over the REST API (range support) **[BE]**
- ⬜ Capable in-app video player (e.g. Flutter `media_kit`, bundles ffmpeg) **[FE]**
- ⬜ WebDAV endpoint exposing the video library for Infuse **[BE]** (basic-auth; protected by Tailscale when remote)
- ⬜ Video metadata enhancement — TMDB / TVDB-style enrichment (posters, synopsis, cast,
  episode/season data) **[BE][FE]** — separate from §8 (different providers, movie/show data
  model); reuse §8's `MetadataProvider` abstraction routed by content type
- Per-device scope: Apple TV (Infuse) needs **video only** — no document/book reader there.
- **Out of scope:** server-side transcoding (clients decode on-device); a custom tvOS app (use Infuse instead — Flutter doesn't target tvOS anyway).

### Documents
**Data model (decided):** a document is an **ordered list of nodes** (sentences/blocks),
each with a **unique id** and references to its neighbour ids — i.e. a linked list.
Edits reference **neighbour ids, not character positions**, so the doc can shift without
breaking an edit's anchor. Edits are small append-only **ops** (deltas); the same op is
the unit of *local storage*, *wire payload*, and *sync* (one representation, no
translation) — which also gives history/undo for free. Unconfirmed ops **queue locally
and flush on reconnect** (offline-first; ties into offline reading).

**Concurrency (decided): server-serialized, NOT a CRDT.** All edits funnel through the
one backend, processed one at a time → the server *is* the total order (by arrival), so
there's nothing to reconcile. This is the simple, correct choice *because* there's a
central backend; CRDTs exist for the *decentralised / no-arbiter* case, which we don't
have. Two surviving edges: (1) **tombstones** — mark a deleted node dead but keep its id
so edits anchored to it still resolve; (2) editing the **same node** at once is
**last-writer-wins** (the server orders but doesn't merge). Auto-merging the same node is
the *only* thing that would need a CRDT.

- ⬜ v1: serialized sync on **markdown/plaintext** — node list + unique ids + neighbour refs, small ops, autosave, history, offline queue **[BE][FE]**
- ⬜ Tombstones for deleted nodes so anchored edits still resolve **[BE]**
- ⬜ Real-time delivery — push updates to clients with the doc open (WebSocket / SSE) **[BE][FE]**
- ⬜ Create new documents in-app **[FE][BE]**
- ⬜ Escape hatch (only if ever needed): true offline-concurrent / same-node auto-merge → a CRDT lib (`yrs` / `automerge`), or hand-rolled WOOT/RGA-style as a learning exercise **[BE][FE]**
- ⬜ Later: **Office formats** (docx/xlsx/pptx) — zipped XML trees, so deltas are XML-tree mutations, not text → meaningfully more work **[BE][FE]**
  - *Idea (not committed):* reuse the v1 op-store as a neutral intermediate model instead of editing the zip in place. Confine XML-tree surgery to two boundaries — **import** (docx → node list, assign ids to paragraphs/runs/cells) and **materialize/export** (replay ops → XML → rezip, only on explicit save). Inserted assets (images) live in the op as a blob and only get wired into `word/media/` + rels + `<w:drawing>` at export. Tradeoff: import/export is **lossy** for formatting the importer doesn't model — "edit a docx" ≠ "round-trip a docx perfectly". Fidelity-preserving overlay (anchor ids onto untouched original XML) is the later escape hatch.

### PDF tooling
- ⬜ Comments + highlighting, per user **[DB][BE][FE]**

### Interop
- ⬜ OPDS catalog feed **[BE]** — lets standard reader apps browse the server (secondary to the custom frontend app, but cheap interop)

### Mobile apps
- ⬜ iOS / Android apps — deferred until there's revenue to justify the effort; manual install on devices until then **[FE]**

---

## Ideas to run by you (proposals, not commitments)

These came up while writing the list — pick what's interesting, ignore the rest.
*(Promoted: incremental scanning → Phase 1; OPDS → Phase 2.)*

1. **File watching for auto-rescan** — use the `notify` crate to detect
   filesystem changes and rescan automatically, instead of a manual scan button.
2. **Thumbnail generation** — serve small resized covers for grid views so the
   frontend isn't downloading full-size images for every card.
3. **Per-library permissions** — once roles exist, control which users can see
   which libraries (e.g. a kids' library).
4. **Format-aware reading position** — "page number" works for PDF/CBZ, but EPUB
   needs a locator/CFI (reflowable text has no fixed pages). Worth designing the
   read-state schema with this difference in mind from the start.
5. **DB backup/export endpoint** — it's a home server holding your whole library
   catalog; a one-click SQLite backup is cheap insurance.

### Reality check on document collaboration
Full design is in **Phase 2 → Documents** (server-serialized, id-anchored ops +
tombstones; CRDT only as an escape hatch — see there for the reasoning). The two
genuinely hard parts that remain after that approach:

- **Office-format fidelity** — docx/xlsx/pptx are zipped XML trees, not text, so edits
  are XML-tree mutations. Start collaboration on markdown/plaintext; treat Office as a
  separate, later effort.
- **Auto-merging simultaneous edits to the *same* node** — the one case the central-server
  approach can't handle (it's last-writer-wins). Only this needs a CRDT; everything else
  is achievable with serialized writes.
