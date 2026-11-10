# Search

State: not started

One search box across titles, authors and tags, backed by SQLite's built-in full-text index
(FTS5) — no extra service. `GET /search` already exists as a placeholder.

## Build
- [ ] 3 h · FTS5 table over `books.title` / `author` / `tags` and `series.name` — pick the tokenizer up front (`porter` for stemming, or `unicode61 remove_diacritics 2`), since changing it means rebuilding the index [DB]
- [ ] 3 h · Keep the index in step with every write, per the index-sync decision [DB]
- [ ] 2 h · `GET /search` handler (answers 501 until then): the term goes in as a bound parameter, wrapped in double quotes so FTS syntax such as `*`, `AND` or `-` is treated as text; results shaped per the result decision [BE]
- [ ] 1 h · Ship an SQLite that has FTS5, per the SQLite decision [BE]
- [ ] 4 h · Search box and results view [FE]

## Open decisions
- [ ] Keep linking the system SQLite (`rusqlite` has no features today), or ship a bundled one? This Mac's SQLite (3.51) has FTS5, but the Windows and Docker builds give no such guarantee — blocks: shipping an SQLite with FTS5. Suggested: turn on `rusqlite`'s `bundled` feature in the same change as the FTS5 table — it also removes "works on my machine" surprises in packaging.
- [ ] How does the index stay current: an external-content FTS5 table (`content='books'`) with insert / update / delete triggers, or a rebuild at the end of each scan? — blocks: keeping the index in step. Suggested: the external-content table with triggers — the write paths are about to multiply (uploads, metadata enrichment, manual edits), and each would otherwise need remembering.
- [ ] What does a result look like: books only, books grouped under their series, or series and book hits together? — blocks: the search handler. Suggested: series and book hits in one response, each `WithId`-shaped and tagged with its type (`#[serde(tag = "type")]`), so the frontend renders two sections from one request.
