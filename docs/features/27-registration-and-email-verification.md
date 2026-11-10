# Registration & email verification

State: in progress

New accounts, by open sign-up. The first account needs the setup code the server logs at start,
becomes admin and is verified on the spot.
Later accounts verify their email with an 8-character code (Argon2-hashed, valid for 15
minutes) when SMTP is configured, and are verified on the spot when it isn't. Redeeming the code
together with the password marks the email verified and logs the user in.

## Build
- [x] 2 h · Route `sign_up` at `POST /api/v1/auth/register`, open to anyone, with `{email, username, password, password_repeat, device_name, platform, device_id?}` — the device fields serve the login response of an account verified on the spot [BE]
- [x] 1 h · Route `verify_email` at `POST /api/v1/auth/verify_email` with `{username_or_email, password, code, device_name, platform, device_id?}`: login's password check and throttle first, then the code [BE]
- [x] 2 h · The first sign-up — into an empty `users` table — becomes admin and is verified on the spot [BE]
- [x] 2 h · The first sign-up must send `setup_code`: a code the server makes at every start, keeps only as a BLAKE3 hash, and logs while no account exists; typed like the emailed codes. Missing or wrong → 403 `invalid_setup_code`, nothing written; later sign-ups ignore the field [BE]
- [x] 2 h · Later sign-ups: with SMTP, mail the code inline and answer 202 `{"verification_required": true}`; without SMTP, create the account verified. A verified sign-up and a redeemed code answer with login's response ([03](03-login-and-sessions.md)), so a TOTP user gets the code step. If the code mail can't be sent, the still-unverified account is deleted again and the answer is 500 [BE]
- [x] 1 h · Password rules in one shared check — `check_new_password` in `auth.rs`: 12 to 128 characters, nothing else; [password reset](30-password-reset.md) uses it [BE]
- [x] 0.5 h · Sign-up runs `check_new_password` [BE]
- [x] 1 h · `check_email` and `check_username` in `auth.rs`, run on sign-up: the email is printable ASCII with exactly one `@` between non-empty parts, none of `( ) < > [ ] : ; \ , "` and at most 254 characters; the username is 3–32 characters from `A–Z a–z 0–9 _ . -` [BE]
- [x] 2 h · Keep verification codes in `email_codes` ([30](30-password-reset.md)), with its limit of 5 tries per code; the `email_verification` table goes [DB] [BE]
- [x] 2 h · `POST /api/v1/auth/verify_email/resend` with `{username_or_email}` mails a new code to an unverified account, within the `email_codes` limits, from a background task: 202 at once, 503 `email_not_configured` when email is off [BE]
- [ ] 1 h · Each sign-up first deletes the accounts still unverified 48 hours after they were created, through the generic `Delete` with `AtMost`, before checking whether the email and username are free — today they stay forever, so anyone can hold an address or a username they don't own [BE]
