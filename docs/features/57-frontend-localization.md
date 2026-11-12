# Frontend localization

State: not started

Every UI string goes through `t("key")` from the first screen, even while only English exists,
so adding a language later is catalog data instead of a rework. The client fetches the
[catalog](13-internationalization.md) once, caches it, and re-checks it by version.

## Build
- [ ] 2 h · `t("namespace.key")` lookup for every UI string, from the first screen on [FE]
- [ ] 2 h · Plural forms — one key per form (`unread.one` / `unread.other`), picked with the language's plural rules, `.other` when a form is missing — and interpolation (`"{count} unread"`) [FE]
- [ ] 3 h · Load the catalog at runtime into a lookup map — most i18n libraries expect files at build time, so check remote-loading support (especially on Flutter) [FE]
- [ ] 2 h · Cache the fetched catalog and ship English built in, so a cold or offline start still renders [FE]
- [ ] 1 h · Re-check the cached catalog with `If-None-Match` and its ETag (on app start, or daily) instead of downloading it every time; a `304` keeps the cached copy [FE]
- [ ] 1 h · Pick the language: the user's setting ([31](31-per-user-settings.md)), else the browser or OS locale if the server offers it, else the default from `GET /api/v1/locales` [FE]
- [ ] 1 h · Show each language's icon in the language list from `GET /api/v1/locales` [FE]
- [ ] 0.5 h · Check by hand: the first screen shows no literal UI strings
