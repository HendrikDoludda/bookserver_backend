# Sorting, filtering & paging

State: not started

The list endpoints return everything, in whatever order SQLite hands it back. This adds
sorting (title, recently added, volume number), filters (finished or not, tag, author) and
paging — the backend half of the browse screens.

## Build
- [ ] ? h · Paging on the list endpoints per the paging decision, with a hard upper limit — `limit` and `offset` arrive through a `Query<ListParams>` struct of `Option` fields (the codebase's first `Query` extractor) and are bound as parameters [BE]
- [ ] ? h · Sort by title, recently added and volume number — the sort field is parsed into a `*DatabaseColumns` variant and rejected otherwise, never spliced in as text; titles sort with `COLLATE NOCASE` [BE]
- [ ] ? h · "Recently added" per the recently-added decision — `books` only has the file's `last_modified`, no time the row was added [DB] [BE]
- [ ] ? h · Indexes for the browse queries — no table has any, so filtering or sorting on `books.series`, `books.title` or `series.name` reads the whole table [DB]
- [ ] ? h · Filter by finished / not finished for the calling user, from [read state](02-read-state.md) [BE]
- [ ] ? h · Filter by tag, per the tag decision [BE]
- [ ] ? h · Filter by author [BE]
- [ ] ? h · Sorting controls [FE]
- [ ] ? h · Filter controls — leave author out until books have real authors; the scanner writes "Unknown Author" for every book until metadata enrichment fills it in [FE]

## Open decisions
- [ ] Offset/limit or cursor paging? — blocks: paging. Suggested: offset/limit with a hard ceiling — it fits page numbers and the A–Z jump, and the instability cursors fix (rows inserted while paging) isn't real for a home library.
- [ ] "Recently added": order by `id DESC`, or add a `created_at` column? — blocks: "recently added". Suggested: add `created_at` — this is the change that defines the sort keys anyway, and `id DESC` stops meaning "added" once rows get re-created.
- [ ] Tags are one comma-joined text column — filter on it as it is (match `,tag,` against `',' || tags || ','`), or move tags into `tags` + `book_tags` tables? — blocks: filtering by tag. Suggested: the two tables once an exact tag filter is built — matching inside the joined string is only right if every write normalizes the delimiters.
