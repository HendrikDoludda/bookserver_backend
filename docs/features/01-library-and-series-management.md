# Library & series management

State: in progress

The scanner creates libraries and series on its own; this makes them manageable — create a
library and assign series to it, edit and delete both — through the same API for the
frontend and the admin view. Only deleting a library exists today.

## Build
- [ ] ? h · Create a library and assign existing series to it, per the request-shape decision — `POST /insert_entry` is still a hollow stub [BE]
- [ ] ? h · Edit a library's name, type and description, per the request-shape decision — `PUT /update_entry` is still a hollow stub [BE]
- [ ] ? h · Edit a series' name, description and release years — no route exists [BE]
- [ ] ? h · Delete a series — no route exists [BE]
- [ ] ? h · Make deleting a library follow the library-delete decision — today its `library_elements` rows cascade away, but `books.series` has no foreign key, so its series and books are left behind [DB] [BE]
- [ ] ? h · Delete the unused `GetLibraryRequest` [BE]
- [ ] ? h · Create a library and assign series to it from the frontend [FE]
- [ ] ? h · Edit and delete libraries and series from the frontend [FE]

## Open decisions
- [ ] One generic `insert_entry` / `update_entry` envelope, or typed endpoints per resource (`POST /libraries`, `PATCH /libraries/{id}`, `POST /series`, …)? — blocks: creating and editing libraries and series. Suggested: typed endpoints per resource — real request validation and honest status codes; the generic envelope pushes validation into the client.
- [ ] What happens to a library's series and books when it is deleted? — blocks: deleting a library. Suggested: add the missing `books.series` foreign key, and make deleting a library *unlink* its series rather than delete them — removing a curated grouping should not remove content.
