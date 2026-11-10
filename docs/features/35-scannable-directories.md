# Scannable directories

State: not started

Which directories get scanned is set by the `BOOK_DIRS` env var today (default
`../Downloads`), so changing the list means a restart. This moves the list into the
database, managed through the API from both UIs.

## Build
- [ ] 1 h · Scannable-directories table, shaped per the directories decision [DB]
- [ ] 3 h · Endpoints to add, list and remove scannable directories [BE]
- [ ] 1 h · Reject a directory that doesn't exist when it is added — today a missing `BOOK_DIRS` entry still gets a library row before its scan fails [BE]
- [ ] 2 h · Scan one directory: `POST /directories/{id}/scan` (answers 501 until then) [BE]
- [ ] 2 h · Scan the directories from the table and stop reading `BOOK_DIRS` [BE]
- [ ] 1 h · A directory gets a required library type when it is added — nothing can infer it from the files, and today every auto-created library is hard-coded to `LibraryType::Books` and named after the lowercased path [BE]
- [ ] 3 h · Add and remove directories from the frontend [FE]

## Open decisions
- [ ] Does a directory replace the library or sit beside it? Today they are one-to-one (one auto-created library per `BOOK_DIRS` entry), which curated libraries with hand-picked series break — blocks: the directories table. Suggested: two tables — a directory points at a default library for newly found series, and series can be assigned to further libraries independently.
