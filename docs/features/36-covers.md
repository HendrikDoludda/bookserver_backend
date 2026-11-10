# Covers

State: in progress

The scanner extracts one cover per book into `./cover_images/<file hash>.<ext>` — the first
page of a PDF, the first image of a CBZ or image folder, an image from an EPUB. Nothing
serves them yet, and series and libraries have no covers of their own.

## Build
- [ ] 2 h · Without the PDFium library, PDFs are left out of the catalog entirely: `Pdfium::default()` panics during cover extraction, which runs before the book row is inserted. Bind with a call that returns an error instead, so a missing library costs the cover, not the book [BE]
- [ ] 4 h · Bundle the PDFium library so PDF covers work on every OS, and on macOS without symlinking the dylib (TODO in `cover_image_retriever.rs`) [BE]
- [ ] 2 h · Serve covers at `GET /covers/{kind}/{id}` (now `ping_server`) through `ServeFile`, the way `request_file` streams books, with `kind` per the cover-route decision [BE]
- [ ] 3 h · Covers for series and libraries that a user can set, falling back to the first book's cover, per the cover-default decision — the columns exist, but the scanner always writes NULL and no endpoint sets them [BE]
- [ ] 3 h · Set a series or library cover from the frontend [FE]
- [ ] 3 h · EPUB: take the cover the OPF manifest names instead of `epub.images.first()`, and stop deriving the file extension from the media type (`image/svg+xml` becomes `.svg+xml`) [BE]
- [ ] 1 h · When hashing a file fails, its cover is written to `./cover_images.jpg` — one file beside the cover folder, shared by every such book — because the empty hash is joined to the folder path and `with_extension` renames the folder. Skip the cover when there is no hash [BE]
- [ ] 1 h · Check for an existing cover before extracting: CBZ never checks, and PDF renders the page before it checks [BE]
- [ ] 0.5 h · Check by hand: the cover extracted from a real `.epub` is the book's actual cover

## Open decisions
- [ ] What does `kind` in `GET /covers/{kind}/{id}` select — `book | series | library`, or full size vs thumbnail? — blocks: serving covers. Suggested: `book | series | library`; thumbnails stay with the optional [thumbnails](91-thumbnails.md) feature.
- [ ] Series and library fallback covers: worked out when read (the first book's cover while none is set), or written at scan time? — blocks: covers a user can set. Suggested: worked out when read — a stored fallback can't be told apart from a user's choice, so a re-scan could never safely update it.
