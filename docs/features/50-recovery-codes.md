# Recovery codes

State: in progress

Ten single-use codes, shown once when 2FA is switched on, that stand in for the authenticator
code when the phone is lost. The `recovery_codes` table exists; generating, storing and
redeeming codes is still a comment-only stub.

## Build
- [ ] 2 h · Generate ten codes when 2FA is confirmed ([04](04-two-factor-auth-totp.md)) and the user has none, return them once, and store only their hashes, per the hashing decision — `create_crypto_code` is private to `auth.rs` and needs `pub(crate)` [BE]
- [ ] 2 h · Sign in with a recovery code: route `sign_in_totp_with_recovery_code` (now 501) behind `AuthTOTPLoginSession` — reject a used code, mark the code used, and complete the session like the code step does [BE]
- [ ] 2 h · Reset a locked-out user's 2FA, per the lockout decision [BE]
- [ ] 1 h · Show the recovery codes once, right after 2FA is switched on [FE]
- [ ] 1 h · "Use a recovery code" option on the code-entry step [FE]

## Open decisions
- [ ] Which hash for the codes? `code_hash` is `UNIQUE`, so finding a code by its hash needs a deterministic hash — salted Argon2 can't be looked up — blocks: generating the codes. Suggested: long random codes hashed with BLAKE3, like the session tokens.
- [ ] Phone and codes both lost: an admin reset endpoint, or a manual database edit? — blocks: resetting a locked-out user. Suggested: a manual database edit while you are the only admin; an admin reset once the [admin web view](06-admin-web-view.md) has user management.
