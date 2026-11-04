# Manual metadata editing

State: not started

An admin can edit any metadata field by hand — to fix a wrong automatic match, or to describe a
sensitive item that must never be looked up online. A later enrichment run never overwrites a
hand-edited field.

## Build
- [ ] ? h · Endpoint to edit a book's metadata fields, behind `AdminSession` — library and series editing live in [01](01-library-and-series-management.md) [BE]
- [ ] ? h · Remember which fields were edited by hand (a per-field marker), and skip them in every later merge [DB] [BE]
- [ ] ? h · Edit screen for any metadata field of a book or series [FE]
