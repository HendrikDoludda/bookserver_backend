# Two-factor auth (TOTP)

State: in progress

An optional second login factor: a 6-digit code from an authenticator app, computed from a
shared secret and the current time, so no delivery service is needed. Login already gives a
TOTP user a half-authenticated session (`authentication_completed = false`) that only the
code step accepts; setup, verification and disable are still comment-only stubs in
`totp_management.rs`, and their handlers answer 501.

## Build
- [ ] ? h · Pending state for an enrollment in progress, per the pending-secret decision — `totp` has no such column today [DB]
- [ ] ? h · `log_in` counts 2FA as enabled only for a confirmed `totp` row — otherwise a half-finished enrollment demands a code the user can't produce [BE]
- [ ] ? h · Secret generation and the `otpauth://` URI with `totp-rs`: switch on its `otpauth` feature (without it `to_url()` and the issuer and account fields don't exist), plus `gen_secret` or generate the bytes the way `create_new_token` does; import the crate's `Totp` under another name (`use totp_rs::Totp as TotpGenerator`) so it isn't confused with the `TOTP` model [BE]
- [ ] ? h · Derive `Deserialize` on `TOTPSignInRequest`, `RecoveryCodeSignInRequest` and `DisableTOTPRequest` (`totp_model.rs`) so they can be `Json<…>` bodies [BE]
- [ ] ? h · Start enrollment: route `totp_start_set_up` (now 501) behind `AuthSession` — generate a secret, store it as pending (replacing an earlier pending one), return the URI and the manual setup key [BE]
- [ ] ? h · Confirm enrollment: route `finalize_totp_set_up` (now 501) — check a live code against the pending secret with `check_current` (the `totp-rs` defaults already allow ±1 time step), mark it confirmed, and issue the [recovery codes](50-recovery-codes.md) [BE]
- [ ] ? h · Code step at login: route `sign_in_totp` (now 501) behind `AuthTOTPLoginSession`, per the login-flow decision — on a valid code, set `authentication_completed` on the existing session row instead of inserting a second one [BE]
- [ ] ? h · `verify_email_logic` hard-codes `user_has_totp_enabled: false`, so a session created by email verification skips the code step — run the same probe as `log_in` [BE]
- [ ] ? h · Disable 2FA: route `disable_totp` (now 501) — require the current password (already a field of `DisableTOTPRequest`), delete the `totp` row and all recovery codes, and end all of the user's sessions [BE]
- [ ] ? h · Protect the secret at rest, per the secret decision. If encrypted: the declared `chacha20-poly1305` 0.2.1 is the low-level rust-bitcoin crate — generate a fresh 12-byte nonce for every write, store nonce + ciphertext + tag, and never reuse a nonce with the same key; the key reaches handlers through `AppState` ([14](14-email-and-notifications.md)) [BE]
- [ ] ? h · Limit wrong codes per user, per the counter decision — a 6-digit code is only a million combinations, so this is the throttle that matters even behind Tailscale [BE]
- [ ] ? h · Code-entry step after the password, when login answers that a code is needed [FE]
- [ ] ? h · 2FA settings: enable (QR code + confirm) and disable [FE]

## Open decisions
- [ ] Store the secret in plaintext, or encrypted with the signing key from `secrets.toml`? A leaked secret generates valid codes forever — blocks: protecting the secret at rest. Suggested: encrypt — it is the signing key's only real job, which also settles the key question in [12](12-security-hardening.md).
- [ ] Log in with one request (password + code together) or two (half-session, then code)? The old roadmap said one; the code is built for two — blocks: the code step at login. Suggested: keep the two-step flow the code already has.
- [ ] Where does the pending secret live: a `confirmed` column on `totp`, a separate table, or memory? — blocks: the pending state. Suggested: a `confirmed BOOLEAN NOT NULL DEFAULT FALSE` column on `totp` — in early dev, an in-place edit of migration 0001.
- [ ] Where does the wrong-code counter live: in memory or in the database? — blocks: limiting wrong codes. Suggested: in memory in the shared state — it resets on restart, which is acceptable for a single-process home server.
- [ ] Reject a code that was already used in its 30-second window (replay protection)? — blocks: the pending state, if wanted. Suggested: skip for v1; if wanted, store the last-used time step (`check_current` returns it) in the same schema change as `confirmed`.
