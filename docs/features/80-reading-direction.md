# Reading direction

State: not started

A button in the PDF, CBZ and image-folder readers switches between right to left, left to right
and continuous scroll. Each user's choice is kept per series on the server; until there is one,
a book opens in the direction its file states, else left to right. EPUB lays out its own text
and gets no button; [handedness](79-handedness.md) only moves where the button sits.

## Build
- [ ] 1 h · Table of each user's reading mode per series: `(user_id, series_id)` as primary key, both declared foreign keys with `ON DELETE CASCADE`, the mode stored as text from a Rust enum (`rtl` / `ltr` / `continuous`) [DB]
- [ ] 0.5 h · Nullable `books.reading_direction` (`rtl` / `ltr`): the direction the file states, NULL when it states none [DB]
- [ ] 2 h · Scanner fills `books.reading_direction` from a `ComicInfo.xml` — inside a CBZ, or in an image folder: `<Manga>YesAndRightToLeft</Manga>` is `rtl`, `<Manga>No</Manga>` is `ltr` [BE]
- [ ] 1 h · `GET /books/{id}` answers the mode the calling user opens it in: their choice for the book's series, else `books.reading_direction`, else `ltr` [BE]
- [ ] 1 h · `PUT /api/v1/series/{id}/reading_mode` with `{"mode": …}` stores the calling user's choice; an unknown mode answers 400 `invalid_reading_mode`, an unknown series 404 `not_found` [BE]
- [ ] 0.5 h · Texts: the button and the three modes in a `reader` namespace, `invalid_reading_mode` in `errors`, in `translations/en/frontend/` and `translations/de/frontend/` [BE]
- [ ] 1 h · Reading-mode button in the PDF, CBZ and image-folder readers: switches at once and saves the choice with the `PUT` [FE]
- [ ] 2 h · Right to left and left to right: page order and swipe direction follow the mode [FE]
- [ ] 2 h · Continuous scroll: the pages run top to bottom in one scrolling column [FE]
- [ ] 2 h · Page-turn tap zones as one layout shared by the readers; in the two paged modes the "next page" edge follows the mode [FE]
- [ ] 0.5 h · Check by hand: a right-to-left manga goes forward by swiping right and tapping the left edge; the chosen mode carries over to the next volume and to another device
