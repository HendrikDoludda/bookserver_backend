# Registration & email verification

State: in progress

New accounts. Sign-up stores an Argon2 password hash and mails an 8-character code (stored
Argon2-hashed, valid for 15 minutes); redeeming it marks the email verified and logs the user
in. Whether anyone may sign up, or only admins create users, is still open.

## Build
- [ ] ? h · Route `sign_up` at `POST /api/v1/register_user` (now `ping_server`), open or admin-only per the registration decision [BE]
- [ ] ? h · Route `verify_email` — no route exists; take the body as `Json<…>`, as the last argument (today it is a bare struct before `State`) [BE]
- [ ] ? h · Create the first admin per the first-admin decision — today `validate_new_user` makes the first sign-up an admin, but the flag never reaches the database ([26](26-database-layer-fixes.md)) [BE]
- [ ] ? h · Without an SMTP config, behave per the no-SMTP decision — today `sign_up` silently auto-verifies the account and hands out real tokens, and every such sign-up after the first fails silently on the shared `device_id` `"Unknown"` [BE]
- [ ] ? h · Password rules (at least a minimum length) in one shared check, used here and by [password change](29-account-management.md) and [password reset](30-password-reset.md) — none exist today [BE]
- [ ] ? h · Validate the email format and the allowed username characters on sign-up [BE]
- [ ] ? h · Limit wrong verification-code attempts [BE]
- [ ] ? h · Verification email text from the [translation catalog](13-internationalization.md) instead of hard-coded English [BE]
- [ ] ? h · Sign-up screen, or admin create-user screen, per the registration decision [FE]
- [ ] ? h · Verification-code screen [FE]

## Open decisions
- [ ] Open sign-up, or only admins create users? The old roadmap decided admin-only; the code has open sign-up — blocks: routing `sign_up`. Suggested: a `REGISTRATION_OPEN` setting, closed by default.
- [ ] How is the first admin created: from env on first start, or the first sign-up? — blocks: creating the first admin. Suggested: from env — "first sign-up wins" races with anyone else who can reach the server.
- [ ] Without an SMTP config, auto-verify new accounts or refuse sign-up? — blocks: the no-SMTP behaviour. Suggested: refuse with a clear "email is not configured" error, and allow auto-verify only behind an explicit dev flag such as `ALLOW_AUTO_VERIFY=1`.
- [ ] Which language does the verification email use — the server default, the `Accept-Language` header, or a locale sent with the sign-up? — blocks: verification email text from the catalog. Suggested: a locale field in the sign-up request, stored right away in the [per-user settings](31-per-user-settings.md), so even the first email is localized.
