# Bookserver — Roadmap

**What to build, in what order.** This file carries the build sequence and the headline
items only.

Every feature's detail — the full task checklist, the settled decisions and their
reasoning, the open questions, and what exists in the code today — lives in its own
document under [docs/features/](docs/features/). Start at
[docs/README.md](docs/README.md) for the index and the doc conventions.

> Rule of thumb: **order** changes often, so it lives here. **Detail** accumulates, so it
> lives in the feature docs. When a task's specifics change, edit the feature doc, not this
> file.

**Status:** ✅ done · 🟡 partial · ⬜ not started
**Tags:** **[BE]** backend · **[FE]** frontend · **[DB]** schema/migration

---

## Guiding principle

> Everything must be changeable from **both** the frontend and the backend web view.

The way to actually guarantee this is **API-first**: every mutation is an HTTP endpoint, and
both the frontend and the backend admin UI are just *clients* of that same API. If any
behaviour lives only inside one UI, the principle is broken. Keep business logic in the
backend, not in either UI.

---

## Blocked right now

Two defects sit in front of the entire ladder. Neither is a feature, and nothing below can
be tested until both are cleared. Both are owned by
[07 Cross-Cutting Backend Polish](docs/features/07-cross-cutting-backend-polish.md).

1. ⬜ **The crate does not compile** — five type errors at
   [api_caller.rs:617-625](src/routes/api_caller.rs#L617): the TOTP handler bodies are
   empty, so they return `()` where `Response<Body>` is declared. Fill them or remove the
   handlers. **[BE]**
2. ⬜ **The router panics before the port opens** — `axum` is pinned to 0.8, which rejects
   the 0.7 `:param` path syntax; **eight** registrations still use it in
   [api_routes.rs](src/routes/api_routes.rs#L48) (nine params — `/get_cover/:kind/:id` has
   two). Mechanical fix: `:id` → `{id}`. **[BE]**

Two more break live endpoints once it boots:

3. ⬜ `GET /api/v1/get_libraries` can't return a row — `LibraryMetadata::from_row` reads
   column `name` while the query selects `library_name`
   ([db_from_row.rs:71](src/database_related_scripts/db_from_row.rs#L71)). Two more `FromRow`
   column typos sit beside it. → [01](docs/features/01-library-and-directory-management.md) **[BE]**
4. ⬜ No `CookieManagerLayer` is installed, so every handler behind the `AuthSession`
   extractor answers 400 before its body runs. →
   [03](docs/features/03-auth-users-and-sessions.md) **[BE]**

---

## Priority order (build sequence)

Ordered by **dependency**, not feature count — most of the list is blocked behind a few
foundational pieces.

**Gating rule:** finish *every* item in a tier before moving to the next. One thing in front
of you at a time; no skipping ahead.

Two parallel tracks, each gated internally:

- **Backend ladder** — Tiers 0 → 4, in order.
- **Frontend track** — *opens* once backend Tier 1 is done, then grows as each later backend
  tier lands (a frontend screen can't precede the API it calls).

### Backend ladder

**Tier 0 — tiny unblockers** *(each small, high leverage)*: 🟡 landed on paper
All three are owned by [07](docs/features/07-cross-cutting-backend-polish.md) and are in the
code, but see it — the derive landed while its handlers are still stubbed, the `/api/v1`
prefix landed but no request can traverse it, and the status-code decision landed for 1
error enum out of 10.

1. ✅ Add `Deserialize` to models — unblocks the stubbed `insert_entry` / `update_entry`
   handlers → unblocks create/edit (#6). **[BE]**
2. ✅ API prefix `/api/v1/...` — trivial now, painful after the frontend couples to it. **[BE]**
3. 🟡 Correct HTTP status codes — `EntryNotFound` → 404. **[BE]**

**Tier 1 — the foundation that gates everything per-user:**

4. 🟡 [Auth, users & sessions](docs/features/03-auth-users-and-sessions.md) — the keystone,
   and the biggest single piece. **Further along than it looks:** Argon2 hashing, sign-up,
   email verification and the session + refresh token pair are written; nothing is wired to
   a route yet. **[BE][DB][FE]**
   *Dependency note (not tasks here):* read state (#8), uploads (#12), sensitive content
   (#15) and the admin UI (#13, built in Tier 4) all wait on this.
5. 🟡 [Two-factor auth (TOTP)](docs/features/04-two-factor-auth-totp.md) — *only after #4
   works*; it extends the login flow #4 builds. Schema is drafted, logic is comment-only
   stubs. Learning project: TOTP + recovery codes. **[BE][DB][FE]**

**Tier 2 — complete core CRUD (fulfils the guiding principle):**

6. ⬜ Create/update libraries & series →
   [01](docs/features/01-library-and-directory-management.md) — unblocked by Tier 0 #1. **[BE][FE]**
7. ⬜ Scannable-directories table replacing the `BOOK_DIRS` env var →
   [01](docs/features/01-library-and-directory-management.md) **[DB][BE]**

**Tier 3 — makes it feel like a product:**

8. ⬜ [Per-user read state + "continue reading"](docs/features/02-reading-and-read-state.md)
   — needs auth. **[DB][BE][FE]**
9. ⬜ [Search / FTS5](docs/features/11-browsing-search-and-discovery.md) — route stub
   exists; mostly self-contained. **[BE][FE]**
10. ⬜ Sorting + pagination →
    [07](docs/features/07-cross-cutting-backend-polish.md) — the backend support the search
    and filter work depends on. **[BE]**
11. ⬜ [Metadata enrichment](docs/features/09-metadata-enrichment.md) for books / manga /
    comics — online providers routed by library type. **[BE][DB][FE]**

**Tier 4 — later, in this order:**

12. ⬜ [Uploads & file ingestion](docs/features/05-uploads-and-ingestion.md) **[BE][FE]**
13. ⬜ [Backend admin web view](docs/features/06-admin-web-view.md) **[BE][FE]**
14. ⬜ [Theming & branding](docs/features/08-theming-and-branding.md) **[FE]**
15. ⬜ [Sensitive content controls](docs/features/10-sensitive-content-controls.md) **[DB][BE][FE]**

**After Tier 4:** nothing else is scheduled — everything beyond it is *Phase 2, parked*
(see below), including video metadata enhancement as its own separate item.

### Frontend track (opens after backend Tier 1)

- **F0 — [i18n backend](docs/features/13-internationalization.md)** *(do right after Tier 1,
  before F1)*: build the catalog + `GET /i18n/:locale` (+ section scoping + version/ETag) so
  the frontend can wire `t("key")` against a *real* endpoint from its first screen.
  Backend-only; small. **[BE]**
- **F1 — Browse & read** *(start here)*: library/series/book browsing plus the reader, built
  against the already-live `get_libraries` / `get_series_*` / `request_file` endpoints and
  the login flow from Tier 1. Read-only — no management screens yet. Readers differ per
  format → [02](docs/features/02-reading-and-read-state.md).
  *Wire i18n from the first screen — `t("key")` everywhere, even English-only; adding
  languages later is then just catalog edits, not a rework.*
- **F2 — Management** *(after backend Tier 2)*: create/edit libraries & series, manage
  scannable directories → [01](docs/features/01-library-and-directory-management.md).
- **F3 — Read-state & discovery** *(after backend Tier 3)*: "continue reading", read/unread
  indicators → [02](docs/features/02-reading-and-read-state.md); search, sort/filter, the
  A–Z bar → [11](docs/features/11-browsing-search-and-discovery.md).

### Cross-cutting (no tier of their own)

These aren't sequenced — they ride along with whatever tier touches them.

- 🟡 [Operations & security](docs/features/12-operations-and-security.md) — hardening sized
  to a LAN + Tailscale home server. In-app TLS and "`user_id` always from the session token"
  ride along with Tier 1; deployment/packaging comes whenever. The doc also records what is
  deliberately **excluded** as over-engineering for this threat model. **[BE]**
- 🟡 [Email & notifications](docs/features/14-email-and-notifications.md) — already in use by
  Tier 1 (verification, password reset), extended again by Tier 4 uploads. **[BE]**
- ⬜ Testing strategy → [07](docs/features/07-cross-cutting-backend-polish.md) — unit-test
  the DB layer, validate EPUB covers. New territory; plan to learn Rust testing together,
  walking through the first few. **[BE]**

---

## Phase 2 — parked (out of focus)

Not scheduled. Designs are recorded so the decisions aren't re-litigated later.

- ⬜ [Video library](docs/features/20-video.md) — direct-play over the REST API for the
  Flutter app, WebDAV for Infuse. Includes **video metadata enhancement** (TMDB/TVDB-style),
  kept separate from Tier 3 #11 because it needs different providers *and* depends on the
  video feature existing first. **[BE][FE]**
- ⬜ [Collaborative documents](docs/features/21-documents.md) — server-serialized,
  id-anchored ops; explicitly *not* a CRDT. Markdown first; Office formats much later. **[BE][FE][DB]**
- ⬜ [PDF annotations](docs/features/22-pdf-annotations.md) — per-user comments and
  highlighting. **[DB][BE][FE]**
- ⬜ [Interop & client apps](docs/features/23-interop-and-clients.md) — OPDS catalog feed;
  native mobile apps deferred until there's revenue to justify them. **[BE][FE]**

---

## Backlog

[90 Ideas Backlog](docs/features/90-ideas-backlog.md) — five proposals awaiting an
accept-or-drop: file watching for auto-rescan, thumbnail generation, per-library
permissions, format-aware reading position, and a DB backup/export endpoint. One is
time-sensitive: **format-aware reading position** has to be answered before the read-state
schema (Tier 3 #8) is written.
