# Frontend localization

State: not started

Every UI string goes through `t("key")` from the first screen, even while only English exists,
so adding a language later is catalog data instead of a rework. The client fetches the
[catalog](13-internationalization.md) once, caches it, and re-checks it by version.

## Build
- [ ] ? h · `t("namespace.key")` lookup for every UI string, from the first screen on [FE]
- [ ] ? h · Plural forms and interpolation (`"{count} unread"`) [FE]
- [ ] ? h · Load the catalog at runtime into a lookup map — most i18n libraries expect files at build time, so check remote-loading support (especially on Flutter) [FE]
- [ ] ? h · Cache the fetched catalog and ship English built in, so a cold or offline start still renders [FE]
- [ ] ? h · Re-check the cached catalog by its version / ETag (on app start, or daily) instead of downloading it every time [FE]
- [ ] ? h · Pick the language: the user's setting ([31](31-per-user-settings.md)), else the browser or OS locale, else the default [FE]
- [ ] ? h · Show each language's icon in the language list [FE]
- [ ] ? h · Check by hand: the first screen shows no literal UI strings while only English exists
