# Metadata enrichment

State: not started

Fill in what filenames can't tell — descriptions, covers, tags, release years, authors — from
online databases. One `MetadataProvider` picks the adapter from the library's type; a title
search returns candidates, and only a confirmed or auto-matched candidate is merged into the
entry. The adapters are [63](63-book-metadata.md) books, [64](64-manga-metadata.md) manga and
[65](65-comic-metadata.md) comics.

## Build
- [ ] 1 h · Store the provider's external id, per the external-id decision, so an entry can be re-fetched without searching again (the TODO at the top of `api_caller.rs` asks for this link) [DB]
- [ ] 1 h · Add an HTTP client with `rustls` features, matching [32](32-https-tls.md); `serde_json` is already a dependency [BE]
- [ ] 3 h · `MetadataProvider`: one adapter per source, chosen from the library's `library_type`, per the provider-chain decision — start with an `enum` and one `match` (an `async fn` trait isn't object-safe without `async-trait`) [BE]
- [ ] 2 h · Title search through the library's provider, returning candidates without writing anything [BE]
- [ ] 4 h · Merge a confirmed candidate — description, cover, tags, release years, author — and store the external id, in one transaction per entry; fetched covers go into `cover_images/` like extracted ones [BE]
- [ ] 1 h · Treat the scanner's placeholders (`"Unknown Author"`, `BookLanguage::Other("Unknown")`) as empty when merging [BE]
- [ ] 1 h · Merge tags into the comma-joined `books.tags` through one split, de-duplicate and re-join helper, per the tags decision [BE]
- [ ] 2 h · Auto-match per the auto-match decision, logging every auto-match [BE]
- [ ] 3 h · Cache provider results per the cache decision, so a repeated scan doesn't call the provider again [DB] [BE]
- [ ] 2 h · Respect each provider's documented rate limit (not one global number); hitting it gives a logged, non-500 outcome [BE]
- [ ] 2 h · Store provider API keys per the key-storage decision; never log a key [BE]
- [ ] 1 h · Sensitive items skip every online lookup: one check in the provider dispatch, not per adapter, so a new adapter can't forget it ([10](10-sensitive-content-controls.md)) [BE]
- [ ] 3 h · Per-entry search and confirm endpoints, per the endpoint decision [BE]
- [ ] 4 h · Bulk enrichment pass behind `POST /scan_metadata`, run in the background — not inline, where an unresponsive provider would stall the response — per the endpoint decision [BE]
- [ ] 0.5 h · Require `AdminSession` on the enrichment endpoints — today `scan_for_metadata` takes an `AuthSession` [BE]
- [ ] 4 h · Candidate list and confirm screen [FE]

## Open decisions
- [ ] External id on `series`, `books`, or both? — blocks: storing the external id. Suggested: `series` first — that is where providers describe things; a per-book id only if single volumes need matching.
- [ ] One provider per library type, or a fallback chain? — blocks: `MetadataProvider`. Suggested: one per type to start, shaped so a chain can be added without changing call sites.
- [ ] When may it auto-match instead of asking? — blocks: auto-match. Suggested: only on a single exact match of the normalised title.
- [ ] Where do API keys live: env vars, `secrets.toml`, or a database table the admin web view can edit? — blocks: key storage. Suggested: the database, with env vars as the bootstrap fallback.
- [ ] Cache in a database table or in memory? — blocks: the cache. Suggested: a table keyed by provider and query or external id — it survives restarts.
- [ ] Keep `books.tags` a comma-joined string, or make tags a relation? — blocks: tag merging. Suggested: keep the string for now; a relation also changes the search plan in [11](11-search.md).
- [ ] Is `POST /scan_metadata` the bulk pass or a single lookup? — blocks: the endpoints. Suggested: the bulk pass, with separate per-entry search and confirm endpoints.
- [ ] Add the `is_sensitive` column here, since enrichment (Tier 3) ships before the sensitive toggle (Tier 4)? — blocks: the sensitive skip. Suggested: yes — add it with this feature and honour it from the first request.
- [ ] Can an admin bypass the skip for one sensitive item? — blocks: the sensitive skip. Suggested: no — a sensitive item gets hand-entered metadata only ([66](66-manual-metadata-editing.md)).
