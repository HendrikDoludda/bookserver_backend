# PDF Annotations

> Per-user comments and highlighting on PDFs, stored in the backend so any client can read them back.

**Status:** ⬜ not started · **Phase:** 2 · **Tier:** Parked — out of focus · **Tags:** [DB] [BE] [FE]
**Depends on:** [03 Authentication, Users & Sessions](03-auth-users-and-sessions.md) (annotations are per-user) · [02 Reading & Read State](02-reading-and-read-state.md) (shares the anchor/locator problem with read state)
**Blocks:** Nothing
**Roadmap:** [Build order](../../ROADMAP.md#priority-order-build-sequence)

## Why
The PDF reading path is already written — the file streams with range support and a cover is rendered — but nothing a reader *does* with a book is remembered. Comments and highlights turn the server from a delivery pipe into something usable for study and reference. It has to be per-user, because this is a multi-user household server and an annotation is personal, not library metadata. The roadmap deliberately parks this behind all of Phase 1: it is one line in the plan, and it should stay one line until the auth and read-state foundations it leans on exist.

## Current state
Nothing exists for this feature — no table, no struct, no route, not even a stub.

- No annotation / highlight / comment table anywhere in the schema. [0001_initial_schema.sql](../../migrations/0001_initial_schema.sql) creates exactly ten tables (`books`, `library`, `library_elements`, `series`, `users`, `email_verification`, `reset_password_requests`, `sessions`, `recovery_codes`, `totp`) and none of them is annotation-shaped.
- No `DatabaseTypes` variant to address such a table — the enum has seven variants and stops at `Session` ([models.rs:18](../../src/data_models/models.rs#L18)).
- The route table ([api_routes.rs:45](../../src/routes/api_routes.rs#L45)) has nothing PDF-specific — no annotation route, real handler or placeholder. The per-book endpoints that exist are `GET /api/v1/book/:id` ([api_routes.rs:55](../../src/routes/api_routes.rs#L55)) and `GET /api/v1/book_metadata/:id` ([api_routes.rs:58](../../src/routes/api_routes.rs#L58)), both live, plus `PATCH|GET /api/v1/reading_progress/:id` ([api_routes.rs:57](../../src/routes/api_routes.rs#L57)), a route skeleton wired to the `ping_server` placeholder.
- The backend opens a PDF in exactly two places today, and neither understands the document's contents in a way an anchor could use: cover rendering of page 0 through pdfium ([cover_image_retriever.rs:77](../../src/scanner/cover_image_retriever.rs#L77)), and opaque byte-range file serving via `ServeFile` ([stream_reader.rs:10](../../src/stream_reader.rs#L10)). No PDF text extraction, no page geometry, no page count.
- `books.page_count` exists as a column ([0001_initial_schema.sql:12](../../migrations/0001_initial_schema.sql#L12)) but is always NULL in practice: the scanner fills it from `FileExtractedMetadata.page_number` ([folder_scanner.rs:146](../../src/scanner/folder_scanner.rs#L146)), which `get_file_metadata` hard-codes to `None` on both branches ([folder_scanner.rs:191](../../src/scanner/folder_scanner.rs#L191), [folder_scanner.rs:199](../../src/scanner/folder_scanner.rs#L199)).
- The neighbouring per-user locator problem — read state — is also entirely unbuilt (see [Reading & Read State](02-reading-and-read-state.md)), so there is no existing anchor shape to copy yet.

Roadmap drift: none. ⬜ not started matches the code exactly.

## Decisions
- **Parked in Phase 2, out of focus.** The roadmap groups "PDF tooling" with video and documents as parked Phase 2 work, and the backend ladder has a hard gating rule — finish every item in a tier before moving on, one thing in front of you at a time. So this does not start until Phase 1 is done. Rationale: it is a nice-to-have on top of features (auth, read state) that are themselves not built.
- **Per user, not per book.** The roadmap line is explicitly "Comments + highlighting, **per user**" and tagged **[DB][BE][FE]** — a schema change, backend endpoints, and reader UI. Rationale: annotations belong to the reader, so they are rows keyed by user, and that makes auth a hard prerequisite rather than a nicety.
- **Comments and highlighting are one item.** The roadmap carries them on a single line, and they can share one anchor and one table — only the payload differs (a highlight has no body text, a comment does). Rationale: two tables would duplicate the whole anchor design for no gain.
- **API-first, like everything else.** Per the guiding principle, every mutation is an HTTP endpoint and both the frontend and the backend admin web view are just clients of it — no annotation logic may live only inside a UI.
- **`user_id` comes from the session token, never from the request body.** Inherited from the §11 hardening rule ("the single rule that prevents acting as another user") — see [Operations & Security](12-operations-and-security.md). Rationale: an annotation endpoint that accepts a user id in the body lets anyone write into anyone else's notes.

## Open questions
- **What is the anchor — where does a highlight actually live?** Options: (a) page index + a normalized rectangle (or quad list) on the rendered page — simple, format-agnostic, needs no PDF parsing on the server; (b) PDF text offsets / selection quads produced by the client's viewer — more faithful to a text selection, but ties the anchor to whichever viewer library the client uses; (c) the PDF's own annotation objects, written into the file. Suggested: (a) page + normalized rect, treated as opaque data the backend stores but never interprets, so the server needs no PDF understanding at all.
- **Store annotations in the DB or write them into the PDF file?** In-DB rows leave the file byte-identical, which keeps `file_hash` / `last_modified` stable and means a re-scan and any future incremental scan see nothing changed. Writing them into the PDF is portable to third-party readers but mutates library files, invalidates the hash, and would need pdfium write support. Suggested: DB rows only; the file on disk stays read-only for the whole server.
- **Should the anchor share one design with read state?** The roadmap's own idea 4 (see [Ideas Backlog](90-ideas-backlog.md)) already flags that "page number" works for PDF/CBZ but EPUB needs a locator/CFI, and says the read-state schema is worth designing with that difference in mind from the start. Documents solve the same class of problem a third way — anchor by node id, never by character position, so content can shift without breaking the anchor. Suggested: settle one locator/anchor shape once, when read state is built, and have annotations reuse it rather than inventing a second one.
- **Visibility.** The roadmap says only "per user", which reads as private-to-the-owner. Shared or per-library-visible annotations are not in scope. Suggested: private only; revisit only if someone actually asks.
- **Formats other than PDF.** The roadmap scopes this to PDF tooling. A page + rect anchor would work just as well for CBZ / image comics. Suggested: keep it PDF-only as written, and note the reuse if it ever comes up — do not build it speculatively.

## Tasks
The roadmap carries this as a single line; the rest is that line split along its own **[DB][BE][FE]** tags, not added scope.

- ⬜ Comments + highlighting, per user **[DB][BE][FE]**
- ⬜ Settle the anchor model before writing any migration (see Open questions) **[DB]**
- ⬜ Migration: per-user annotation table — rows keyed by user + book + anchor, `ON DELETE CASCADE` from `users` and `books` **[DB]**
- ⬜ Model + trait impls for the new table, following the existing `FromRow` / `Insert` / `Search` / `Update` pattern **[BE]**
- ⬜ Endpoints: list / create / edit / delete an annotation, scoped to the session user **[BE]**
- ⬜ Reader UI: select text or a region to highlight, attach a comment, render existing annotations, delete one **[FE]**

## Done when
- [ ] A logged-in user can create a highlight on a PDF and get it back, at the same page and position, on a later request.
- [ ] A second user opening the same book gets `200` with an empty list — user A's annotations are never visible to user B.
- [ ] Any annotation endpoint called without a valid session token returns `401`, and the stored `user_id` always comes from the session, never from the request body.
- [ ] Creating and deleting annotations leaves the PDF on disk byte-identical — `file_hash` and `last_modified` are unchanged, so a re-scan does nothing. *(Only a criterion if the storage open question settles on DB rows, as suggested; writing annotations into the file would mutate it by design.)*
- [ ] Deleting the user (or the book) removes their annotations, leaving no orphan rows.
- [ ] A comment survives a server restart and comes back with its text and anchor intact.

## Implementation notes
- **Auth first, genuinely.** Every row needs a `user_id`, and there is no way to get a trustworthy one until the session extractor from [Auth](03-auth-users-and-sessions.md) works. Starting this before then means either faking a user id or accepting one from the body, and the second is the exact mistake §11 warns about.
- **The Rust shape is the one this repo already repeats for every table it reads:** one metadata struct in [models.rs](../../src/data_models/models.rs), one `*DatabaseColumns` enum next to it with a hand-written `impl AsRef<str>`, and small `FromRow` / `Insert` / `Search` / `Update` impls in `src/database_related_scripts/`. Nothing generic or clever is needed.
- **Name the primary key column `id`.** The `Extract` and `Update` default methods hard-code `WHERE id = ?1` / `WHERE id = ?2` ([extract.rs:21](../../src/database_related_scripts/extract.rs#L21), [db_update.rs:23](../../src/database_related_scripts/db_update.rs#L23)). Every auth table that named its key something else (`session_id`, `recovery_id`, `user_id`) inherited SQL that references a non-existent column and is a live source of runtime failures — don't repeat it here.
- **Register the migration** as a new numbered `.sql` file plus a new entry in the `MIGRATIONS` array ([migrations.rs:16](../../src/database_related_scripts/migrations.rs#L16)); each migration runs in its own transaction, so a multi-statement file is all-or-nothing. (The maintainer's early-dev habit of editing `0001` in place and wiping the DB should be long over by Phase 2 — there will be real user data by then.)
- **FKs do work at runtime** — the pool sets `PRAGMA foreign_keys = ON` per connection ([db.rs:36](../../src/database_related_scripts/db.rs#L36)) — but only where a `FOREIGN KEY` clause is actually declared. Note that `books.series` has none ([0001_initial_schema.sql:19](../../migrations/0001_initial_schema.sql#L19)), so declare the annotation table's FKs explicitly instead of assuming the pattern is uniform.
- **pdfium is a real deployment constraint.** The only PDF library in the tree is `pdfium-render = "0.9.4"` ([Cargo.toml:36](../../Cargo.toml#L36)), and it needs the dynamic pdfium library resolvable at runtime — the standing TODO says it still has to be bundled "so that on a mac you can run this without symlinking" ([cover_image_retriever.rs:75](../../src/scanner/cover_image_retriever.rs#L75)). If the backend ever has to parse page geometry or text to *validate* an anchor, that bundling problem has to be solved first. Keeping anchors opaque to the backend sidesteps the dependency entirely.
- Because `books.page_count` is always NULL today (see Current state), a page-index anchor cannot currently be range-checked server-side. Either populate `page_count` during the scan or accept the client's page number unvalidated.
- *Suggested table shape (a suggestion, not roadmap scope):* `id`, `user_id`, `book_id`, `kind` (`comment` | `highlight`), the anchor (page number + normalized rect, or a single locator string), `body TEXT` for comment text, `created_at` / `updated_at`. Storing the anchor as one opaque locator string keeps the door open to reusing whatever shape read state settles on.

## Related
- [Authentication, Users & Sessions](03-auth-users-and-sessions.md) — hard prerequisite; supplies the session-derived `user_id` every annotation row needs.
- [Reading & Read State](02-reading-and-read-state.md) — the same per-user locator problem (PDF pages vs EPUB CFI); solve the anchor shape there once and reuse it.
- [Collaborative Documents](21-documents.md) — its id-anchored ops and tombstones are the same "anchor that survives shifting content" problem in a third form.
- [Library & Directory Management](01-library-and-directory-management.md) — incremental scanning relies on `file_hash` / `last_modified`, which is the argument for annotations not mutating the PDF (the storage question is still open above).
- [Operations & Security](12-operations-and-security.md) — the "derive `user_id` from the session, never the body" rule and generic error bodies.
- [Video Library](20-video.md) — parked alongside this as Phase 2 work; both wait on the Phase 1 ladder.
- [Ideas Backlog](90-ideas-backlog.md) — holds idea 4 (format-aware reading position), the locator question this feature's anchor should inherit an answer from.
