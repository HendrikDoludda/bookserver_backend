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

## Phase 1 — Core (v1 "complete")

### 1. Library & directory management
- 🟡 Scan a directory for supported files **[BE]** — `scan_all_folders` works; `POST /scan_all_directories` live
- ⬜ Incremental scanning — skip unchanged files using stored `file_hash` + `last_modified` **[BE]**
- ⬜ Scannable-directories table to replace the `BOOK_DIRS` env var **[DB][BE]** (code already has a TODO for this)
- ⬜ Add / remove scannable directories via API **[BE]**
- ⬜ Add / remove directories from the frontend **[FE]**
- ⬜ Create a library and assign specific series to it **[BE][FE]** (`insert_entry`/`update_entry` currently stubbed)
- 🟡 Edit / delete libraries & series from both UIs **[BE][FE]** — `DELETE /delete_library/:id` done; create/update pending
- 🟡 Per-format support (PDF / EPUB / CBZ / images) **[BE]** — scan + cover extraction implemented; **EPUB covers untested**
- ⬜ Series & library covers user-definable; default to the first entry's image **[BE][FE]**

### 2. Reading
- 🟡 Stream / serve files with HTTP range requests **[BE]** — `stream_reader` + `GET /book/:id` live
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
- ⬜ Endpoints: login, logout, admin-only create-user **[BE][FE]**
- ⬜ Bootstrap the first admin on startup **[BE]**
- ⬜ Roles enforced (admin vs member) on protected routes **[BE]**
- ⬜ Per-user settings **[DB][BE][FE]**

### 4. Uploads / file ingestion
- ⬜ Upload endpoint that writes the file to the correct series folder on disk **[BE]**
- ⬜ Drag a file into a series view → triggers upload **[FE]**
- ⬜ "Where to store this + metadata" dialog on upload **[FE]**
- ⬜ Post-upload pipeline: create DB entry + extract cover + hash **[BE]** (reuse the scanner pipeline)

### 5. Backend web view (admin UI)
- ⬜ Serve an admin web UI from the backend **[BE][FE]**
- ⬜ Manage libraries / series / directories / users / scans from it **[FE]**

### 6. Cross-cutting backend polish
- ⬜ Add `Deserialize` to models so write endpoints can parse request bodies **[BE]**
- ⬜ Correct HTTP status codes — `EntryNotFound` → 404, not 500 **[BE]**
- ⬜ Sorting + pagination on list endpoints **[BE]**
- ⬜ API prefix / versioning (`/api/v1/...`) before the frontend couples to it **[BE]**
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
- ⬜ Search across titles / authors / tags (SQLite FTS5) **[BE][FE]** — `/search` route stub already exists
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
- ⬜ TLS / HTTPS **[BE]** *(public only)* — WireGuard already encrypts the Tailscale case; needed only if directly exposed (reverse proxy like Caddy is the easy route; or `tailscale cert`)
- ⬜ Login rate-limiting (brute-force protection) **[BE]** *(public only)*
- ⬜ CORS configuration **[BE]** — only once a browser client (web frontend / backend web view) calls the API
- ⬜ File reconciliation — a scan prunes/flags DB rows whose files were moved or deleted on disk **[BE]** (data integrity, not security)
- ⬜ Deployment / packaging: Windows `.exe`, macOS `.dmg`, Docker image; persistent volume for the SQLite DB + covers **[BE]**

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
