# Two-factor auth (TOTP) — found

- `totp_management.rs` · no endpoint hands out fresh recovery codes while 2FA stays on; only disabling and enabling again does · a user down to their last codes has no clean way to get new ones
- `error_types.rs` · a 429 doesn't say how long it lasts (no `Retry-After`), for login's throttle neither · with code locks of up to 24 h the app can't tell the user when to try again
- `api_routes.rs` · on the LAN the API is plain HTTP, so the setup answer carries the TOTP secret to anyone listening on the network · until [32](features/32-https-tls.md) (HTTPS) lands, enrolling is safe only over Tailscale
- `db_sessions.rs` · `delete_for_device`, `delete_by_token_hash` and `rotate_tokens` are hand-written SQL the generic functions can express (`Delete` with two columns, `Delete` with `Or`, a read plus `update_multiple`) · they break the new ARCHITECTURE rule to use the generic functions
- `error_types.rs` · `ApplicationSetUpErrors::KeyEncodingFailure` is never constructed, and doc 33's cleanup task doesn't list it · dead code
