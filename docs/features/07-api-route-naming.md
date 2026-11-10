# API route naming

State: completed

Routes under `/api/v1` are resource paths, namespaced per area or media type (`/api/v1/auth/…`,
`/api/v1/account/…`, `/api/v1/libraries`, `/api/v1/books/…`), so the optional
[modular core](94-modular-core.md) won't break every client later. The RPC-style verbs
(`/get_libraries`, `/delete_library/{id}`) are renamed before a frontend couples to them, and
`/setup_db` goes.

## Build
- [x] 2 h · Rename the existing routes to namespaced resource paths (`GET /libraries`, `DELETE /libraries/{id}`, …) [BE]
- [x] 0.5 h · Remove `POST /api/v1/setup_db` and `initiate_database` — the migrations are compiled in and run at every start, so the endpoint never has anything to do, and its `Database::new()` builds and drops a second connection pool. Drop its mentions in `project-state.md`, the README's API table and [28](28-roles-and-access-control.md) [BE]
