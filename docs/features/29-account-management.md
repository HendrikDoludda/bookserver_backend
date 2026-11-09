# Account management

State: in progress

A logged-in user changes their password, username or email. The handlers exist but are not
routed, and each needs a fix first.

## Build
- [ ] 1 h · Route `change_password`, with the body as `Json<…>` [BE]
- [ ] 1 h · `change_password` answers 400 for a wrong current password — it reuses login's `IncorrectCredentials` (401), which the app reads as an expired session. `check_current_password` (`login.rs`) does it: login's throttle, and 400 `incorrect_password` [BE]
- [ ] 1 h · Reject a new password that equals the old one — the plan in the comment-only `verify_new_password` stub in `auth.rs`, which then goes [BE]
- [ ] 0.5 h · After a password change, end all of the user's other sessions with `end_other_sessions` (`auth.rs`), which keeps the current one [BE]
- [ ] 0.5 h · `change_password` runs `check_new_password` from `auth.rs` [BE]
- [ ] 2 h · Route `change_username` (body as `Json<…>`); the new name passes `check_username` and is checked to be free first — today a taken name hits the `UNIQUE` constraint and comes back as 500 [BE]
- [ ] 3 h · Route `change_email` (body as `Json<…>`); the new address passes `check_email` and is verified per the change-email decision — today it is written directly and keeps the old `email_verified` flag [BE]

## Open decisions
- [ ] Changing the email: verify the new address before switching to it, or switch and mark it unverified? — blocks: routing `change_email`. Suggested: verify first — otherwise anyone holding a session can redirect password-reset mail.
