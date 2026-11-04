# Account management

State: in progress

A logged-in user changes their password, username or email. The handlers exist but are not
routed, and each needs a fix first.

## Build
- [ ] ? h · Route `change_password`, with the body as `Json<…>` [BE]
- [ ] ? h · `change_password` answers 400, not 500, for a wrong current password or a mismatched repeat [BE]
- [ ] ? h · Reject a new password that equals the old one — the plan in the comment-only `verify_new_password` stub in `auth.rs`, which then goes [BE]
- [ ] ? h · After a password change, end all of the user's other sessions — the plan in the comment-only `password_changed` stub, which becomes the real function; [password reset](30-password-reset.md) reuses it [BE]
- [ ] ? h · Route `change_username` (body as `Json<…>`) and check the new name is free first — today a taken name hits the `UNIQUE` constraint and comes back as 500 [BE]
- [ ] ? h · Route `change_email` (body as `Json<…>`), verifying per the change-email decision — today the new address is written directly and keeps the old `email_verified` flag [BE]

## Open decisions
- [ ] Changing the email: verify the new address before switching to it, or switch and mark it unverified? — blocks: routing `change_email`. Suggested: verify first — otherwise anyone holding a session can redirect password-reset mail.
