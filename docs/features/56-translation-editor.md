# Translation editor

State: not started

In the [admin web view](06-admin-web-view.md), an admin adds languages and fills in
translations: one tab per namespace, one input box per key, with the English text as the
placeholder. Keys come from the code, so the editor can't add or delete them.

## Build
- [ ] ? h · Admin-only endpoints to add and edit a language: display name, icon (an emoji, or a short id the frontend maps to a bundled icon), enabled, default, sort order [BE]
- [ ] ? h · Admin-only endpoint saving one value (`INSERT … ON CONFLICT(locale_code, key_id) DO UPDATE`), bumping the catalog version in the same transaction [BE]
- [ ] ? h · Saving an emptied box stores the English text instead, so no box in the app is ever empty [BE]
- [ ] ? h · "N of M translated" per language, from the same `LEFT JOIN` as the catalog read [BE]
- [ ] ? h · Editor screen: one tab per namespace, one input per key with the English text as placeholder, progress per language [FE]
- [ ] ? h · Languages screen: add a language, set its icon, enable it, make it the default, order the list [FE]
