# Per-user visibility of sensitive items

State: not started

Decide per user who may see sensitive libraries and series at all. An optional extra on top of
the [sensitive content flag](10-sensitive-content-controls.md); it overlaps with per-library
permissions ([92](92-per-library-permissions.md)) and
[download permissions](24-download-permissions.md).

## Build
- [ ] ? h · A user↔library visibility table, shaped per the access-model decision [DB]
- [ ] ? h · Apply it in every list and search query, with `user_id` from the session [BE]
- [ ] ? h · Per-user visibility settings in the [admin web view](06-admin-web-view.md) [FE]

## Open decisions
- [ ] One access model shared with per-library permissions and download permissions? Three features want a per-user, per-library setting — blocks: the visibility table. Suggested: one user↔library table with a column per capability, settled when the role model lands.
