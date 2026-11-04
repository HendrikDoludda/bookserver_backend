# Reading files

State: in progress

`GET /book/{id}` streams a book's file with HTTP range support through `tower-http`'s
`ServeFile`, and `GET /book_metadata/{id}` returns its metadata (404 for an unknown id). PDF,
EPUB and CBZ files stream; a folder of page images can't be served, and the readers don't
exist yet.

## Build
- [ ] ? h · Serve image-folder books page by page — their stored `file_path` is the folder, so `ServeFile` has no file to stream. A per-page route: book id + page index → the file at that position in natural-sort order, the order the cover picker uses [BE]
- [ ] ? h · Give the reader each page's dimensions for image-folder books, so it can lay pages out before they load (TODO in `request_file`) [BE]
- [ ] ? h · Delete the unused `StreamReaderErrors` and `convert_file_path_to_blob` — leftovers from a hand-rolled range reader that `ServeFile` replaced [BE]
- [ ] ? h · PDF reader [FE]
- [ ] ? h · EPUB reader [FE]
- [ ] ? h · CBZ and image-folder reader [FE]
