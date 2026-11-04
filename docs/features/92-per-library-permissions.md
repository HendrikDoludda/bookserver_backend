# Per-library permissions

State: not started

Control which users see which libraries — a kids' library, for example. Optional, and only
worth weighing once roles exist ([28](28-roles-and-access-control.md)).

## Build
- [ ] ? h · User↔library access table with the composite key `(user_id, library_id)` and no `id` — dedicated `Database` methods instead of `Extract`, like `library_elements` [DB]
- [ ] ? h · Library, series, book, search and file endpoints return only what the calling user may see, with `user_id` from the session [BE]
- [ ] ? h · Assign libraries to users in the admin view [FE]

## Open decisions
- [ ] One access model, or several? [Sensitive content controls](10-sensitive-content-controls.md) (per-user visibility) and [download permissions](24-download-permissions.md) (who may keep the original file) want the same user↔library axis — blocks: the access table. Suggested: one table with a column per capability, decided when the role model lands — one visibility check is easier to reason about than two that can disagree.
