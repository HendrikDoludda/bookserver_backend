# Email & notifications

State: in progress

One SMTP sender, `create_new_email`, that every email goes through: today the verification
code and the password-reset link, later the [upload notices](05-uploads-and-ingestion.md).
Plain-text bodies; the `SMTP_*` environment variables configure it, and if any one is missing
there is no email at all.

## Build
- [ ] ? h · Fix the recipient: `.to(vec![username, email])` turns the username into a second recipient address, so every verification and reset mail gets a bogus extra recipient that the SMTP server may reject — pass the name/address pair `.to((username, email))` [BE]
- [ ] ? h · Load the SMTP config once at startup into an `AppState` router state next to `Arc<Database>` — `get_email_config` re-reads the environment on every send today. `FromRef` impls keep the existing `State<Arc<Database>>` extractors working, and the signing key ([12](12-security-hardening.md)) and an encrypted TOTP secret ([04](04-two-factor-auth-totp.md)) use the same state. Log once at startup when email is not configured [BE]
- [ ] ? h · Set a send timeout — `SmtpClientBuilder` defaults to one hour, so an unresponsive SMTP host holds the request that long [BE]
- [ ] ? h · Configurable TLS mode, per the TLS decision — `implicit_tls(false)` (STARTTLS, port 587) is hard-coded [BE]
- [ ] ? h · Send emails nobody waits for in the background, per the inline-or-background decision — `tokio::spawn` after the commit; the task owns its data and can only log a failure [BE]
- [ ] ? h · Write each email in the recipient's language ([31](31-per-user-settings.md)), falling back to the default [BE]
- [ ] ? h · Check by hand: a verification email arrives with exactly one recipient and `To: Username <user@example.com>`, against a local SMTP catcher

## Open decisions
- [ ] Send inline, or in the background? — blocks: background sending. Suggested: keep the verification email inline — the user is waiting for the code — and send upload notices in the background.
- [ ] TLS mode: derived from the port (465 → implicit TLS) with an env override, or its own `SMTP_IMPLICIT_TLS` setting? — blocks: configurable TLS mode. Suggested: derive it from the port, with an env override.
