# Account management

State: completed

A logged-in user changes their password, username or email.

## Build
- [x] 1 h · Route `change_password` at `PUT /api/v1/account/password`, with the body as `JsonBody<…>`, answering 204 [BE]
- [x] 1 h · `change_password` answers 400 for a wrong current password — not login's `IncorrectCredentials` (401), which the app reads as an expired session. `check_current_password` (`login.rs`) does it: login's throttle, and 400 `incorrect_password` [BE]
- [x] 1 h · Reject a new password that equals the old one with 400 `password_unchanged` [BE]
- [x] 0.5 h · After a password change, end all of the user's other sessions with `end_other_sessions` (`auth.rs`), which keeps the current one [BE]
- [x] 0.5 h · `change_password` runs `check_new_password` from `auth.rs` [BE]
- [x] 2 h · Route `change_username` at `PUT /api/v1/account/username` (body as `JsonBody<…>`); the new name passes `check_username` and is checked to be free in the same transaction as the write — otherwise a taken name would hit the `UNIQUE` constraint and come back as 500 [BE]
- [x] 3 h · Request an email change at `PUT /api/v1/account/email` with `{new_email, password}`: `check_email`, `check_current_password`, 409 for a taken address; the code is stored in `email_codes` (purpose `change_email`) with the pending address in a new `new_email` column, and mailed to that address from a background task — 202. Without email configured the change applies at once [DB] [BE]
- [x] 2 h · Confirm an email change at `POST /api/v1/account/email/confirm` with `{code}`: the try is counted first; the final transaction re-reads the code — a parallel confirm finds it gone — and re-checks that the address is still free, then writes the address as verified and deletes the code [BE]
- [x] 0.5 h · The confirmed email change also deletes the account's other email codes, in the same transaction — they went to the old address, and a reset code is stored per account, so it would stay redeemable through the new address until it expires [BE]
