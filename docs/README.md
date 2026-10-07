# Bookserver — Feature Docs

This directory is the detail layer of the plan: one document per feature under
[features/](features/), each recording why the feature exists, what the code does today,
the decisions already settled, the questions still open, and the task checklist.

[../ROADMAP.md](../ROADMAP.md) is the other half. It carries the **build order** and the
headline items only — which tier comes next, what gates what. This index is the **map of
the feature docs**, not a second roadmap: when a task's specifics change, the feature doc
is the place to edit.

## How to use these docs

- **A new task goes into the feature doc that owns it**, in that doc's `## Tasks` list —
  not into the roadmap. The roadmap only gains a line when a whole feature moves in the
  build order.
- **The Tasks checklist in each doc is authoritative.** The roadmap's per-tier bullets are
  headline items and will always be coarser; if the two disagree about scope or progress,
  the feature doc wins.
- **`## Current state` sections carry `file:line` links, and those go stale** the moment
  the file above them is edited. Treat them as a pointer, not a fact: re-check and update
  the lines in the doc you are touching as part of touching it.
- **A settled Open question moves up into that doc's `## Decisions` section**, written as
  the decision plus the reasoning that produced it. Do not just delete the question — the
  rationale is the part that stops the same debate restarting in six months.

## Document conventions

**Fixed section order** — every feature doc has the same eight `##` sections, in this
order, with nothing extra at that level:

`Why` → `Current state` → `Decisions` → `Open questions` → `Tasks` → `Done when` →
`Implementation notes` → `Related`

**Status vocabulary** — the same three markers as the roadmap, used both for a doc's own
status and for each task line: ✅ done · 🟡 partial · ⬜ not started. `⬜ proposals only`
is a variant used by the backlog, where a marker tracks an accept/drop decision rather
than code.

**Tag vocabulary** — **[BE]** backend · **[FE]** frontend · **[DB]** schema/migration. A
trailing `?` (as in `[BE?]`) means the tag is not yet decided.

**Four-line metadata header** — directly under the H1 title and its one-sentence
blockquote, every doc carries exactly four lines:

1. `**Status:** … · **Phase:** … · **Tier:** … · **Tags:** …`
2. `**Depends on:** …` — numbered doc links with a short parenthetical reason, `·`-separated, or `Nothing`
3. `**Blocks:** …` — same shape, or `Nothing`
4. `**Roadmap:** [Build order](../../ROADMAP.md#priority-order-build-sequence)`

**File numbering** — `01`–`19` are Phase 1, `20`–`29` are Phase 2, `90`+ is the backlog.
The gaps are deliberate: a new feature slots into its range without renumbering anything
that already exists, and doc numbers stay stable so cross-links keep working.

## Phase 1

