# Password reset

State: in progress

A user who forgot their password asks for a code by email and types it into the app together
with a new password. The answer to a request never reveals whether the address has an account,
and a reset never switches two-factor auth off.

## Build
- [ ] 3 h · `email_codes` table, one row per user and purpose (`reset_password`, `verify_email` — [27](27-registration-and-email-verification.md)), replacing `reset_password_requests`: the Argon2 hash of an 8-character code from `ABCDEFGHJKMNPQRSTUVWXYZ23456789` (compared case-insensitively), an expiry 15 minutes out, and a try counter — 5 per code, counted before the code is checked. Per account at least 60 s between mails and at most 5 mails in 24 hours [DB] [BE]
- [ ] 3 h · Request: `POST /api/v1/auth/password_reset` with `{email}` answers 503 `email_not_configured` when email is off, otherwise 202 at once. A background task looks the address up (the email only — today's lookup also matches usernames), applies the limits, hashes and stores the code, and after the commit mails the stored address, addressed with the account's username — today the mail goes to whatever string was sent, addressed to `"Anonymous"`. A failed send is logged without its body [BE]
- [ ] 3 h · Redeem: `POST /api/v1/auth/password_reset/confirm` with `{email, code, new_password, new_password_repeat}` — the password check of [27](27-registration-and-email-verification.md) first; an unknown address, a missing, expired or used-up code and a wrong code all get one 400 `invalid_or_expired_code` in the same time (a dummy Argon2 verify where there is no code). On success store the new hash, delete the code (exactly one row), end all sessions (the function from [29](29-account-management.md)) and answer 200 without tokens [BE]
- [ ] 0.5 h · Remove the cancel link: `cancelation_token`, the `Url` parameter of `send_reset_password_email`, the `https://invalid.link` placeholder and the `url` dependency [BE]
- [ ] 0.5 h · Delete the comment-only `reset_password` stub in `auth.rs`, and `store_user_password_reset_token`, which the request task replaces [BE]
- [ ] 1 h · Reset email text from the [translation catalog](13-internationalization.md) instead of hard-coded English [BE]
- [ ] 2 h · "Forgot password" form [FE]
- [ ] 2 h · Code and new-password screen [FE]
