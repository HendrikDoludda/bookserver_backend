# File reconciliation

State: not started

A scan only adds files that aren't in the database yet, so a book whose file was moved or
deleted keeps its row forever. Reconciliation runs the other way: it walks the `books` rows and
checks that each file is still there.

## Build
- [ ] ? h · Reconciliation pass over the `books` rows, checking that each `file_path` still exists — a separate pass, not a change to `create_book_entry` [BE]
- [ ] ? h · Find a moved file again by its `file_hash` and update its row, instead of deleting and re-inserting it [BE]
- [ ] ? h · Handle a file that is gone, per the prune-or-flag decision [DB] [BE]

## Open decisions
- [ ] Delete the rows of missing files, or flag them? — blocks: handling a file that is gone. Suggested: flag — an unplugged drive would otherwise empty the catalog.
