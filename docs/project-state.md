# Project state

Not released yet. What works when the server runs today:

- **Scanning** the `BOOK_DIRS` folders (`POST /api/v1/scan`): PDF, EPUB, CBZ and image-folder books become book, series and library rows; volume, chapter and page come from the file names; one cover per book is extracted. Only lowercase extensions are recognised, and PDFs need the PDFium library — without it they are left out.
- **Streaming** a book file with HTTP range requests (`GET /api/v1/books/{id}/file`).
- **Book details**: one book's metadata (`GET /api/v1/books/{id}`) and the books of a series (`GET /api/v1/series/{id}/books`).
- **Deleting a library** (`DELETE /api/v1/libraries/{id}`) — its series and books stay.
- **Health check** (`GET /health`).

Next milestone: Tier 0 — the [database-layer fixes](features/26-database-layer-fixes.md) that make the library and series lists load, plus the [error responses](features/33-error-responses.md) and [route naming](features/07-api-route-naming.md) that Tier 1's auth routes build on.
