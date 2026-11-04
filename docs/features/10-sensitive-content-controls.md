# Sensitive content flag

State: not started

An admin can mark a library or series as sensitive. Its titles are then never sent to an online
metadata service ([09](09-metadata-enrichment.md) owns that check). Further protections are
separate, optional features: [hidden from default views](67-hide-sensitive-items.md),
[per-user visibility](68-sensitive-per-user-visibility.md),
[blurred covers](69-blurred-sensitive-covers.md) and a
[PIN or re-login to open](62-sensitive-pin-or-reauth.md).

## Build
- [ ] ? h · Add `is_sensitive` (`BOOLEAN NOT NULL DEFAULT FALSE`) to `library` and `series` — unless [09](09-metadata-enrichment.md) adds it first — and to `LibraryMetadata` and `BookSeriesMetadata` in all five places: struct field, column enum, `Extract::COLUMNS`, `FromRow`, `Insert`. The last three are plain strings, so a miss fails at runtime, not at `cargo check` [DB] [BE]
- [ ] ? h · Admin-only toggle endpoint for a library and a series (`AdminSession`; `Update` already works for both tables) [BE]
- [ ] ? h · Compute effective sensitivity in the backend, per the inheritance decision, so every endpoint gives the same answer for an item [BE]
- [ ] ? h · Sensitive toggle on the library and series screens — the same endpoint from the main frontend and the admin web view [FE]

## Open decisions
- [ ] Does sensitivity inherit from library to series? — blocks: effective sensitivity. Suggested: an item is sensitive if its library or its series is — marking the library is enough.