The **Tier** cell is shorthand for where the work sits in
[../ROADMAP.md](../ROADMAP.md#priority-order-build-sequence); each doc's own header spells it
out in full, including which items inside the doc are unscheduled.

| Doc | Status | Tier | Tags | What it covers |
| --- | --- | --- | --- | --- |
| [01 Library & Directory Management](features/01-library-and-directory-management.md) | 🟡 partial | Tier 2 · F2 | [BE] [DB] [FE] | Scanning, scannable directories, and CRUD on libraries, series, formats and covers — 11 tasks, 7 open questions |
| [02 Reading & Read State](features/02-reading-and-read-state.md) | 🟡 partial | Tier 3 · F1/F3 | [BE] [DB] [FE] | Range-streaming book files and remembering each user's position and finished flag — 11 tasks, 6 open questions |
| [03 Authentication, Users & Sessions](features/03-auth-users-and-sessions.md) | 🟡 partial | Tier 1 (#4) | [BE] [DB] [FE] | Password accounts with opaque DB-backed sessions, email verification and admin roles — 51 tasks, 12 open questions |
| [04 Two-Factor Authentication (TOTP)](features/04-two-factor-auth-totp.md) | 🟡 partial | Tier 1 (#5) | [BE] [DB] [FE] | Authenticator-app second factor with single-use recovery codes, layered on password login — 18 tasks, 7 open questions |
| [05 Uploads & File Ingestion](features/05-uploads-and-ingestion.md) | ⬜ not started | Tier 4 (1st) | [BE] [FE] | Upload files from the frontend, named and placed exactly as the scanner reads them back — 15 tasks, 7 open questions |
| [06 Backend Admin Web View](features/06-admin-web-view.md) | ⬜ not started | Tier 4 (2nd) | [BE] [FE] | An admin UI served by the backend, driving the same HTTP API as the main frontend — 3 tasks, 7 open questions |
| [07 Cross-Cutting Backend Polish](features/07-cross-cutting-backend-polish.md) | 🟡 partial | Tier 0 · Tier 3 (#10) | [BE] | Foundational backend chores that unblock everything else: parsing, status codes, paging, tests — 7 tasks, 7 open questions |
| [08 Theming & Branding](features/08-theming-and-branding.md) | ⬜ not started | Tier 4 (3rd) | [FE] [BE?] | Keep the app re-skinnable from one place — tokens, app name, logo, favicon — 4 tasks, 3 open questions |
| [09 Metadata Enrichment (Online Sources)](features/09-metadata-enrichment.md) | ⬜ not started | Tier 3 (#11) | [BE] [DB] [FE] | Descriptions, covers and tags from online providers, routed by library type — 10 tasks, 11 open questions |
| [10 Sensitive Content Controls](features/10-sensitive-content-controls.md) | ⬜ not started | Tier 4 (4th) | [DB] [BE] [FE] | Admin-only `is_sensitive` flag on libraries and series that skips online metadata lookups — 4 tasks, 5 open questions |
| [11 Browsing, Search & Discovery](features/11-browsing-search-and-discovery.md) | ⬜ not started | Tier 3 (#9) · F3 | [BE] [DB] [FE] | Full-text search plus the A–Z bar, filters and sorting that make the catalog navigable — 8 tasks, 8 open questions |
| [12 Operations & Security](features/12-operations-and-security.md) | 🟡 partial | Cross-cutting | [BE] [FE] | Hardening, secrets handling, and deployment sized to a LAN + Tailscale home server — 16 tasks, 5 open questions |
| [13 Internationalization (i18n)](features/13-internationalization.md) | ⬜ not started | F0 | [BE] [DB] [FE] | Backend-owned translation catalog served over the API, so adding a language is data entry — 13 tasks, 6 open questions |
| [14 Email & Notifications](features/14-email-and-notifications.md) | 🟡 partial | Cross-cutting | [BE] | One SMTP sender for account verification, password reset, and upload notifications — 18 tasks, 8 open questions |

## Phase 2

Parked and out of focus. The designs are written down so the decisions don't get
re-litigated later, not because the work is queued.

| Doc | Status | Tier | Tags | What it covers |
| --- | --- | --- | --- | --- |
| [20 Video Library](features/20-video.md) | ⬜ not started | Parked | [BE] [FE] | One server, two front doors: REST direct-play for the app, WebDAV for Infuse on Apple TV — 4 tasks, 4 open questions |
| [21 Collaborative Documents](features/21-documents.md) | ⬜ not started | Parked | [BE] [DB] [FE] | Editable text documents synced live via server-serialized append-only ops — 6 tasks, 7 open questions |
| [22 PDF Annotations](features/22-pdf-annotations.md) | ⬜ not started | Parked | [DB] [BE] [FE] | Per-user comments and highlights on PDFs, stored server-side for any client to read back — 6 tasks, 5 open questions |
| [23 Interop & Client Apps](features/23-interop-and-clients.md) | ⬜ not started | Parked | [BE] [FE] | A read-only OPDS feed so third-party reader apps can browse the library — 2 tasks, 5 open questions |
| [24 Download Permissions & Protected Delivery](features/24-download-permissions.md) | ⬜ not started | Parked | [DB] [BE] [FE] | A per-user permission deciding who may keep the original file and who may only view it through the app — 9 tasks, 8 open questions |

## Backlog

| Doc | Status | Tier | Tags | What it covers |
| --- | --- | --- | --- | --- |
| [90 Ideas Backlog](features/90-ideas-backlog.md) | ⬜ proposals only | Unscheduled | [BE] [DB] [FE] | Holding pen for unaccepted ideas — each gets promoted into a feature doc or dropped — 7 tasks, 6 open questions |

## Where the code lives

Module-level orientation only — the feature docs carry the `file:line` detail.

- **Entry & wiring** — [src/main.rs](../src/main.rs) (tracing, DB, `start_server`),
  [src/lib.rs](../src/lib.rs) (module declarations), [src/config.rs](../src/config.rs)
  (env vars and the signing key in `secrets.toml`).
- **HTTP layer** — [src/routes/](../src/routes/): `api_routes.rs` is the route table and
  server startup, `api_caller.rs` the handlers and the session extractors, `auth.rs` the
  password/session/verification logic, `totp_management.rs` the TOTP helpers,
  `email_helper.rs` the SMTP sender.
- **Database layer** — [src/database_related_scripts/](../src/database_related_scripts/):
  `db.rs` is the pooled `Database` struct, with one trait per operation in `extract.rs`
  (read), `insert.rs` (write), `db_update.rs` (modify), `db_search.rs` (lookup/filter) and
  `db_from_row.rs` (row → struct); `migrations.rs` runs the versioned SQL in
  [migrations/](../migrations/).
- **Data models & errors** — [src/data_models/](../src/data_models/): `models.rs` holds the
  metadata structs, the `*DatabaseColumns` enums and `WithId<T>`, with
  `authentication_model.rs`, `totp_model.rs` and `information_retrieval_models.rs`
  alongside it; the typed error enums and their Axum `IntoResponse` impls are in
  [src/error_types.rs](../src/error_types.rs).
- **Scanner** — [src/scanner/](../src/scanner/): `folder_scanner.rs` walks the configured
  directories and parses filenames, `cover_image_retriever.rs` extracts a cover per format,
  `metadata_fetcher.rs` is an empty placeholder for online enrichment.
- **Reading** — [src/stream_reader.rs](../src/stream_reader.rs) serves a file with HTTP
  range support; [src/utils.rs](../src/utils.rs) holds small helpers.

[../CLAUDE.md](../CLAUDE.md) at the repo root holds the fuller project map — module
responsibilities, the DB trait conventions, the SQL-safety rule. Keep it in sync when a
module's job changes; it is what a fresh session reads first.

## Known blockers

As of this writing, nothing in these docs can be exercised, because two defects sit in
front of all of it:

1. **The crate does not compile** — five type errors in
   [src/routes/api_caller.rs](../src/routes/api_caller.rs), where the TOTP handler bodies
   are empty and so return `()` where `Response<Body>` is declared.
2. **The router would panic at startup even if it did** — Axum 0.8 rejects the older
   colon-prefixed path parameter syntax, and eight route registrations still use it. The
   panic happens before the port opens.

Both are owned by
[07 Cross-Cutting Backend Polish](features/07-cross-cutting-backend-polish.md). Everything
else in these docs — every task, every "Done when" line — is blocked behind them.
