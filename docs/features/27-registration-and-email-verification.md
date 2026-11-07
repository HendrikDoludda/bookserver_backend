# Registration & email verification

State: in progress

New accounts, by open sign-up. The first account becomes admin and is verified on the spot.
Later accounts verify their email with an 8-character code (Argon2-hashed, valid for 15
minutes) when SMTP is configured, and are verified on the spot when it isn't. Redeeming the code
marks the email verified and logs the user in.

## Build
- [ ] 2 h · Route `sign_up` at `POST /api/v1/auth/register` (replacing the `/register_user` placeholder), open to anyone [BE]
- [ ] 1 h · Route `verify_email` at `POST /api/v1/auth/verify_email`; take the body as `Json<…>`, as the last argument (today it is a bare struct before `State`) [BE]
- [ ] 2 h · The first sign-up becomes admin and is verified on the spot — today `validate_new_user` decides it, but the flag never reaches the database ([26](26-database-layer-fixes.md)) [BE]
- [ ] 2 h · Later sign-ups: with SMTP, mail the code inline and answer 202 `{"verification_required": true}`; without SMTP, create the account verified. A verified account gets the login response of [03](03-login-and-sessions.md). `auto_verify_account` goes, with its fake code round trip and its shared `device_id` `"Unknown"`, on which every such sign-up after the first fails [BE]
- [x] 1 h · Password rules in one shared check — `check_new_password` in `auth.rs`: 12 to 128 characters, nothing else; [password reset](30-password-reset.md) uses it [BE]
- [ ] 0.5 h · Sign-up runs `check_new_password` [BE]
- [ ] 1 h · Validate the email (one `@`, no spaces, at most 254 characters) and the username (3–32 characters from `A–Z a–z 0–9 _ . -` — never `@`, which the login lookup reserves for emails, nor `:`, which `totp-rs` rejects) on sign-up [BE]
- [ ] 2 h · Keep verification codes in `email_codes` ([30](30-password-reset.md)), with its limit of 5 tries per code; the `email_verification` table goes [DB] [BE]
- [ ] 2 h · `POST /api/v1/auth/verify_email/resend` with `{email}` mails a new code to an unverified account, within the `email_codes` limits, and always answers 202 — codes expire after 15 minutes and nothing can send a new one today, so a user who misses the window is stuck [BE]
- [ ] 2 h · Verification email text from the [translation catalog](13-internationalization.md) instead of hard-coded English [BE]
- [ ] 3 h · Sign-up screen [FE]
- [ ] 2 h · Verification-code screen, with "send a new code" [FE]

## Open decisions
- [ ] Which language does the verification email use — the server default, the `Accept-Language` header, or a locale sent with the sign-up? — blocks: verification email text from the catalog. Suggested: a locale field in the sign-up request, stored right away in the [per-user settings](31-per-user-settings.md), so even the first email is localized.
