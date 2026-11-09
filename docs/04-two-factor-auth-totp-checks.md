# Two-factor auth (TOTP) — checks

On the wiped dev database, with a real authenticator app (Google Authenticator, 1Password, …).

- [ ] `POST /api/v1/account/totp` with your session token; add the account in the app by typing `secret_base32` (or by scanning a QR code made from `otpauth_url`) — the app shows "Bookserver" and your username
- [ ] `POST /api/v1/account/totp/confirm` with the app's current code and your password answers 10 recovery codes, and a session you logged in on a second device answers 401 afterwards
- [ ] `GET /api/v1/account/totp` then answers `{"enabled": true, "recovery_codes_left": 10}`, and logging in again answers `"status": "totp_required"`
