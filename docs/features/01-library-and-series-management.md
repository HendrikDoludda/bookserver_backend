# Library & series management

State: in progress

The scanner creates libraries and series on its own; this makes them manageable — create a
library and assign series to it, edit and delete both — through the same API for the
frontend and the admin view. Only deleting a library exists today.

## Build
- [ ] 3 h · Create a library and assign existing series to it: `POST /libraries`, which answers 501 until then [BE]
- [ ] 2 h · Edit a library's name, type and description: `PATCH /libraries/{id}`, which answers 501 until then [BE]
- [ ] 2 h · Edit a series' name, description and release years: `PATCH /series/{id}` — no route exists [BE]
- [ ] 1 h · Delete a series: `DELETE /series/{id}` — no route exists [BE]
- [ ] 2 h · Make deleting a library follow the library-delete decision — today its `library_elements` rows cascade away, but `books.series` has no foreign key, so its series and books are left behind [DB] [BE]
- [ ] 0.5 h · `DELETE /libraries/{id}` answers 200 "Library deleted" for an id no library has — `remove_entry` returns the deleted-row count; answer 404 `not_found` when it is 0 [BE]
- [ ] 0.5 h · Delete the unused `GetLibraryRequest` [BE]
- [ ] 3 h · Create a library and assign series to it from the frontend [FE]
- [ ] 3 h · Edit and delete libraries and series from the frontend [FE]

## Open decisions
- [ ] What happens to a library's series and books when it is deleted? — blocks: deleting a library. Suggested: add the missing `books.series` foreign key, and make deleting a library *unlink* its series rather than delete them — removing a curated grouping should not remove content.
