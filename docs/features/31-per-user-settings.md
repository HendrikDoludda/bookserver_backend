# Per-user settings

State: not started

Settings that belong to one user, starting with their language, which emails and both UIs
read to [localize](13-internationalization.md).

## Build
- [ ] 1 h · A typed `user_settings` table — `user_id` as primary key and foreign key, like `totp` — holding the user's `locale`, `NULL` while none is chosen [DB]
- [ ] 2 h · `GET` and `PUT /api/v1/account/settings` read and replace the current user's settings, with `user_id` from the session; without a row the answer is the defaults, and the first write creates it. Until the [catalog](13-internationalization.md) lists its locales, a locale is `ll` or `ll-RR` (`de`, `pt-BR`), else 400 `invalid_locale` [BE]
