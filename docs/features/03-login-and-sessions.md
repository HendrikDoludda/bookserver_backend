# Login, logout & sessions

State: completed

A user logs in with username or email plus password and gets a session: a 15-minute session
token and a 30-day refresh token, sent as `Authorization: Bearer` and stored only as BLAKE3
hashes in `sessions`. A user with [two-factor auth](04-two-factor-auth-totp.md) gets a 5-minute
login challenge instead, which the code step turns into a session. Logging out deletes the row.
Every per-user feature needs to know who is asking, so this is the core of Tier 1.

## Build
- [x] 2 h · The extractors read `Authorization: Bearer <token>`, parsed by hand (no new crate), instead of cookies; `tower-cookies` goes [BE]
- [x] 2 h · Merge the three near-identical session lookups behind the extractors in `api_caller/extractors.rs` into one helper — they differ only in lookup column, expiry field and token kind [BE]
- [x] 2 h · Answer 401, not 400, for a missing, malformed, expired or unknown token — one identical body for all [BE]
- [x] 1 h · Look login input containing `@` up by email and anything else by username — `email = ? OR username = ? LIMIT 1` lets a username that equals another account's email hide that account [BE]
- [x] 0.5 h · `users.email` and `users.username` become `COLLATE NOCASE`, so uniqueness and lookups ignore case [DB]
- [x] 2 h · Route `sign_in` at `POST /api/v1/auth/login` with the body as `JsonBody<…>`: an unknown user and a wrong password get the same 401 in the same time, by verifying against a dummy Argon2 hash made with the default parameters; Argon2 runs in `spawn_blocking`, outside any transaction [BE]
- [x] 3 h · Throttle failed logins per typed username or email, counted before the password is checked and whether or not the account exists, so a 429 reveals nothing: 10 failures lock that name for 15 minutes, a success clears the count. A `login_failures` table keyed by the lower-cased name; rows older than 15 minutes are pruned on write — any device on the LAN can reach login [DB] [BE]
- [x] 1 h · Login answers 403 `email_not_verified` for an unverified email, only after a correct password [BE]
- [x] 2 h · Login answers 200 with `{"status": "ok", session_token, session_expires_at, refresh_token, refresh_expires_at, device_id}` — times as unix seconds — and `Cache-Control: no-store` [BE]
- [x] 3 h · `login_challenges` table for users with confirmed TOTP: a BLAKE3-hashed 5-minute token plus the device fields, no refresh token; login answers `{"status": "totp_required", challenge_token, challenge_expires_at}`. `sessions.authentication_completed` goes [DB] [BE]
- [x] 2 h · One session per user and device: `UNIQUE(user_id, device_id)` on a `TEXT` column; the server generates device ids (UUID), and a client that sends back its earlier id replaces that device's row on re-login [DB] [BE]
- [x] 0.5 h · `sessions.session_token_hashed` becomes `UNIQUE` — every authenticated request looks it up [DB]
- [x] 1 h · Write `users.last_login` on a successful login [BE]
- [x] 1 h · Route `log_out` at `POST /api/v1/auth/logout`: it takes the session or the refresh token, ignores expiry and always answers 204, so an app idle for 15 minutes can still log out [BE]
- [x] 2 h · Route `refresh_session` at `POST /api/v1/auth/refresh` with the refresh token as Bearer; the rotation is conditional on the old refresh-token hash and must change exactly one row, so a replayed or raced refresh token gets 401 [BE]
- [x] 0.5 h · Log the insert error in `create_new_session` instead of discarding it [BE]
- [x] 2 h · Each login also deletes the user's sessions whose refresh token has expired, through a new `AtMost` comparison in the generic `Delete` — otherwise only a new login on the same device or a logout removes a session, so abandoned devices would stay forever and [active sessions](51-active-sessions.md) would list them [BE]
