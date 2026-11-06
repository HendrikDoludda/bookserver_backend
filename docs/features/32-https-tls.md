# HTTPS (TLS)

State: not started

The server speaks HTTPS itself, even on the LAN, because login carries the password and later
the TOTP code. The certificate comes from `tailscale cert`: a real certificate for the
server's tailnet name, so browsers trust it without any setup.

## Build
- [ ] 3 h · Serve over TLS with `axum-server` (feature `tls-rustls`, not a dependency yet): `RustlsConfig::from_pem_file(cert, key)` and `bind_rustls` [BE]
- [ ] 1 h · Settings for the certificate and key paths — no `TLS_*` setting exists yet [BE]
- [ ] 2 h · Pick up a renewed certificate without a restart (`RustlsConfig::reload_from_pem_file`), triggered per the reload decision [BE]
- [ ] 1 h · Check by hand: inside the tailnet the browser shows no certificate warning, and a renewed certificate is served without a restart

## Open decisions
- [ ] What triggers the reload — a timer, a signal, or watching the certificate files? — blocks: picking up a renewed certificate. Suggested: a daily timer — the certificates last 90 days, so polling is plenty and needs no extra crate.
