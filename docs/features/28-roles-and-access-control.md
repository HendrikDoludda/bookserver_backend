# Roles & access control

State: in progress

Admins and members. Admin-only actions — database setup, scanning, deleting libraries — need
an admin session; every other endpoint needs a logged-in user. The `AdminSession` and
`AuthSession` extractors exist, but no routed endpoint uses them yet.

## Build
- [ ] 1 h · Require `AdminSession` on `POST /setup_db`, `POST /scan_all_directories` and `DELETE /delete_library/{id}` — today they need no session at all [BE]
- [ ] 2 h · Require `AuthSession` on every other endpoint except the liveness ping and the public auth routes (login, sign-up, email verification, password reset) [BE]
- [ ] 2 h · Admin endpoint listing all users — username, email, verified, admin, created and last login; never the password hash [BE]
- [ ] 2 h · Admin endpoint to make a user admin or member, per the role decision — refusing to demote the last admin [BE]
- [ ] ? h · Admin endpoint to mark a user's email verified — an account still unverified when SMTP is switched off can never get a code [BE]

## Open decisions
- [ ] Keep the `is_admin` boolean, or switch to a `role` column? — blocks: requiring `AdminSession`, changing a user's role. Suggested: keep `is_admin`; revisit only if the optional per-library permissions feature is accepted.
