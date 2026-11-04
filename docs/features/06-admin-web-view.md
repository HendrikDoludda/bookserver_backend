# Backend admin web view

State: not started

An admin UI served by the backend itself, for running the server — libraries, series,
scannable directories, users and scans — without the main app. It is just another client of
the same HTTP API: no endpoint or code path exists that only it can use.

## Build
- [ ] ? h · Serve the admin UI under `/admin` with its own SPA fallback (`ServeDir` with a `ServeFile` index fallback, mounted with `nest_service` on the outer router), so deep links reload while unknown `/api/v1/…` paths still get the API's 404 — shipped per the build decision [BE]
- [ ] ? h · If served from disk, read the UI directory from config, not a path relative to the working directory [BE]
- [ ] ? h · Admin UI shell with login, using the same token transport as the main frontend ([03](03-login-and-sessions.md)) [FE]
- [ ] ? h · Library and series management screens [FE]
- [ ] ? h · Scannable-directory management screen [FE]
- [ ] ? h · User management screen [FE]
- [ ] ? h · Scan screen: start a scan and follow its status [FE]
- [ ] ? h · Every visible string through `t("key")` against the backend catalog, with a language select — no locale files in the admin UI [FE]
- [ ] ? h · A 403 from an admin-only endpoint shows a refused state, not a blank screen [FE]

## Open decisions
- [ ] How is the UI built and shipped: static files served with `ServeDir`, assets embedded in the binary (`rust-embed` / `include_dir`), or server-rendered HTML? — blocks: serving the UI. Suggested: develop against `ServeDir` and embed before packaging, so the `.exe` / `.dmg` / Docker builds stay one self-contained binary; avoid server-rendered HTML, which makes admin-only shortcuts too easy.
- [ ] One frontend codebase with the main app, or two? — blocks: the admin UI shell. Suggested: two apps sharing conventions — the same `t("key")` calls, design tokens and auth mechanism.
