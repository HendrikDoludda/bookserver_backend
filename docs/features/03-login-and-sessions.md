# Login, logout & sessions

State: in progress

A user logs in with username or email plus password and gets a session: a 15-minute session
token and a 30-day refresh token, stored only as BLAKE3 hashes in `sessions`. Logging out
deletes the row. Every per-user feature needs to know who is asking, so this is the core of
Tier 1.

## Build
- [ ] ? h · Install `tower_cookies::CookieManagerLayer` on the router (`api_routes.rs`) — the extractors read cookies, and without the layer every session-guarded request fails with 400 [BE]
- [ ] ? h · Deliver the tokens per the transport decision — today `create_auth_response` only puts them in the JSON body, and nothing sets a cookie [BE]
- [ ] ? h · Route `sign_in` at `POST /api/v1/login` (now `ping_server`), with the body as `Json<…>` [BE]
- [ ] ? h · Route `log_out` at `POST /api/v1/log_out` (now `ping_server`), and delete the comment-only `log_out` stub in `auth.rs` [BE]
- [ ] ? h · Route `refresh_session` — no route exists; [two-factor auth](04-two-factor-auth-totp.md) uses it too [BE]
- [ ] ? h · Make the token rotation in `update_existing_session` conditional on the old refresh-token hash, so a replayed refresh token matches no row and is rejected — this also closes the race the code comment there describes [BE]
- [ ] ? h · Answer 401, not 400, for a missing, expired or unknown session token [BE]
- [ ] ? h · Merge the three near-identical session lookups behind the extractors in `api_caller.rs` into one helper — they differ only in cookie name, lookup column, expiry field and the expected `authentication_completed` [BE]
- [ ] ? h · Write `users.last_login` on a successful login — nothing writes it today [BE]
- [ ] ? h · One session per user and device: replace the global `UNIQUE` on `sessions.device_id` with `UNIQUE(user_id, device_id)`, make the column `TEXT` to match the Rust `String`, and replace the old row on re-login — today a second login from the same device fails [DB] [BE]
- [ ] ? h · Log the insert error in `create_new_session` instead of discarding it [BE]
- [ ] ? h · Login screen: username or email plus password [FE]
- [ ] ? h · Log-out action [FE]

## Open decisions
- [ ] Where do the tokens travel: cookie, `Authorization: Bearer` header, or both? — blocks: delivering the tokens. Suggested: both — `HttpOnly`, `Secure`, `SameSite=Lax` cookies for the browser clients, the header for the app; `create_auth_response` sets the cookies and keeps the JSON.
- [ ] Device ids: take the client's value, or generate them on the server? — blocks: one session per user and device. Suggested: generate them with `uuid` (already a dependency) and treat the client's value as a hint.
- [ ] Must the email be verified before login? `log_in` never checks `email_verified` — blocks: routing `sign_in`. Suggested: yes, once the no-SMTP decision in [27](27-registration-and-email-verification.md) is made — otherwise a server without SMTP locks everyone out.
