# Recovery codes

State: in progress

Ten single-use codes, shown once when 2FA is switched on, that stand in for the authenticator
code when the phone is lost. Generating and storing them works; redeeming one is still a
comment-only stub.

## Build
- [x] 2 h · Generate ten codes when 2FA is confirmed ([04](04-two-factor-auth-totp.md)), replacing any earlier ones: 16 base32 characters each (Crockford's alphabet, without `I L O U`; `XXXX-XXXX-XXXX-XXXX`, 80 bits), returned once and stored only as BLAKE3 hashes — `code_hash` is `UNIQUE` and looked up, so the hash has to be deterministic, and 80 random bits make an unsalted one safe [BE]
- [ ] 2 h · Sign in with a recovery code: route `sign_in_totp_with_recovery_code` (now 501) at `POST /api/v1/auth/login/recovery_code` behind `AuthTOTPLoginSession` — normalise the input (upper case, no dashes or spaces, `O` read as `0` and `I`/`L` as `1`), mark the unused code with that hash used — re-checked in the final transaction, exactly one row — share the wrong-code limit of [04](04-two-factor-auth-totp.md), and complete the login like the code step does [BE]
- [ ] 1 h · Fresh codes while 2FA stays on: `POST /api/v1/account/totp/recovery_codes` behind `AuthSession` with `{password, code}` — checked like disabling 2FA ([04](04-two-factor-auth-totp.md)) — answers ten new codes and drops the old ones; today only turning 2FA off and on again gives new codes [BE]
- [ ] 0.5 h · Add the manual 2FA reset for a user who lost phone and codes — deleting their `totp` and `recovery_codes` rows with `sqlite3` — to `commands.md`, flagged destructive, until the [admin web view](06-admin-web-view.md) has user management
- [ ] 1 h · Show the recovery codes once, right after 2FA is switched on [FE]
- [ ] 1 h · "Use a recovery code" option on the code-entry step [FE]
