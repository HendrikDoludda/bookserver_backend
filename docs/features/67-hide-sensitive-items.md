# Hide sensitive items from default views

State: not started

Sensitive libraries and series stay out of lists and search results unless the request
explicitly asks for them. An optional extra on top of the
[sensitive content flag](10-sensitive-content-controls.md).

## Build
- [ ] ? h · Leave sensitive items out of `get_libraries`, `get_series_in_library` and `get_series_children` unless the request opts in, filtering where the filtering decision says [BE]
- [ ] ? h · The same filter on [search](11-search.md) results [BE]
- [ ] ? h · "Show sensitive items" opt-in [FE]

## Open decisions
- [ ] Filter in the backend or the frontend? The TODO in `db.rs` leans backend — blocks: the filter. Suggested: backend, in SQL — a client-side filter is cosmetic, since the titles are already on the wire.
- [ ] Build this together with the flag (Tier 4) instead of leaving it optional? — blocks: its tier. Suggested: yes — it is one query predicate plus a request flag.
