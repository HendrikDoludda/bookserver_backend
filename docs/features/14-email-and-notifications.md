# Email & notifications

State: completed

One sender that every email goes through: today the verification code and the password-reset
code, later the [upload notices](05-uploads-and-ingestion.md). Plain-text bodies; the `SMTP_*`
environment variables configure it, and without them email is off.

## Build
- [x] 1 h · Fix the recipient: `.to(vec![username, email])` turns the username into a second recipient address, so every verification and reset mail gets a bogus extra recipient that the SMTP server may reject — pass the name/address pair `.to((username, email))` [BE]
- [x] 3 h · Load the email config once at startup into an `AppState` router state next to `Arc<Database>`, as `enum Mailer { Smtp(…), Disabled, Capture(…) }` — `Capture` exists only in tests and records the outgoing mail; an enum because an `async fn` in a trait can't be called through `dyn`. `FromRef` impls keep the existing `State<Arc<Database>>` extractors working, and the signing key ([12](12-security-hardening.md)) uses the same state. Log once at startup when email is off [BE]
- [x] 0.5 h · Set a 30 s send timeout — `SmtpClientBuilder` defaults to one hour, so an unresponsive SMTP host holds the request that long [BE]
- [x] 2 h · `SMTP_TLS = starttls (default) | implicit | none` — `implicit_tls(false)` is hard-coded, and `connect()` then refuses a server without STARTTLS, such as a local Mailpit; `none` uses `connect_plain()` and is refused at startup while a password is set. `SMTP_USERNAME` and `SMTP_PASSWORD` become optional [BE]
- [x] 1 h · Send mail nobody waits for from a background task after the commit — the reset code ([30](30-password-reset.md)) first; the sign-up's verification code stays inline, because the user is waiting for it [BE]
- [x] 0.5 h · Mailpit and `SMTP_TLS` steps in `run-deploy/local.md`
