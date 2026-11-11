# Translation catalog (i18n)

State: not started

The backend owns one translation catalog and serves it over the API, so both UIs and the
emails read the same strings, and adding a language is data entry instead of a code change.
Keys belong to the code and values to the admins; only UI text is translated — library and
series names are user data. Built right after Tier 1, so the frontend's first screen can use it.

## Build
- [ ] 2 h · Schema: `locales` (code, display name, icon, default, enabled, sort order), `translation_keys` (namespace, key, optional translator hint, `UNIQUE (namespace, key)`) and `translations` — a junction keyed `(locale_code, key_id)` with no `id`, so like `library_elements` it gets dedicated `Database` methods instead of `Extract` [DB]
- [ ] 2 h · Seed the keys and the English text from the repo — in early dev inside migration 0001, later as a seeder so re-seeding needs no database wipe [DB] [BE]
- [ ] 2 h · `GET /api/v1/i18n/{locale}` returning the locale's strings grouped by namespace, readable per the access decision [BE]
- [ ] 1 h · Missing values fall back to English in the same query (`translation_keys LEFT JOIN translations` + `COALESCE`), so no client ever gets an empty string [BE]
- [ ] 1 h · `?sections=settings,library` returns only those namespaces — bind the names as parameters (`?1..?n`, like `extract_batch`), never format them into the SQL [BE]
- [ ] 3 h · The catalog version as an ETag, per the version decision: answer `304 Not Modified` when `If-None-Match` matches, and bump the version in the same transaction as every catalog write [BE]
- [ ] 1 h · Interpolation for text the backend renders, such as emails — a small `{name}` replace helper; no ICU needed [BE]
- [ ] 3 h · Every code mail — email verification, password reset, email change — takes its subject and body from the catalog, in the language per the email-language decision, else the default [BE]
- [ ] 1 h · `PUT /api/v1/account/settings` accepts only an enabled locale, in place of [31](31-per-user-settings.md)'s shape check [BE]

## Open decisions
- [ ] Is the catalog readable without a session? — blocks: the GET endpoint. Suggested: yes — the login screen needs strings before anyone signs in, and it is only UI text; writes stay admin-only ([56](56-translation-editor.md)).
- [ ] Plural forms: one key per plural category (`library.unread.one` / `.other`), or ICU MessageFormat values? — blocks: seeding the keys. Suggested: one key per category — admins should only ever type prose.
- [ ] One catalog version, or one per language? — blocks: the ETag. Suggested: one per language — an edit to German shouldn't make every English client download again.
- [ ] Which language does a code mail use — the server default, the `Accept-Language` header, or a locale sent with the sign-up? — blocks: code mails from the catalog. Suggested: the recipient's [per-user setting](31-per-user-settings.md), filled right away from a locale field in the sign-up request, so even the first email is localized.
