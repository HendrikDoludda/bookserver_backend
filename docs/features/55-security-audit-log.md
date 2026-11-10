# Security audit log

State: not started

Failed logins, password changes and ended sessions are written to the server log as structured
`tracing` events, so they can be found later. Secrets never are.

## Build
- [ ] ? h · Log failed logins, password changes and ended sessions as structured `tracing` events (`tracing::warn!(user_id, event = "login_failed", …)`) — never a password, a token (plain or hashed), a verification or reset code, or the signing key [BE]
- [ ] ? h · Check by hand: each of those events writes exactly one log line, and searching the log for the submitted password, a session token or the signing key finds nothing
