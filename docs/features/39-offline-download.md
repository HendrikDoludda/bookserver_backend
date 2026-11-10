# Offline download

State: not started

The app keeps a whole copy of a book to read offline — a different job from streaming, even
though it serves the same bytes. [Download permissions](24-download-permissions.md) will
later decide per user who may do this.

## Build
- [ ] ? h · Download endpoint per the download-route decision: `ServeFile` plus a `Content-Disposition: attachment; filename="…"` header [BE]
- [ ] ? h · Download a book for offline reading [FE]
- [ ] ? h · Check by hand: a downloaded book opens in the reader with the network off

## Open decisions
- [ ] A route of its own (`GET /books/{id}/download`), or a flag on `GET /books/{id}/file`? — blocks: the download endpoint. Suggested: a route of its own — easier to gate per user later and to tell apart from reading in the logs.
