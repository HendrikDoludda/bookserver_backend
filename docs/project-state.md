# Project state

Not released yet. What works when the server runs today — every endpoint except the health check
and the auth routes needs a logged-in user:

- **Scanning** the `BOOK_DIRS` folders (`POST /api/v1/scan`, admin only): PDF, EPUB, CBZ and image-folder books become book, series and library rows; volume, chapter and page come from the file names; one cover per book is extracted. Only lowercase extensions are recognised, and PDFs need the PDFium library — without it they are left out.
- **Streaming** a book file with HTTP range requests (`GET /api/v1/books/{id}/file`).
- **Browsing**: the libraries (`GET /api/v1/libraries`), a library's series (`GET /api/v1/libraries/{id}/series`), a series' books (`GET /api/v1/series/{id}/books`) and one book's metadata (`GET /api/v1/books/{id}`).
- **Deleting a library** (`DELETE /api/v1/libraries/{id}`, admin only) — its series and books stay.
- **Health check** (`GET /health`).

Next milestone: Tier 1 — the accounts, sessions, permissions and HTTPS the app needs before it logs in ([checklist](build-checklist.md)).
