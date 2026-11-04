# Translation catalog (i18n)

State: not started

The backend owns one translation catalog and serves it over the API, so both UIs and the
emails read the same strings, and adding a language is data entry instead of a code change.
Keys belong to the code and values to the admins; only UI text is translated — library and
series names are user data. Built right after Tier 1, so the frontend's first screen can use it.

## Build
- [ ] ? h · Schema: `locales` (code, display name, icon, default, enabled, sort order), `translation_keys` (namespace, key, optional translator hint, `UNIQUE (namespace, key)`) and `translations` — a junction keyed `(locale_code, key_id)` with no `id`, so like `library_elements` it gets dedicated `Database` methods instead of `Extract` [DB]
- [ ] ? h · Seed the keys and the English text from the repo — in early dev inside migration 0001, later as a seeder so re-seeding needs no database wipe [DB] [BE]
- [ ] ? h · `GET /api/v1/i18n/{locale}` returning the locale's strings grouped by namespace, readable per the access decision [BE]
- [ ] ? h · Missing values fall back to English in the same query (`translation_keys LEFT JOIN translations` + `COALESCE`), so no client ever gets an empty string [BE]
- [ ] ? h · `?sections=settings,library` returns only those namespaces — bind the names as parameters (`?1..?n`, like `extract_batch`), never format them into the SQL [BE]
- [ ] ? h · The catalog version as an ETag, per the version decision: answer `304 Not Modified` when `If-None-Match` matches, and bump the version in the same transaction as every catalog write [BE]
- [ ] ? h · Interpolation for text the backend renders, such as emails — a small `{name}` replace helper; no ICU needed [BE]

## Open decisions
- [ ] Is the catalog readable without a session? — blocks: the GET endpoint. Suggested: yes — the login screen needs strings before anyone signs in, and it is only UI text; writes stay admin-only ([56](56-translation-editor.md)).
- [ ] Plural forms: one key per plural category (`library.unread.one` / `.other`), or ICU MessageFormat values? — blocks: seeding the keys. Suggested: one key per category — admins should only ever type prose.
- [ ] One catalog version, or one per language? — blocks: the ETag. Suggested: one per language — an edit to German shouldn't make every English client download again.
