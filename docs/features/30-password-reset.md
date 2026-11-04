# Password reset

State: in progress

A user who forgot their password asks for a reset link by email, sets a new password through
it, and can cancel a reset they did not ask for. The answer to a request never reveals
whether the address has an account.

## Build
- [ ] ? h · Fix the row write in `store_user_password_reset_token`: in `.ok_or(return Err(…))` the `return` runs before `ok_or` does, so the old row is deleted and no new one is ever written (the compiler warns "unreachable call"). The fix exposes a move out of `&tokens` a few lines below — fix both in one change [BE]
- [ ] ? h · Answer the same for known and unknown addresses — today an unknown address fails the lookup with 500, so once the row write works, 202 versus 500 tells anyone which addresses have accounts [BE]
- [ ] ? h · Stop dropping a failed reset-mail send: `.map_err(|err| return …)` returns from the closure, not from the handler, and the result is discarded — log the failure and keep the answer unchanged [BE]
- [ ] ? h · Build reset links from a configured public base URL — today `https://invalid.link` is hard-coded [BE]
- [ ] ? h · In `send_reset_password_email`, `join("/password-reset/cancel")` drops any base path of the server URL because of the leading `/`, and a join failure is reported as `EmailErrors::MissingEmail` [BE]
- [ ] ? h · Redeem: implement `reset_password` (today 501) — check the token and its expiry, take the new password twice, store the new hash, delete the reset row and end all sessions (the function from [29](29-account-management.md)) [BE]
- [ ] ? h · Cancel: `GET /password-reset/cancel?token=…` invalidates a reset the user did not ask for [BE]
- [ ] ? h · Routes for request, redeem and cancel — `request_reset_password_link` and `reset_password` take a bare struct before `State`; take the body as `Json<…>`, last [BE]
- [ ] ? h · Reset email text from the [translation catalog](13-internationalization.md) instead of hard-coded English [BE]
- [ ] ? h · `send_reset_password_email` sets the username to `"Anonymous"` — once the recipient fix in [14](14-email-and-notifications.md) makes it the display name, pass the account's username [BE]
- [ ] ? h · Delete the comment-only `reset_password` stub in `auth.rs` [BE]
- [ ] ? h · "Forgot password" form [FE]
- [ ] ? h · New-password page that the reset link opens [FE]
- [ ] ? h · Cancel confirmation page [FE]

## Open decisions
- [ ] How does the server find the reset row from a link? Both tokens are Argon2-hashed with a random salt and the link carries only the token, so no lookup is possible — blocks: redeem, cancel. Suggested: long random tokens hashed with BLAKE3 like the session tokens — without a salt, the row is found by its hash.
