# HTTPS (TLS) — checks

**On this Mac.** Stop the server first; `cargo run` rebuilds with this step's code.

- [ ] A bad address stops the server at start, naming it → the output ends with `the setting BIND_ADDRESS has an invalid value`
  ```
  BIND_ADDRESS=localhost cargo run
  ```
- [ ] The same for the port → `the setting PORT has an invalid value`
  ```
  PORT=99999 cargo run
  ```
- [ ] Start the server for this machine only — [`run`](run-deploy/tailscale.md#commands), with your `BOOK_DIRS` — then, in a second terminal → `pong → 200`
  ```
  curl -s http://localhost:8080/health -w ' → %{http_code}\n'
  ```
- [ ] The same Mac's LAN address is refused → ` → 000`. `PASTE_LAN_IP` is what `ipconfig getifaddr en0` prints.
  ```
  curl -s --max-time 5 http://PASTE_LAN_IP:8080/health -w ' → %{http_code}\n'
  ```

**On the other Mac**, signed in to Tailscale, with this step's code (your commit, pulled there).

- [ ] The `tailscale` command prints its version. If it isn't found: with the standalone app, install it from the app's settings (CLI integration); with the App Store app, run [`cli-alias`](run-deploy/tailscale.md#commands) first.
  ```
  tailscale version
  ```
- [ ] In the admin console under DNS, MagicDNS and HTTPS Certificates are on. Start the server — [`run`](run-deploy/tailscale.md#commands) — then, in a second terminal, [`serve`](run-deploy/tailscale.md#commands). `PASTE_TS_URL` below is the `https://….ts.net` URL that `serve` prints, without its trailing `/`.
- [ ] Health over HTTPS → `pong → 200`. A certificate problem shows ` → 000` instead: curl refuses a certificate it doesn't trust.
  ```
  curl -s PASTE_TS_URL/health -w ' → %{http_code}\n'
  ```
- [ ] Sign up over HTTPS → `{"status":"ok",…} → 200` — only if that Mac's database has no account yet; its start log then shows the code for `PASTE_SETUP_CODE`. The other `PASTE_` values are your own choice.
  ```
  curl -s -X POST PASTE_TS_URL/api/v1/auth/register -H 'Content-Type: application/json' -d '{"email":"PASTE_EMAIL","username":"PASTE_USERNAME","password":"PASTE_PASSWORD","password_repeat":"PASTE_PASSWORD","device_name":"terminal","platform":"macos","setup_code":"PASTE_SETUP_CODE"}' -w ' → %{http_code}\n'
  ```
- [ ] Log in over HTTPS with that username and password → `{"status":"ok",…} → 200`
  ```
  curl -s -X POST PASTE_TS_URL/api/v1/auth/login -H 'Content-Type: application/json' -d '{"username_or_email":"PASTE_USERNAME","password":"PASTE_PASSWORD","device_name":"terminal","platform":"macos"}' -w ' → %{http_code}\n'
  ```
- [ ] Optional: your phone, with Tailscale on, opens `PASTE_TS_URL/health` in the browser — `pong`, and no certificate warning.

[`serve-off`](run-deploy/tailscale.md#commands) stops serving afterwards, if you don't want to keep it.
