# Translation editor

State: not started

In the [admin web view](06-admin-web-view.md), an admin adds languages and fills in
translations: one tab per namespace, one input box per key, with the English text as the
placeholder. Keys come from the code, so the editor can't add or delete them.

## Build
- [ ] ? h · Admin-only endpoints to add and edit a language: display name, icon (an emoji, or a short id the frontend maps to a bundled icon), enabled, default, sort order [BE]
- [ ] ? h · Admin-only endpoint saving one value (`INSERT … ON CONFLICT(locale_code, key_id) DO UPDATE`) [BE]
- [ ] ? h · Mark admin-saved values, so the sync of the shipped files at start ([13](13-internationalization.md)) neither overwrites nor deletes them [DB] [BE]
- [ ] ? h · A saved value keeps the English text's `{placeholders}`, else 400 — the check the sync runs on the shipped files [BE]
- [ ] ? h · A new language's code has the shape of a language code (`de`, `pt-BR`) — the check the sync runs on folder names [BE]
- [ ] ? h · Saving an emptied box deletes the value, so the app shows the English text through the catalog's fallback and follows later English edits [BE]
- [ ] ? h · "N of M translated" per language, from the same `LEFT JOIN` as the catalog read [BE]
- [ ] ? h · Editor screen: one tab per namespace, one input per key with the English text as placeholder, progress per language [FE]
- [ ] ? h · Languages screen: add a language, set its icon, enable it, make it the default, order the list [FE]

## Open decisions
- [ ] Languages with more plural forms than English (Polish: `few`, `many`) need keys the English files don't have — how are they created? — blocks: adding such a language
