# Per-user settings

State: not started

Settings that belong to one user, starting with their language, which emails and both UIs
read to [localize](13-internationalization.md).

## Build
- [ ] ? h · Schema for per-user settings, shaped per the decision, holding the user's locale [DB]
- [ ] ? h · Endpoints to read and change the current user's settings, with `user_id` from the session [BE]
- [ ] ? h · Settings screen with a language picker [FE]

## Open decisions
- [ ] Columns on `users`, a typed `user_settings` table, or key/value rows? — blocks: the schema. Suggested: a typed `user_settings` table with `user_id` as primary key, like `totp` and `email_verification` — it keeps `users` about identity.
