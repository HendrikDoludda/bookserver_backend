# API route naming

State: not started

The routes under `/api/v1` are RPC-style verbs (`/get_libraries`, `/delete_library/{id}`,
`/setup_db`). Whatever shape they should have is cheapest to settle now: Tier 1 is about to
add a dozen auth routes, and once a frontend calls them, every rename changes two codebases
at once.

## Build
- [ ] ? h · Rename the existing routes to the chosen shape, and update the new route names in the Tier 1 docs ([03](03-login-and-sessions.md), [27](27-registration-and-email-verification.md), [29](29-account-management.md), [30](30-password-reset.md)) to match [BE]

## Open decisions
- [ ] RPC-style verbs, or resource paths (`GET /libraries`, `DELETE /libraries/{id}`)? And namespaced per media type (`/api/v1/books/…` beside `/api/v1/auth/…` and `/api/v1/libraries`), so the optional [modular core](94-modular-core.md) wouldn't break every client later? — blocks: renaming the routes. Suggested: resource paths, namespaced per media type — the same reasoning that put `/api/v1` in first: trivial now, painful once the frontend couples to it.
