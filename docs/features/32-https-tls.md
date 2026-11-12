# HTTPS (TLS)

State: completed

Login carries the password and the TOTP code, and the 2FA setup answer carries the TOTP secret
itself, so they travel over HTTPS. `tailscale serve` provides it: Tailscale answers on the
server's tailnet name with a certificate it renews itself, and forwards to the server on
`127.0.0.1`. HTTPS for installs without Tailscale is part of [52](52-packaging-and-deployment.md).

## Build
- [x] 1 h · `BIND_ADDRESS` setting, an IP address (default `0.0.0.0`): the address `start_server` listens on — `127.0.0.1` behind `tailscale serve` keeps plain HTTP off the LAN; anything else stops the server at start [BE]
- [x] 0.5 h · `PORT`: 1–65535 (default 8080), read together with `BIND_ADDRESS`; anything else stops the server at start, naming it [BE]
- [x] 0.5 h · `run-deploy/tailscale.md`: MagicDNS and HTTPS certificates on in the admin console, `tailscale serve --bg 8080`, the server started with `BIND_ADDRESS=127.0.0.1`
- [x] 0.5 h · Check by hand: on a tailnet device the `*.ts.net` URL answers without a certificate warning, and plain HTTP to the LAN address on port 8080 is refused
