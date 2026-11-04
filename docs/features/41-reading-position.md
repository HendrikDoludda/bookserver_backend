# Reading position format

State: not started

A page number works for PDF and CBZ, but EPUB text reflows and has no fixed pages — it needs
a locator (an EPUB CFI). The format has to be settled before the
[read-state](02-read-state.md) table is written; afterwards it costs a migration.

## Build
- [ ] ? h · Position type in Rust, per the position decision — an enum such as `Page(i64)` / `Locator(String)` converted at the database boundary, not two `Option`s that callers must check in the right order [BE]
- [ ] ? h · Fill `books.page_count` at scan time — page count for PDF, image count for CBZ and image folders; it is NULL for every book today, so "page 40 of 200" can't be shown [BE]
- [ ] ? h · EPUB reader saves and resumes from a locator [FE]
- [ ] ? h · PDF and CBZ readers save and resume from a page [FE]

## Open decisions
- [ ] What shape does the stored position take? Decide it together with where per-user state lives in the [modular core](94-modular-core.md) question — blocks: the read-state table, the position type. Suggested: an opaque `position TEXT` that the reader for the book's format writes and reads back, plus a `progress_percent REAL` the client also sends — the backend can then order "continue reading" and draw progress without ever parsing a CFI.
