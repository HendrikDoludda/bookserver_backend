# Translation catalog (i18n)

State: in progress

The backend owns one translation catalog and serves it over the API, so both UIs and the
emails read the same strings, and adding a language is data entry instead of a code change.
The shipped text lives in `translations/<language>/` and is synced into the database at every
start; the API, the mails and the [editor](56-translation-editor.md) read only the database.
Keys belong to the code and values to the admins; only UI text is translated — library and
series names are user data.

## Build
- [x] 2 h · Schema: `locales` (code, display name, optional icon, default, enabled, sort order — at most one default, and it is enabled), `translation_keys` (`frontend` or `backend`, namespace, name, optional translator hint, `UNIQUE (namespace, name)`) and `translations` — a junction keyed `(locale_code, key_id)` with no `id`, so like `library_elements` it gets dedicated functions instead of `Extract` [DB]
- [ ] 1 h · The shipped files, compiled into the binary: `translations/<code>/language.json` (display name, sort order, optional icon) and one JSON file per namespace under `frontend/` (sent to the apps) or `backend/` (rendered by the server). Nested objects become dotted names, so a plural is one key per form — `unread.one` / `unread.other`; `"@name"` is a translator hint, English only. A file that breaks a rule — a name other than `a-z 0-9 _`, a folder name that isn't a language code, a key English lacks, `{placeholders}` that differ from English — stops the server, naming the file and the key; a test runs the same checks on the real files [BE]
- [ ] 2 h · Sync the shipped files into the tables at every start, in one transaction: a new language gets its row, which the admin owns from then on, and English becomes the default when there is none; keys follow the English files, and removed ones are deleted with their translations; shipped texts are inserted or updated, and one the files no longer have is deleted [DB] [BE]
- [ ] 2 h · English and German text for every API error key (`frontend/errors.json`) and the three code mails (`backend/email.json`); English is the source, the fallback and the default. A test fails for an error key without English text [BE]
- [ ] 1 h · `GET /api/v1/locales`: the enabled languages in sort order — code, display name, icon, whether it is the default — without a session [BE]
- [ ] 2 h · `GET /api/v1/locales/{code}/translations`: the `frontend` strings grouped by namespace, without a session; an unknown or disabled language answers 404 `not_found` [BE]
- [ ] 1 h · Missing values fall back to English in the same query (`translation_keys LEFT JOIN translations` + `COALESCE`), so no client ever gets an empty string [BE]
- [ ] 1 h · `?sections=settings,library` returns only those namespaces, filtered in Rust after the one fixed query — no names in the SQL [BE]
- [ ] 2 h · The ETag is the BLAKE3 hash of the answer, sent with `Cache-Control: no-cache`; a matching `If-None-Match` answers `304 Not Modified` [BE]
- [ ] 1 h · `PUT /api/v1/account/settings` and sign-up accept only an enabled locale (400 `invalid_locale`), in place of [31](31-per-user-settings.md)'s shape check; `user_settings.locale` becomes a foreign key to `locales`, `ON DELETE SET NULL` [DB] [BE]
- [ ] 1 h · Sign-up takes an optional `locale` and stores it with the new account, so its first mail is already in that language [BE]
- [ ] 1 h · Interpolation for text the backend renders, such as emails — a `{name}` replace in one pass, so an inserted value is never replaced again; no ICU needed [BE]
- [ ] 3 h · Every code mail — email verification, password reset, email change — takes its subject and body from the catalog, in the recipient's language setting if that language is enabled, else the default [BE]
