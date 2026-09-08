# Email & Notifications

> The backend's outbound SMTP path: the one email sender, the account-verification and password-reset messages it already carries, and the upload notifications the uploads feature needs.

**Status:** 🟡 partial · **Phase:** 1 · **Tier:** Cross-cutting, unscheduled — already in use by Backend Tier 1, extended again in Tier 4 · **Tags:** [BE]
**Depends on:** [13 Internationalization (i18n)](13-internationalization.md) (only for localized bodies; the catalog makes the emails translate for free)
**Blocks:** [03 Authentication, Users & Sessions](03-auth-users-and-sessions.md) (sign-up verification and password reset both send mail) · [05 Uploads & File Ingestion](05-uploads-and-ingestion.md) (the rename and out-of-space policies both notify by email)
**Roadmap:** [Build order](../../ROADMAP.md#priority-order-build-sequence)

## Why
The sign-up flow already sends a verification code, so SMTP is live code today even though the roadmap never gave it a section. The uploads feature then makes email load-bearing a second time: the decided collision policy (auto-rename) and insufficient-storage policy (drop the upload) both notify the user by email, so a working sender is a prerequisite for those. Because i18n decided on a backend-owned catalog, the emails localize for free — the backend already has both the catalog and the user's locale, so there is no separate email catalog to build.

## Current state
Live, and reachable from a real handler:
- [email_helper.rs:7](../../src/routes/email_helper.rs#L7) — `create_new_email(EmailInformation) -> Result<(), EmailErrors>`, the single send function. Builds a plaintext message with `mail_send::mail_builder::MessageBuilder` ([email_helper.rs:13-17](../../src/routes/email_helper.rs#L13-L17)), then `SmtpClientBuilder::new(host, port)` → `.implicit_tls(false)` → `.credentials(...)` → `.connect()` → `.send()` ([email_helper.rs:19-37](../../src/routes/email_helper.rs#L19-L37)), each step mapped to `ClientBuilderFailed` / `SMTPConnectionFailed` / `SendingFailure` with a `log::error!`.
- [email_helper.rs:43](../../src/routes/email_helper.rs#L43) — `EmailInformation { username, email, body, subject }`, the only input type.
- Crate: `mail-send = "0.6.2"` ([Cargo.toml:30](../../Cargo.toml#L30)), which pulls `mail-builder` 0.5.0.
- Config: `config::get_email_config()` ([config.rs:80](../../src/config.rs#L80)) reads `SMTP_HOST`, `SMTP_PORT`, `SMTP_USERNAME`, `SMTP_PASSWORD`, `SMTP_FROM`, `SMTP_FROM_NAME` from the environment into `EmailConfig` ([models.rs:526](../../src/data_models/models.rs#L526)). All-or-nothing: any single missing or unparsable var yields `None`. It is called **inside** `create_new_email` ([email_helper.rs:12](../../src/routes/email_helper.rs#L12)), so the environment is re-read on every send; the file's own comment already plans to fix this ([email_helper.rs:6](../../src/routes/email_helper.rs#L6): *"Eventually I need to change it to use impl with a struct this way we can load the config once on opening the back end."*).
- Errors: `EmailErrors` ([error_types.rs:275](../../src/error_types.rs#L275)), 12 variants including the struct variant `SentFailedDueToNoConfig { code: String }` ([error_types.rs:291](../../src/error_types.rs#L291)). It has no `IntoResponse`; it reaches clients only through `AppErrors` and hand-built 500 bodies.

Email #1 — account verification (live, wired):
- [auth.rs:128](../../src/routes/auth.rs#L128) `send_verification_email` issues an 8-character code, Argon2-hashes it into `email_verification`, and returns the plaintext code plus the user row.
- [auth.rs:172](../../src/routes/auth.rs#L172) `create_verification_email` builds subject `Verify Book Server Account` and a plaintext body ([auth.rs:176](../../src/routes/auth.rs#L176)) and calls `create_new_email`. `UserMetadata.email` is `Option<String>` ([models.rs:261](../../src/data_models/models.rs#L261)) even though `users.email` is `TEXT NOT NULL`, so this step can still bail out with `MissingEmail` ([auth.rs:185](../../src/routes/auth.rs#L185)).
- Called from `sign_up` at [api_caller.rs:366](../../src/routes/api_caller.rs#L366) — inline in the request path, after the transaction commits.
- If the SMTP config is absent, `EmailSetUpNotFound` is remapped to `SentFailedDueToNoConfig { code }` ([api_caller.rs:369-371](../../src/routes/api_caller.rs#L369-L371)) and `sign_up` falls through to `auto_verify_account` ([api_caller.rs:381-383](../../src/routes/api_caller.rs#L381-L383), [api_caller.rs:393](../../src/routes/api_caller.rs#L393)), which self-verifies the account and hands out real tokens with the device fields hard-coded to `"Unknown"`. Owned by [auth](03-auth-users-and-sessions.md); noted here because the no-config path is an email-layer consequence.

Email #2 — password reset with cancelation link (code exists, **no caller**):
- [auth.rs:353](../../src/routes/auth.rs#L353) `send_reset_password_email` builds subject `Password Reset`, a body containing the reset code plus a cancelation link `"{server_address}/password-reset/cancel?token={cancelation_token}"` ([auth.rs:360-363](../../src/routes/auth.rs#L360-L363)), and passes `username: "Anonymous"` ([auth.rs:367](../../src/routes/auth.rs#L367)).
- Its only caller is `request_reset_password` ([auth.rs:321](../../src/routes/auth.rs#L321)), which nothing calls — the handler `request_reset_password_link` ([api_caller.rs:597](../../src/routes/api_caller.rs#L597)) looks the user up and then has a comment where the email should be ([api_caller.rs:603](../../src/routes/api_caller.rs#L603): *"create something similar or the same as the verification email"*), returning 202 "Email has been sent" without sending anything.
- The server base URL it needs has no config source; the comment at [auth.rs:359](../../src/routes/auth.rs#L359) says *"must look into getting the server address even if it is just storing the value somewhere"*. `config.rs` has no such variable.
- A URL-join failure is mapped to the unrelated `EmailErrors::MissingEmail` ([auth.rs:362](../../src/routes/auth.rs#L362)).

Empty comment-only stub:
- [email_helper.rs:41](../../src/routes/email_helper.rs#L41) — `fn send_email() {}`. Private, empty, no callers. `create_new_email` is the only real entry point.

Not started at all:
- Both upload notifications (rename, insufficient storage) — no code, no caller, nothing in `email_helper.rs` for them.
- Localization. Both bodies and both subjects are hardcoded English `format!` strings, with a standing TODO to move them to the i18n catalog ([auth.rs:171](../../src/routes/auth.rs#L171): *"Text must be replaced with the i18n text which I will create after the authentication"*).

**Roadmap drift:** [ROADMAP.md](../../ROADMAP.md) has no section for this area at all, yet `email_helper.rs` is implemented and the sign-up flow already depends on it — the only mentions are §4's upload notifications (⬜, correct) and §12's "emails localize for free" note. §3a's rationale for choosing TOTP ("no delivery service to run — no SMTP, no Twilio, no cost") is still true of TOTP itself, but it no longer describes the project: the implemented sign-up flow needs SMTP, so the "no SMTP" property was already given up before uploads reintroduced it.

## Decisions
- **SMTP config from environment variables, all-or-nothing.** `get_email_config()` returns `Option<EmailConfig>` and yields `None` if any of the six `SMTP_*` vars is missing or unparsable ([config.rs:80](../../src/config.rs#L80)). A partially configured mailer is treated as no mailer.
- **One sender function, plaintext bodies.** Every email goes through `create_new_email` with a subject and a text body; callers build the text. No HTML alternative.
- **Crate choice: `mail-send`.** The roadmap's §4 note said "SMTP, e.g. Rust `lettre`"; the code committed to `mail-send = "0.6.2"` instead ([Cargo.toml:30](../../Cargo.toml#L30)), and both existing emails are built on it. Treat `mail-send` as the decision — `lettre` was only an example.
- **TOTP deliberately needs no email (§3a, DECIDED).** TOTP (authenticator app) was chosen over email/SMS second factors precisely because there is no delivery service to run (no SMTP, no Twilio, no cost), it works offline, and it is more secure than SMS. The shared secret plus the current time produce the same 6-digit code on phone and server independently. So 2FA must not grow an email dependency.
- **Upload collision policy (§4, DECIDED): auto-rename + notify by email.** When a resolved upload name already exists on disk, auto-rename rather than overwrite or fail, and **notify the uploader by email that the file was renamed**, sent to the user's `users.email`. This is what reintroduces the email dependency — not needed for TOTP auth, but needed here.
- **Rename scheme (§4, DECIDED): timestamp suffix.** The renamed file must still parse cleanly back through the scanner's filename regex (the round-trip constraint), so the notification names a file the scanner can still read.
- **Insufficient-storage policy (§4, DECIDED): pre-flight free-space check, drop the upload, notify by email.** Check *before* writing so nothing half-lands; do not write a partial file. In a bulk batch, drop the offending file and report it — the rest of the batch is unaffected because ingest is sequential.
- **Email bodies come from the backend i18n catalog (§12, DECIDED).** Because the catalog is backend-owned data and the backend also knows the user's locale, the §4 notification emails localize for free — no separate email catalog is needed. Today's bodies are hardcoded English placeholders pending that catalog.

## Open questions
- **Is `.to(vec![username, email])` correct for `mail-send`? No — it is a bug.** In `mail-builder` 0.5.0, `MessageBuilder::to` takes `impl Into<Address>` (`mail-builder-0.5.0/src/lib.rs:84`); `From<Vec<T>>` builds an `Address::List` in which each `String` becomes `EmailAddress { name: None, email }` (`mail-builder-0.5.0/src/headers/address.rs:93` and `:102`). `mail-send` then derives the SMTP envelope from the `To`/`Cc`/`Bcc` headers and issues one `RCPT TO` per address in that list (`mail-send-0.6.2/src/smtp/message.rs:49-64` and `:361-370`). So [email_helper.rs:15](../../src/routes/email_helper.rs#L15) sends the *username as a second recipient address*, not as a display name — a server that rejects `RCPT TO:<username>` fails the whole send as `SendingFailure`, and a lenient one delivers a message with a bogus second recipient. The name-plus-address form is the `(String, String)` tuple (`address.rs:75`). Suggested: change it to `.to((email_information.username, email_information.email))` — a one-line fix, and add a smoke test against a local catcher before uploads depend on it.
- **Should `implicit_tls` be configurable?** [email_helper.rs:24](../../src/routes/email_helper.rs#L24) hardcodes `implicit_tls(false)`, i.e. STARTTLS on a submission port like 587. Options: leave it (works for 587 relays, no config surface) vs. add an `SMTP_IMPLICIT_TLS` var vs. derive it from the port (465 → true). Suggested: derive from the port with an explicit env override, since a home server may well point at a 465-only relay.
- **Credentials are always sent, with no way to omit them.** An unauthenticated LAN relay cannot be used because `.credentials(...)` is unconditional and the config is all-or-nothing. Suggested: keep it as-is for now and revisit only if a local relay is actually wanted — it is not blocking.
- **Delete or implement `send_email()`?** [email_helper.rs:41](../../src/routes/email_helper.rs#L41) is an empty private fn with no callers. Options: delete it, or make it the struct-based sender the file's comment plans. Suggested: delete it and let the "load config once" work introduce the struct deliberately, rather than keeping a hollow name around.
- **Send inline or off the request path?** Today `sign_up` awaits the send inside the handler ([api_caller.rs:366](../../src/routes/api_caller.rs#L366)), so an unreachable SMTP host stalls the HTTP response. Options: keep it inline (the caller can report failure, at the cost of latency) vs. `tokio::spawn` after commit (fast response, but the failure can only be logged). Suggested: keep verification inline for now — the user is waiting for a code — and spawn the upload notifications, which nobody is waiting on.
- **Where does the server base URL for the reset link come from?** Nothing supplies the `&Url` that `send_reset_password_email` needs, and `config.rs` has no such variable ([auth.rs:359](../../src/routes/auth.rs#L359)). Options: a `SERVER_BASE_URL` env var vs. a value stored in the DB and editable from the admin UI (which fits the guiding principle better). Suggested: env var now, admin-editable later alongside the other admin-managed settings.
- **Does the no-SMTP auto-verify fallback stay?** With no SMTP configured, sign-up currently self-verifies the account and issues tokens ([api_caller.rs:381-383](../../src/routes/api_caller.rs#L381-L383)). Convenient during development, but it means "email is misconfigured" silently becomes "email verification is off". Suggested: keep it only behind an explicit dev flag, and decide it in [auth](03-auth-users-and-sessions.md), which owns the flow.
- **Email error text currently reaches the client.** `sign_up` interpolates `err.to_string()` into a 500 body ([api_caller.rs:385-388](../../src/routes/api_caller.rs#L385-L388)), which §11's "generic error bodies to the client" hardening item forbids. Suggested: log the detail, send a generic body — tracked in [cross-cutting polish](07-cross-cutting-backend-polish.md) / [operations & security](12-operations-and-security.md), listed here because email failures are one of the leaks.

## Tasks

### Sender hardening (do before uploads depend on it)
- 🟡 One SMTP send function with subject + plaintext body **[BE]** — `create_new_email` exists and works apart from the defects below
- ⬜ Fix the recipient construction — pass `(username, email)` as a name/address pair instead of `vec![username, email]`, which sends the username as a second `RCPT TO` **[BE]**
- ⬜ Load the SMTP config **once at startup** instead of re-reading the environment on every send (comment already in the file) **[BE]**
- ⬜ Make the TLS mode configurable (implicit TLS / 465 vs STARTTLS / 587) instead of hardcoded `implicit_tls(false)` **[BE]**
- ⬜ Remove or implement the empty `send_email()` stub **[BE]**
- ⬜ Map URL/build failures to their own error variants rather than reusing `MissingEmail` **[BE]**
- ⬜ Stop returning email error text in HTTP response bodies (log detail, generic body — §11) **[BE]**

### The two existing emails
- ✅ Account-verification email — code issued, hashed, stored, body + subject built, sent from `sign_up` **[BE]**
- 🟡 Password-reset email with reset code + cancelation link — body/subject and link building exist but nothing calls them **[BE]**
- ⬜ Wire the reset email into `request_reset_password_link` so the 202 "Email has been sent" is true **[BE]** *(the token storage/expiry side is owned by [auth](03-auth-users-and-sessions.md))*
- ⬜ Supply the server base URL the cancelation link needs (no config source today) **[BE]**
- ⬜ Use the real recipient name in the reset email instead of the hardcoded `"Anonymous"` **[BE]**

### Upload notifications (from §4 uploads)
- ⬜ Rename notification — when an upload's resolved name collides and the file is auto-renamed (timestamp suffix), email the uploader at `users.email` that the file was renamed **[BE]**
- ⬜ Insufficient-storage notification — when the pre-flight free-space check fails, the upload is dropped without writing a partial file and the user is emailed **[BE]**
- ⬜ In a bulk batch, notify per dropped/renamed file and leave the rest of the sequential batch unaffected **[BE]**

### Localization
- ⬜ Move both existing email bodies + subjects out of hardcoded English `format!` strings into the i18n catalog (TODO already in `auth.rs`) **[BE]**
- ⬜ Render each email in the recipient's locale, falling back to the default language **[BE]** *(needs per-user language preference from §3 per-user settings / §12)*
- ⬜ Add the upload-notification strings to the catalog as the notifications are built **[BE]**

## Done when
- [ ] A verification email arrives with exactly one recipient and a `To:` header of the form `Username <user@example.com>` — no second bogus recipient, verified against a local SMTP catcher.
- [ ] `get_email_config()` is called exactly once during startup; grepping the send path finds no environment read per email.
- [ ] `POST` of a password-reset request actually delivers an email containing both the reset code and a working cancelation link built from a configured base URL.
- [ ] An upload whose target name already exists lands under a timestamp-suffixed name that the scanner's filename regex still parses, and the uploader receives one email naming the old and new name.
- [ ] An upload larger than the free space on the target drive leaves no file on disk, the rest of a bulk batch still ingests, and the uploader receives one email about the dropped file.
- [ ] A send failure (SMTP host down) produces a logged error and a generic client-facing message — no SMTP detail or internal error text in the response body.

## Implementation notes
- **Crates:** `mail-send = "0.6.2"` (with `mail-builder` 0.5.0) is already in use — no need for `lettre` despite the roadmap's example. `mail-send` builds the SMTP envelope from the message's `To`/`Cc`/`Bcc` headers, which is why the recipient-construction bug above turns a display name into a delivery address.
- **"Load the config once" has a concrete Rust shape:** router state is currently just `Arc<Database>` ([api_routes.rs:21](../../src/routes/api_routes.rs#L21)), so there is nowhere for an `EmailConfig` to live. The fitting change is an `AppState` struct holding `Arc<Database>` plus an `Option<EmailConfig>` (or a small `EmailSender` wrapper around it), constructed in `main` and used as the router state; handlers then take `State<AppState>` instead of `State<Arc<Database>>`. That is the same state-widening the unused signing key needs ([main.rs:12](../../src/main.rs#L12)), so do it once for both. Keeping it `Option` means "email not configured" is a startup fact you can log once, not a surprise inside each send.
- **Sending off the request path** means `tokio::spawn(async move { ... })` after the DB transaction commits, which requires the future to own its data (clone the `EmailInformation`, don't borrow it) and means the handler can no longer report the failure — only log it.
- Each send currently opens a fresh TCP + TLS + AUTH cycle, with no retry and no connection reuse, and no send timeout is set: `SmtpClientBuilder` defaults to **one hour** (`mail-send-0.6.2/src/smtp/builder.rs:25`) and `create_new_email` never calls `.timeout(...)`, so an unresponsive host can hold the inline request path far longer than any user will wait. Fine for a handful of emails on a home server; worth revisiting if sends stay inline or a bulk upload batch starts sending many notifications in a row.
- Bodies are text-only, with no HTML alternative and no `Reply-To` header — this code sets nothing beyond `From`, `To`, subject and text body. `mail-builder` does add `Message-ID`, `Date` and `MIME-Version` itself when they are unset (`mail-builder-0.5.0/src/lib.rs:189-234`), so those are not missing; deliverability beyond them is whatever the configured `SMTP_*` relay gives you.
- ⚠️ **Round-trip constraint (from §4):** the rename notification tells the user a filename, and that filename must still match what the scanner's volume/chapter/page regex expects to read back — otherwise a re-scan won't recognise the uploaded file. Treat the naming convention as one shared spec between upload (write) and scan (read).
- The 8-character verification code, the 8-character reset token and the 32-character cancelation token all come from the same `Alphanumeric` sampler ([auth.rs:124](../../src/routes/auth.rs#L124), called at [auth.rs:134](../../src/routes/auth.rs#L134) and [auth.rs:332-333](../../src/routes/auth.rs#L332-L333)); they are the payload of these emails, so never log a body that contains one.

## Related
- [Authentication, Users & Sessions](03-auth-users-and-sessions.md) — owns both existing emails' surrounding flows: verification code issuance/checking, the password-reset token storage, and the no-SMTP auto-verify fallback.
- [Two-Factor Authentication (TOTP)](04-two-factor-auth-totp.md) — deliberately email-free; the "no delivery service to run" rationale lives there.
- [Uploads & File Ingestion](05-uploads-and-ingestion.md) — the rename and insufficient-storage notifications are decided there and delivered here.
- [Internationalization (i18n)](13-internationalization.md) — the catalog that email bodies move into, and the per-user locale that selects the language.
- [Operations & Security](12-operations-and-security.md) — generic error bodies and never logging secrets both apply to the email path.
- [Cross-Cutting Backend Polish](07-cross-cutting-backend-polish.md) — the `AppState` widening that lets the SMTP config be loaded once, and the testing strategy that would cover a send.
