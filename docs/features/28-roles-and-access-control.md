# Roles & access control

State: in progress

Admins and members. Admin-only actions — database setup, scanning, deleting libraries — need
an admin session; every other endpoint needs a logged-in user. The `AdminSession` and
`AuthSession` extractors exist, but no routed endpoint uses them yet.

## Build
- [ ] ? h · Require `AdminSession` on `POST /setup_db`, `POST /scan_all_directories` and `DELETE /delete_library/{id}` — today they need no session at all [BE]
- [ ] ? h · Require `AuthSession` on every other endpoint except the liveness ping and the public auth routes (login, sign-up, email verification, password reset) [BE]

## Open decisions
- [ ] Keep the `is_admin` boolean, or switch to a `role` column? — blocks: requiring `AdminSession`. Suggested: keep `is_admin`; revisit only if the optional per-library permissions feature is accepted.
