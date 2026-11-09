# Two-factor auth (TOTP)

State: in progress

An optional second login factor: a 6-digit code from an authenticator app, computed from a
shared secret and the current time, so no delivery service is needed. Login gives a TOTP user a
5-minute login challenge instead of a session ([03](03-login-and-sessions.md)), and the code step
turns it into one. The secret is stored encrypted. Setup, confirmation and the status work; the
code step and disable are still comment-only stubs in `totp_management.rs`, and their handlers
answer 501.

## Build
- [x] 1 h · `totp` gets `confirmed BOOLEAN NOT NULL DEFAULT FALSE`, `last_used_step INTEGER`, `failed_attempts INTEGER NOT NULL DEFAULT 0`, `lockouts INTEGER NOT NULL DEFAULT 0` and `last_failed_at INTEGER` [DB]
- [x] 1 h · `log_in` counts 2FA as enabled only for a confirmed `totp` row — otherwise a half-finished enrollment demands a code the user can't produce [BE]
- [x] 2 h · Secret and `otpauth://` URI with `totp-rs`: switch on its `otpauth` feature (without it `to_url()` and the issuer and account fields don't exist); 20 random bytes from `SysRng`, the way `create_new_token` does; `Builder::new().with_secret(…).with_account_name(username).with_issuer(…)`; import the crate's `Totp` under another name (`use totp_rs::Totp as TotpGenerator`) so it isn't confused with the `TOTP` model [BE]
- [x] 4 h · Encrypt the secret at rest: replace the rust-bitcoin `chacha20-poly1305` with RustCrypto's `chacha20poly1305` 0.11 (`default-features = false, features = ["alloc"]`) and use XChaCha20-Poly1305; the key is derived from the signing key ([12](12-security-hardening.md)) with `blake3::derive_key`; every write gets a fresh 24-byte nonce from `SysRng` and stores base64 of nonce + ciphertext + tag, with the `user_id` as associated data. Lands together with [12](12-security-hardening.md)'s owner-only `secrets.toml`, which from then on holds the key [BE]
- [x] 0.5 h · Derive `Deserialize` on `TOTPSignInRequest`, `RecoveryCodeSignInRequest` and `DisableTOTPRequest` (`totp_model.rs`) so they can be `Json<…>` bodies — not `Debug`, they carry codes and passwords [BE]
- [x] 2 h · Start enrollment: route `totp_start_set_up` at `POST /api/v1/account/totp` behind `AuthSession` — 409 when a confirmed `totp` row exists, otherwise store a new secret as pending (replacing an earlier pending one) and return `{otpauth_url, secret_base32}`; the app draws the QR code [BE]
- [x] 3 h · Confirm enrollment: route `finalize_totp_set_up` at `POST /api/v1/account/totp/confirm` with `{code, password}` — re-check the password, so a stolen session token alone can't enroll the thief's phone; check the code against the pending secret, mark it confirmed, store its step, and issue the [recovery codes](50-recovery-codes.md) [BE]
- [x] 1 h · Confirming 2FA ends every other session of the user (`end_other_sessions`), so each live session has passed the second factor [BE]
- [x] 1 h · `GET /api/v1/account/totp` behind `AuthSession` answers `{enabled, recovery_codes_left}`, for the 2FA settings screen [BE]
- [ ] 3 h · Code step at login: route `sign_in_totp` (now 501) at `POST /api/v1/auth/login/totp` behind `AuthTOTPLoginSession`, which reads the login challenge ([03](03-login-and-sessions.md)); on a valid code, delete the challenge (exactly one row) and create a new session, so the pre-2FA token never becomes a full one [BE]
- [ ] 3 h · Limit wrong codes per user, authenticator and recovery codes together: count the attempt before checking the code — a count written in the same transaction as the failure is rolled back with it. Every 5 wrong codes lock the code checks with 429: 15 minutes the first time, doubling with each further lock up to 24 h; only a correct code resets the count and the locks [BE]
- [ ] 1 h · Replay protection: accept a code only for a time step newer than `last_used_step`, re-checked in the final transaction — confirming already stores its code's step, so that code can't log in [BE]
- [x] 0.5 h · Strip spaces from a submitted code — `check` accepts exactly 6 ASCII digits [BE]
- [x] 1 h · Email verification logs in through login's `start_login`, so a TOTP user gets the code step there too [BE]
- [ ] 3 h · Disable 2FA: route `disable_totp` (now 501) at `POST /api/v1/account/totp/disable` with `{password, code}` — require the current password and a current authenticator or recovery code (6 digits are read as an authenticator code), so a stolen session plus the password can't switch 2FA off; delete the `totp` row and all recovery codes, and end all of the user's sessions [BE]
- [ ] 2 h · Code-entry step after the password, when login answers `totp_required` [FE]
- [ ] 4 h · 2FA settings: enable (QR code + confirm) and disable [FE]
