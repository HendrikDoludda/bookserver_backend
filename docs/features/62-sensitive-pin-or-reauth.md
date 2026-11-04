# PIN or re-login to open sensitive items

State: not started

Opening a sensitive item asks for a PIN or the password again. An optional extra on top of the
[sensitive content flag](10-sensitive-content-controls.md).

## Build
- [ ] ? h · Require the PIN or a fresh password check before serving a sensitive item, per the PIN decision — reuse the session machinery; `hash_string_securely` is private to `auth.rs`, so expose it or wrap it [BE]
- [ ] ? h · PIN or password prompt before opening a sensitive item [FE]

## Open decisions
- [ ] A separate PIN, or the account password again? — blocks: both tasks. Suggested: the password — a PIN is a second credential to store, reset and recover.
