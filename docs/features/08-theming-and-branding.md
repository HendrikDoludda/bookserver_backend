# Theming & branding

State: not started

The app's look — corner radius, light and dark colour schemes, fonts, spacing — plus its name,
logo and favicon live in one place, and components only ever read from it. Re-skinning is then
one edit, not a change to every screen.

## Build
- [ ] ? h · Design tokens in the single source: border radius, colours for light and dark, fonts, spacing [FE]
- [ ] ? h · App name, logo and favicon as configurable values in the same source; an unset logo falls back to a default image [FE]
- [ ] ? h · Components read tokens (CSS variables or a theme object), never hard-coded values — from the first screen [FE]
- [ ] ? h · Ship a default token set with the frontend, so a cold or offline start still renders — if branding is backend-served [FE]
- [ ] ? h · `GET /branding`, stored per the storage decision — if branding is backend-served [BE]
- [ ] ? h · Version or ETag on the branding payload, so clients only re-download it when it changed [BE]
- [ ] ? h · Serve the logo and favicon from an asset route and return their URLs from `GET /branding`, per the logo decision; any uploaded file name goes through the path-containment check [BE]
- [ ] ? h · Branding screen in the [admin web view](06-admin-web-view.md) — if branding is backend-served [FE]
- [ ] ? h · Check by hand: changing one token changes every screen, light and dark swap the full palette, and a new app name shows up everywhere, including the browser tab

## Open decisions
- [ ] Frontend-owned tokens file, or backend-served `GET /branding`? — blocks: the [BE] tasks and the branding screen. Suggested: backend-served, like the translation catalog — the only option the admin web view can edit.
- [ ] If backend-served: a database table or a config file? — blocks: `GET /branding`. Suggested: a table — the admin web view edits it through the same API as everything else; a config file would also need router state beyond `Arc<Database>`.
- [ ] Logo and favicon: bundled with the frontend, or uploaded and served by the backend? — blocks: the asset route. Suggested: uploaded and served, so the JSON stays small and browsers cache the images normally.
