# Roles & access control

State: in progress

Admins and members. Admin-only actions — scanning, deleting libraries, managing users — need an
admin session; every other endpoint needs a logged-in user.

## Build
- [x] 1 h · `users.is_admin` becomes `role TEXT NOT NULL DEFAULT 'member' CHECK (role IN ('admin', 'member'))`, read as a `Role` enum: the first sign-up gets `admin`, and `AdminSession` checks it [DB] [BE]
- [x] 1 h · Require `AdminSession` on `POST /scan` and `DELETE /libraries/{id}` — today they need no session at all [BE]
- [x] 2 h · Require `AuthSession` on every other endpoint, the `not_implemented` placeholders included, except the liveness ping and the public auth routes (login, sign-up, email verification, password reset) — as a handler argument, like the TOTP handlers [BE]
- [ ] 2 h · `GET /api/v1/users` behind `AdminSession` lists every user — username, email, verified, role, created and last login; never the password hash — read through a new generic `Extract::extract_all` [BE]
- [ ] 2 h · `PATCH /api/v1/users/{id}` behind `AdminSession` with `{"role": …}` makes a user admin or member, refusing to demote the last admin with 409 `last_admin` — counted inside the transaction, so two parallel demotions can't both pass [BE]
- [ ] 1 h · The same `PATCH` with `{"email_verified": true}` marks a user's email verified — an account still unverified when SMTP is switched off can never get a code [BE]
