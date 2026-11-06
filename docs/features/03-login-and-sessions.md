# Login, logout & sessions

State: in progress

A user logs in with username or email plus password and gets a session: a 15-minute session
token and a 30-day refresh token, sent as `Authorization: Bearer` and stored only as BLAKE3
hashes in `sessions`. A user with [two-factor auth](04-two-factor-auth-totp.md) gets a 5-minute
login challenge instead, which the code step turns into a session. Logging out deletes the row.
Every per-user feature needs to know who is asking, so this is the core of Tier 1.

## Build
- [ ] 2 h · The extractors read `Authorization: Bearer <token>`, parsed by hand (no new crate), instead of cookies; `tower-cookies` goes [BE]
- [ ] 2 h · Merge the three near-identical session lookups behind the extractors in `api_caller/extractors.rs` into one helper — they differ only in lookup column, expiry field and token kind [BE]
- [ ] 2 h · Answer 401, not 400, for a missing, malformed, expired or unknown token — one identical body for all [BE]
- [ ] 1 h · Look login input containing `@` up by email and anything else by username — `email = ? OR username = ? LIMIT 1` lets a username that equals another account's email hide that account [BE]
- [x] 0.5 h · `users.email` and `users.username` become `COLLATE NOCASE`, so uniqueness and lookups ignore case [DB]
- [ ] 2 h · Route `sign_in` at `POST /api/v1/auth/login` (replacing the `/login` placeholder) with the body as `Json<…>`: an unknown user and a wrong password get the same 401 in the same time, by verifying against a dummy Argon2 hash made with the default parameters; Argon2 runs in `spawn_blocking`, outside any transaction [BE]
- [ ] 3 h · Throttle failed logins per typed username or email, counted before the password is checked and whether or not the account exists, so a 429 reveals nothing: 10 failures lock that name for 15 minutes, a success clears the count. A `login_failures` table keyed by the lower-cased name; rows older than 15 minutes are pruned on write — any device on the LAN can reach login [DB] [BE]
- [ ] 1 h · Login answers 403 `email_not_verified` for an unverified email, only after a correct password [BE]
- [ ] 2 h · Login answers 200 with `{"status": "ok", session_token, session_expires_at, refresh_token, refresh_expires_at, device_id}` — times as unix seconds (serde writes a `SystemTime` as `{secs_since_epoch, …}`) — and `Cache-Control: no-store` [BE]
- [ ] 3 h · `login_challenges` table for users with confirmed TOTP: a BLAKE3-hashed 5-minute token plus the device fields, no refresh token; login answers `{"status": "totp_required", challenge_token, challenge_expires_at}`. `sessions.authentication_completed` goes [DB] [BE]
- [ ] 2 h · One session per user and device: replace the global `UNIQUE` on `sessions.device_id` with `UNIQUE(user_id, device_id)` and make the column `TEXT`; the server generates device ids (UUID), and a client that sends back its earlier id replaces that device's row on re-login — today a second login from the same device fails [DB] [BE]
- [ ] 0.5 h · `sessions.session_token_hashed` becomes `UNIQUE` — every authenticated request looks it up [DB]
- [ ] 1 h · Write `users.last_login` on a successful login — nothing writes it today [BE]
- [ ] 1 h · Route `log_out` at `POST /api/v1/auth/logout` (replacing the `/log_out` placeholder): it takes the session or the refresh token, ignores expiry and always answers 204 — today an app idle for 15 minutes can't log out; delete the comment-only `log_out` stub in `auth.rs` [BE]
- [ ] 2 h · Route `refresh_session` at `POST /api/v1/auth/refresh` with the refresh token as Bearer; the rotation is conditional on the old refresh-token hash and must change exactly one row, so a replayed or raced refresh token gets 401 [BE]
- [ ] 0.5 h · Log the insert error in `create_new_session` instead of discarding it [BE]
- [ ] 3 h · Login screen: username or email plus password [FE]
- [ ] 1 h · Log-out action [FE]
- [ ] 1 h · Refresh runs one at a time: parallel requests that get 401 wait for that single refresh instead of each starting their own [FE]
