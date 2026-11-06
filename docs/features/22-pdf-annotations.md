# PDF annotations

State: not started

Per-user comments and highlights on PDFs, stored in the backend so any client can read them
back. Comments and highlights share one table and one anchor — only the payload differs: a
comment has text, a highlight doesn't.

## Build
- [ ] ? h · Schema: one annotation table — `id`, `user_id`, `book_id`, kind (`comment` | `highlight`), the anchor, the comment text, `created_at` / `updated_at` — with its foreign keys declared and `ON DELETE CASCADE` from `users` and `books` (foreign keys are only enforced where declared, and `books.series` shows they aren't declared everywhere) [DB]
- [ ] ? h · Model and `FromRow` / `Insert` / `Search` / `Update` impls for the table [BE]
- [ ] ? h · Endpoints to list, create, edit and delete annotations, behind `AuthSession` and scoped to the session's user — another user's annotations are never returned, and no `user_id` is taken from the request [BE]
- [ ] ? h · Reader UI: highlight a selection or region, attach a comment, show existing annotations, delete one [FE]

## Open decisions
- [ ] What is the anchor? — blocks: the schema. Suggested: page index plus a normalized rectangle, stored as one opaque locator string the backend never interprets — the same shape the [reading position](41-reading-position.md) settles on, so there is one locator design, not two.
- [ ] Annotations in the database, or written into the PDF? — blocks: the schema. Suggested: database rows only — the file stays byte-identical, so `file_hash` and `last_modified` don't change and a re-scan sees nothing.
- [ ] Private to the owner, or shareable? — blocks: the endpoints. Suggested: private only; revisit if someone asks.
- [ ] Should the server check a page index against the book's page count? `books.page_count` is NULL for every book today — blocks: the endpoints. Suggested: accept the client's page unvalidated until the scanner fills `page_count`.
- [ ] PDF only, or also CBZ / image comics? — blocks: nothing yet. Suggested: PDF only, as the roadmap scopes it; a page-plus-rectangle anchor would carry over if ever wanted.
