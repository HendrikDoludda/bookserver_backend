# File upload & ingestion

State: not started

A user uploads a file from the frontend; the backend places it in the right series folder
under the right name, then runs the scanner's own ingestion steps (hash, cover, database row).
The names it writes must be names the scanner reads back the same way, or a re-scan stops
recognising the server's own files.

## Build
- [ ] ? h · Enable axum's `multipart` feature and add the upload endpoint, gated per the who-may-upload decision; raise axum's 2 MB `DefaultBodyLimit` on this route only, not globally [BE]
- [ ] ? h · Stream each upload to a temp file on the target drive (`field.chunk()` in a loop — `field.bytes()` would hold a whole CBZ in memory), then `rename` it into place; a same-drive rename is atomic, so no partial file ever appears at the final path [BE]
- [ ] ? h · The backend decides folder and name from the shared naming convention, and the name must parse back through `get_final_file_name` to the same series, volume, chapter and page: keyword and number joined by one space, one underscore or nothing; whole numbers only; lowercase extension (the scanner's extension match is case-sensitive) [BE]
- [ ] ? h · Store exactly the `file_path` the scanner would compute — for image comics that is the folder (`get_final_folder_path`), not the image — or the next scan adds a duplicate row [BE]
- [ ] ? h · Ingest through the scanner's steps in `create_book_entry` (format, hash, series, cover, insert) inside `spawn_blocking` — cover extraction is blocking work — instead of a second implementation [BE]
- [ ] ? h · Refuse any write outside the chosen scannable root: sanitise user-supplied segments (series name, title), canonicalise the final path, and check it lies inside the root [BE]
- [ ] ? h · On a name collision, add a timestamp suffix instead of overwriting, and email the uploader (their `users.email`) the new name — per the rename-suffix decision [BE]
- [ ] ? h · Check the free space of the target drive before writing (no crate for this yet — e.g. `fs2::available_space`); if the file won't fit, write nothing and email the uploader [BE]
- [ ] ? h · Without an SMTP config, handle the rename and out-of-space notices per the no-SMTP decision [BE]
- [ ] ? h · Rename and out-of-space email text from the [translation catalog](13-internationalization.md), in the uploader's language ([31](31-per-user-settings.md)) [BE]
- [ ] ? h · "Where to store this + metadata" dialog, its field set picked from the library type: books and comics get series, volume and chapter (chapter only when uploading a single chapter) [FE]
- [ ] ? h · Drag a file into a series view to upload it [FE]

## Open decisions
- [ ] Who may upload — any logged-in user, or admins only? — blocks: the upload endpoint. Suggested: any logged-in user (`AuthSession`); `AdminSession` if write access should start narrow.
- [ ] How does a renamed file survive the title parser? `get_final_file_name` keeps everything except the three regex matches, so a `…_1736451200` suffix ends up in the title and resolves to a new series — blocks: collisions. Suggested: teach the parser to strip a recognised timestamp token — the naming convention is a shared spec, so both halves may know it.
- [ ] Without SMTP, what happens to the rename and out-of-space notices? — blocks: the no-SMTP task. Suggested: ingest anyway and report it in the HTTP response and the log — the email is a courtesy, not the record.
- [ ] What does uploading image pages mean? — blocks: storing the scanner's path. Suggested: an image comic is uploaded as a folder of pages and stored as that folder; single loose images are refused.
