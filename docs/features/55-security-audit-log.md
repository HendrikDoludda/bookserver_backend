# Security audit log

State: not started

Failed logins, password changes and ended sessions are written to the server log as structured
`tracing` events, so they can be found later. Secrets never are.

## Build
- [ ] ? h · Log failed logins, password changes and ended sessions as structured `tracing` events (`tracing::warn!(user_id, event = "login_failed", …)`) — never a password, a token (plain or hashed), a verification or reset code, or the signing key [BE]
- [ ] ? h · Check by hand: each of those events writes exactly one log line, and searching the log for the submitted password, a session token or the signing key finds nothing

## Open decisions
- [ ] Write the events with `tracing` macros directly, or first move the `log::error!` call sites to `tracing`? Both already reach the same output — blocks: logging the events. Suggested: `tracing` macros now; moving the old call sites is a consistency change for [33](33-error-responses.md).
