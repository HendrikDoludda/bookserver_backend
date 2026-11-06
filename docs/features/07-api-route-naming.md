# API route naming

State: not started

Routes under `/api/v1` are resource paths, namespaced per area or media type (`/api/v1/auth/…`,
`/api/v1/account/…`, `/api/v1/libraries`, `/api/v1/books/…`), so the optional
[modular core](94-modular-core.md) won't break every client later. The Tier 1 auth routes are
built in that shape from the start; the existing RPC-style verbs (`/get_libraries`,
`/delete_library/{id}`, `/setup_db`) still have to be renamed before a frontend couples to them.

## Build
- [ ] 2 h · Rename the existing routes to namespaced resource paths (`GET /libraries`, `DELETE /libraries/{id}`, …) [BE]
