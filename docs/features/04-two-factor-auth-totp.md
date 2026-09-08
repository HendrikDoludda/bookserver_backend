# Two-Factor Authentication (TOTP)

> A second login factor using authenticator-app codes (TOTP) plus single-use recovery codes, layered on top of the password login built in §3.

**Status:** 🟡 partial · **Phase:** 1 · **Tier:** Backend Tier 1 (item #5 — only after #4 works) · **Tags:** [BE] [DB] [FE]
**Depends on:** [03 Authentication, Users & Sessions](03-auth-users-and-sessions.md) (TOTP extends the login flow that core auth builds)
**Blocks:** [12 Operations & Security](12-operations-and-security.md) (the signing key’s one concrete use is encrypting the TOTP secret at rest)
**Roadmap:** [Build order](../../ROADMAP.md#priority-order-build-sequence)

## Why

This is explicitly a **learning project**: the point is to build a real second factor and understand the security work around it, not just to tick a box. TOTP was chosen because it needs no delivery service at all — no SMTP, no Twilio, no per-message cost — and it works offline: the shared secret plus the current time produce the same 6-digit code on the phone and on the server, independently. It also protects the one thing a home server can't hide behind Tailscale: a password that leaks elsewhere. Recovery codes exist so that a lost phone doesn't mean a locked-out account.

## Current state

The **schema and the plumbing are drafted; none of the TOTP logic exists.**

Live and working:
- `totp` table — [0001_initial_schema.sql:104](../../migrations/0001_initial_schema.sql#L104): `user_id INTEGER PRIMARY KEY`, `authentication_secret TEXT NOT NULL` (plaintext today), `created_at`, FK to `users(id) ON DELETE CASCADE`.
- `recovery_codes` table — [0001_initial_schema.sql:96](../../migrations/0001_initial_schema.sql#L96): `recovery_id` PK, `user_id`, `code_hash TEXT UNIQUE NOT NULL`, `used BOOLEAN DEFAULT FALSE`, FK cascade.
- `sessions` table with the access/refresh split and the half-session flag — [0001_initial_schema.sql:80](../../migrations/0001_initial_schema.sql#L80): `session_token_hashed` + `session_token_valid_until`, `refresh_token_hashed` + `refresh_token_valid_until`, and `authentication_completed BOOLEAN NOT NULL DEFAULT FALSE`.
- Models `TOTP` ([models.rs:503](../../src/data_models/models.rs#L503)) and `RecoveryCodes` ([models.rs:476](../../src/data_models/models.rs#L476)) with their column enums ([models.rs:509](../../src/data_models/models.rs#L509), [models.rs:483](../../src/data_models/models.rs#L483)).
- Both types have the **full trait set**: `FromRow` ([db_from_row.rs:96](../../src/database_related_scripts/db_from_row.rs#L96), [:108](../../src/database_related_scripts/db_from_row.rs#L108)), `Extract` ([extract.rs:96](../../src/database_related_scripts/extract.rs#L96), [:101](../../src/database_related_scripts/extract.rs#L101)), `Insert` ([insert.rs:212](../../src/database_related_scripts/insert.rs#L212), [:228](../../src/database_related_scripts/insert.rs#L228)), `Update` ([db_update.rs:105](../../src/database_related_scripts/db_update.rs#L105), [:100](../../src/database_related_scripts/db_update.rs#L100)), `Search` ([db_search.rs:137](../../src/database_related_scripts/db_search.rs#L137), [:132](../../src/database_related_scripts/db_search.rs#L132)).
- **The only live TOTP logic in the tree:** `log_in` probes for a `totp` row and returns `UserTotpEnabledResult { user_has_totp_enabled }` — [auth.rs:281-301](../../src/routes/auth.rs#L281). `create_new_session` then sets `authentication_completed = !user_has_totp_enabled` — [auth.rs:473](../../src/routes/auth.rs#L473). So a TOTP user's login already produces a deliberately half-authenticated session.
- The matching extractor exists: `AuthTOTPLoginSession` ([api_caller.rs:132](../../src/routes/api_caller.rs#L132)) and `validate_totp_login_session` ([api_caller.rs:148](../../src/routes/api_caller.rs#L148)), which reads the `session_token` cookie ([api_caller.rs:153](../../src/routes/api_caller.rs#L153)) and accepts it **only** when `authentication_completed` is false ([api_caller.rs:171](../../src/routes/api_caller.rs#L171)). Nothing uses it yet.

Empty comment-only stubs (zero executable statements, no callers):
- [totp_management.rs](../../src/routes/totp_management.rs) — all six functions: `begin_totp_setup` ([:2](../../src/routes/totp_management.rs#L2)), `confirm_totp_setup` ([:10](../../src/routes/totp_management.rs#L10)), `generate_recovery_codes` ([:19](../../src/routes/totp_management.rs#L19), private, opens with "don't know what is required for this call"), `verify_totp_log_in` ([:29](../../src/routes/totp_management.rs#L29)), `authenticate_using_recovery_code` ([:36](../../src/routes/totp_management.rs#L36)), `disable_totp` ([:45](../../src/routes/totp_management.rs#L45)). The bodies are the plan, written as comments.
- The five HTTP handlers — [api_caller.rs:617-625](../../src/routes/api_caller.rs#L617): `totp_start_set_up`, `finalize_totp_set_up`, `disable_totp`, `sign_in_totp_with_recovery_code`, `sign_in_totp`. Each is `-> Response<Body> {}` with a trailing comment naming which authorization it needs ("normal autherization" vs "totp sign in autherization").

Not started at all:
- **No routes.** [api_routes.rs:45-70](../../src/routes/api_routes.rs#L45) registers no TOTP path — not even a `ping_server` skeleton.
- Request DTOs exist but are unusable as bodies: `TOTPSignInRequest`, `RecoveryCodeSignInRequest`, `DisableTOTPRequest` in [totp_model.rs](../../src/data_models/totp_model.rs) are bare structs with **no `Deserialize` derive**, so they cannot back a `Json<T>` extractor.
- `totp-rs = "6.0.0"` is declared ([Cargo.toml:46](../../Cargo.toml#L46)) and **never imported** anywhere in `src/`. Same for `chacha20-poly1305 = "0.2.1"` ([Cargo.toml:24](../../Cargo.toml#L24)), which was added for the encrypt-at-rest option.
- `users` has **no `totp_enabled` column** ([0001_initial_schema.sql:51-60](../../migrations/0001_initial_schema.sql#L51)).
- `totp` has **no pending/confirmed state column**, which enrollment steps 1 and 3 both need.
- No table, column, or counter for verify rate-limiting or replay protection.
- The signing key that the encrypt-at-rest option would use is generated and then thrown away: [config.rs:20](../../src/config.rs#L20) builds it, [main.rs:12](../../src/main.rs#L12) binds it to `secrets` and never passes it on, and router state is `Arc<Database>` alone ([api_routes.rs:21](../../src/routes/api_routes.rs#L21)).

**Roadmap drift:** several things in [ROADMAP.md](../../ROADMAP.md) §3a no longer match the code.
- The four data-model bullets were all marked 🟡. Three are actually **done**: the secret table, `recovery_codes`, and the `sessions` access+refresh split all exist as specified. The fourth, `users.totp_enabled`, is **not started and probably never will be** — TOTP-enabled is derived at runtime from the presence of a `totp` row ([auth.rs:287-300](../../src/routes/auth.rs#L287)), so the column is redundant.
- The roadmap calls the secret table `authentication`; the migration calls it **`totp`**.
- The open decision "consider `UNIQUE(user_id)` — one secret per user" is **already settled** by `user_id INTEGER PRIMARY KEY` ([0001_initial_schema.sql:105](../../migrations/0001_initial_schema.sql#L105)).
- The roadmap's login flow says password and TOTP code are verified "in the *same* step (no second round-trip)". The code has committed to a **two-step half-session flow** instead (`authentication_completed`, `AuthTOTPLoginSession`, and separate `sign_in_totp` / `sign_in_totp_with_recovery_code` endpoints). See Open questions.
- Every task bullet is ⬜, yet these five empty handlers are the reason `cargo check` **fails for the whole crate** (five `E0308` errors: expected `Response<Body>`, found `()`). So 2FA is simultaneously "not started" and the thing blocking every other build — see Implementation notes for the one-line unblock.

## Decisions

- **TOTP, not email or SMS (DECIDED).** Authenticator-app codes on top of normal password auth. Chosen because there is no delivery service to run (no SMTP, no Twilio, no cost), it works offline, and it is more secure than SMS. The shared secret plus the current time produce the same 6-digit code on both phone and server independently.
- **Crate: `totp-rs`.** It covers secret generation, the `otpauth://` URI + QR, and skew-tolerant verification — the three things that would otherwise be hand-rolled crypto. Already in `Cargo.toml`, though secret generation and the URI each sit behind a cargo feature that is not switched on yet (see Implementation notes).
- **Build order: only after core auth works.** Tier 1 item #5, gated behind item #4 ([auth](03-auth-users-and-sessions.md)) because 2FA *extends* the login flow that #4 builds rather than standing alone.
- **Enrollment flow (agreed, four steps).** The secret is only trusted once the user has proved they can produce a live code from it — a user who mis-scans the QR must not end up locked out of their own account.
  1. In settings, user starts enrollment. Backend generates a random secret and returns it as a QR code (`otpauth://` URI); 2FA stays **disabled / pending**.
  2. User scans it into their authenticator app and types back the code it currently shows.
  3. Backend verifies that live code against the secret → on success, store the secret (the `totp` table), set 2FA enabled, and issue **recovery codes**. *(The roadmap wrote this step as "set `users.totp_enabled = true`"; the code derives enabled-ness from the stored row instead — see the bullet below.)*
  4. If they never confirm → the pending secret is discarded; 2FA stays disabled.
- **Login flow (roadmap DECIDED).** Password verified **and** current TOTP code verified in the *same* step (no second round-trip — nothing to send). On success, issue the session + refresh tokens. A valid **recovery code** can stand in for the TOTP code if the device is lost. *The code has since built a two-step flow instead — that divergence is an open question below, but the "recovery code substitutes for the TOTP code" half holds either way.*
- **Recovery codes are the recovery path.** A valid recovery code can stand in for the TOTP code when the device is lost. Ten codes, generated only when the user has none, displayed exactly once, stored hashed ([totp_management.rs:19-26](../../src/routes/totp_management.rs#L19)).
- **One secret per user.** `totp.user_id` is the PRIMARY KEY, so the schema enforces it — this closes the roadmap's "consider `UNIQUE(user_id)`" note.
- **TOTP-enabled is derived, not stored.** The code answers "does this user have 2FA?" by searching the `totp` table rather than reading a flag on `users` ([auth.rs:287](../../src/routes/auth.rs#L287)), so there is one source of truth and no flag to drift out of sync with the secret.
- **Access + refresh token split on `sessions` (DECIDED).** `session_token_hashed` is short-lived — 15 minutes ([auth.rs:507](../../src/routes/auth.rs#L507)) — and `refresh_token_hashed` is longer-lived with sliding expiry, refreshed on use — 30 days ([auth.rs:501-505](../../src/routes/auth.rs#L501)). Both are stored hashed, never in the clear.
- **The half-authenticated session is the TOTP gate (committed in code).** `authentication_completed` starts false for a TOTP user, `AuthSession`-gated routes refuse such a session, and `AuthTOTPLoginSession` accepts *only* such a session. This is a clean way to say "you proved the password, not the second factor" without inventing a second token type.
- **Disable requires re-authentication and is destructive.** Require the current password (or another strong verification), delete the secret, delete all recovery codes, invalidate all active sessions and tokens, mark 2FA disabled ([totp_management.rs:45-51](../../src/routes/totp_management.rs#L45)). Rationale: if an attacker holds a live session, turning 2FA off must not be a quiet one-click action, and the old sessions were authorized under the old factor set.
- **TOTP verify rate-limiting is the priority throttle** — ahead of ordinary login rate-limiting, which stays *(public only)*. A 6-digit code is only 1,000,000 combos, so it is brute-forceable even behind Tailscale, whereas there is no anonymous attacker on the tailnet to brute-force a password. (Also tracked from the security side in [Operations & Security](12-operations-and-security.md).)
- **TLS is part of the auth step, partly because of this feature.** Login sends the password **and** the TOTP code, so the transport is encrypted even on the LAN (defense-in-depth) — the reason in-app TLS via `axum-server` was pulled into §3 rather than left as "public only". Details live in [auth](03-auth-users-and-sessions.md) and [Operations & Security](12-operations-and-security.md).

## Open questions

- **Plaintext vs encrypted-at-rest for the TOTP secret.** A leaked `authentication_secret` lets an attacker generate valid codes forever; it is as sensitive as a password hash, and unlike a hash it is *directly usable* rather than one-way. Options: (a) plaintext — what the schema does today, zero extra moving parts; (b) encrypt with an AEAD (the roadmap names `chacha20poly1305` or `aes-gcm`; what is actually declared is `chacha20-poly1305 = "0.2.1"` — the rust-bitcoin crate, not RustCrypto's similarly-spelled one) using the 256-bit `token_signing_key` from `secrets.toml`. Option (b) is also the **only concrete job that key has** — the roadmap's position is that if nothing consumes it, the key should be removed rather than carried as unused crypto. The cost: the key has to reach handlers, which means growing router state past `Arc<Database>` into an `AppState`, since `main.rs` currently drops it. *Suggested:* encrypt with the existing key — it settles the signing-key question at the same time; only fall back to plaintext-with-a-TODO if the `AppState` refactor would stall 2FA entirely.
- **One-step vs two-step login — roadmap and code disagree.** The roadmap decided password + code in the same request ("no second round-trip — nothing to send"). The code built a two-step flow: login returns a half-session, then a separate TOTP endpoint completes it. One-step means fewer round trips and no partially-authenticated token to protect, but the client must know in advance whether to collect a code (or always send an optional code field, and the server must not leak "this account has 2FA" by rejecting differently). Two-step means the client learns from the login response that a code is needed, and the recovery-code path drops in as a sibling endpoint — at the price of a short-lived half-session that must be scoped to exactly two endpoints. *Suggested:* keep the two-step half-session the code already built — the column, the extractor and the endpoint names all exist — and correct the roadmap note rather than the code.
- **Where the pending secret lives.** Enrollment needs a pending → active transition, and `totp` has no state column. Options: (a) a `confirmed BOOLEAN NOT NULL DEFAULT FALSE` column on `totp`; (b) a separate `pending_totp` table; (c) hold it in server memory until confirmed. (c) loses the secret on restart mid-enrollment; (b) doubles the plumbing for one row. *Suggested:* (a) — smallest change, fits the one-row-per-user PK, and makes an abandoned enrollment just a row you overwrite or delete. While still in early dev this is an in-place edit of migration 0001 plus a DB wipe, not a new migration.
- **Which hash for recovery codes.** `code_hash TEXT UNIQUE` implies looking a code up *by* its hash, which needs a deterministic hash. The only existing helper for hashing secrets is Argon2 with a random salt ([auth.rs:532](../../src/routes/auth.rs#L532)), which cannot be looked up — you would have to load all ten of the user's rows and verify each one. Options: BLAKE3 hex, exactly how session tokens are already stored ([auth.rs:519-530](../../src/routes/auth.rs#L519)) — deterministic, keeps the UNIQUE index meaningful, and safe because the codes are high-entropy random rather than user-chosen; or Argon2 per code — slower, defeats the index, needs an N-row verify loop. *Suggested:* BLAKE3 over a high-entropy generated code, matching the session-token pattern.
- **Where the verify attempt counter lives.** No table or column exists for it. Options: an in-memory map in shared app state (simple; resets on restart, which is acceptable on a single-process home server but does hand an attacker a reset button if they can crash it); or a column/table so the count survives restarts (more writes, and a schema change). *Suggested:* in-memory in the shared state to start, since this is one process on one box — and write down the restart-resets caveat so it isn't rediscovered later.
- **Replay protection — do it or skip it.** The roadmap marks it optional. It needs a last-used-time-step value per user, i.e. another column on `totp`. The value itself is free: `totp-rs`'s verify call hands back the time step that matched, and the crate's own docs state it does *not* enforce single-use — that is the caller's job. *Suggested:* skip for v1 as the roadmap allows, but if you want it, add the column in the same edit as the `confirmed` flag so it is one schema change rather than two.
- **Lockout when both phone and recovery codes are gone.** The roadmap says "recovery codes (or admin reset)" but never specifies the admin path. Options: an admin-only endpoint that clears a user's `totp` row and recovery codes; or a manual SQLite edit. *Suggested:* manual DB edit for now — you are the only admin — and fold a proper reset into user management when the [admin web view](06-admin-web-view.md) lands.

## Tasks

### Data model

- ✅ `totp` table — holds the per-user TOTP `authentication_secret` **[DB]** *(roadmap called this the `authentication` table; it shipped as `totp`)*
- ✅ *(consider `UNIQUE(user_id)` — one secret per user)* **[DB]** — settled: `user_id` is the PRIMARY KEY
- ✅ `recovery_codes` table — `code_hash`, `used` **[DB]**
- ✅ `sessions` table — **access + refresh token split**: `session_token_hashed` (short-lived) + `refresh_token_hashed` (longer-lived, sliding expiry, refreshed on use) **[DB]**
- ⬜ `users.totp_enabled` flag **[DB]** — superseded: enabled-ness is derived from the presence of a `totp` row. Keep or formally drop this bullet.
- ⬜ Pending/confirmed state for an in-progress enrollment **[DB]** — required by enrollment steps 1 and 3; see Open questions for where it should live

### Backend crypto & verification

- ⬜ Secret generation + QR/`otpauth` URI (via `totp-rs`) **[BE]**
- ⬜ Skew-tolerant code verification (±1 time step) **[BE]**
- ⬜ Recovery-code generation + hashed storage + single-use redemption **[BE]**

### Endpoints

- ⬜ Enrollment endpoints: start (returns QR) + confirm (verify live code) **[BE][FE]**
- 🟡 Login: verify password + TOTP (or recovery code) together → issue tokens **[BE][FE]** — re-marked from the roadmap's ⬜: the password half and the token issuance already exist in `sign_in` ([api_caller.rs:409](../../src/routes/api_caller.rs#L409)), but the code/recovery-code half does not, and `/login` is still routed to `ping_server`
- ⬜ Refresh-token endpoint: exchange a valid refresh token for a new session token **[BE]**
- ⬜ Disable-2FA endpoint (re-auth required) **[BE][FE]**

### Security must-haves (the real learning content)

- ⬜ **Protect the secret at rest** — a leaked `authentication_secret` lets an attacker generate valid codes forever; it's as sensitive as a password hash. Decide plaintext vs encrypted-at-rest **[BE]**
- ⬜ **Verify attempt rate-limit** — a 6-digit code is only 1,000,000 combos; throttle/lock after N wrong tries or it's brute-forceable **[BE]**
- ⬜ **Replay protection (optional)** — track the last-used time step so the same code can't be reused within its 30s window **[BE]**
- ⬜ **Recovery path** — lose your phone → recovery codes (or admin reset) or you *will* lock yourself out **[BE][FE]**

### Frontend

- ⬜ Settings UI: enable (QR + confirm) / disable; show recovery codes once **[FE]**

## Done when

- [ ] Enrollment start on an authenticated session returns an `otpauth://` URI, stores the secret as pending, and leaves 2FA disabled; abandoning enrollment leaves it disabled with no usable secret.
- [ ] Confirming with the code the authenticator currently shows enables 2FA and returns exactly ten recovery codes, displayed once and never retrievable afterwards.
- [ ] A login by a TOTP-enabled user returns a session whose `authentication_completed` is false: an `AuthSession`-gated route answers 401 with it, and only the TOTP-completion endpoints accept it.
- [ ] A code from the current time step or ±1 step completes that session and issues the full session + refresh token pair; a code two steps old is rejected.
- [ ] After N wrong codes inside the window, further verification attempts are refused with a throttle response instead of being checked at all.
- [ ] A recovery code authenticates exactly once — a second use of the same code is rejected and its row's `used` is true — and disabling 2FA with the current password deletes the `totp` row, every `recovery_codes` row, and every session for that user.

## Implementation notes

**Unblock the build first.** The five empty handlers at [api_caller.rs:617-625](../../src/routes/api_caller.rs#L617) are the whole crate's compile errors. `todo!()` as a body type-checks as any return type (it diverges), so replacing `{}` with `{ todo!() }` gets `cargo check` green without pretending the endpoints work.

**`totp-rs` 6.0.0 shape.** The crate's type is `Totp`, and you get one from a builder rather than a constructor: `Builder::new().with_secret(secret).with_account_name(&user.email).with_issuer(Some("Bookserver")).build()?`. `Builder::new()` already defaults to SHA1, 6 digits, `step_duration: 30` and `skew: 1`, so **the "±1 time step" task is a default you inherit**, not code you write. Verification is `totp.check_current(&submitted_code)`, which returns `Option<u64>` — `None` for a bad code, otherwise the time step that matched (see the replay-protection question).

Three feature flags matter here, and [Cargo.toml:46](../../Cargo.toml#L46) enables none of them — the crate's defaults are only `std` + `migration`:
- `otpauth` gates the entire URL module, so **without it `Totp::to_url()` does not exist** and `with_issuer` / `with_account_name` are not even fields on the builder. Enrollment needs it.
- `gen_secret` makes `build()` generate a secret when none was set. Without it you supply the bytes yourself — which this codebase already knows how to do, via the `SysRng::try_fill_bytes` pattern in [auth.rs:519-530](../../src/routes/auth.rs#L519).
- `qr` (which implies `otpauth`) renders an actual QR image. Skipping it and returning only the URI is fine and matches what the stub asks for: "return the URI (or QR code) and the manual setup key" ([totp_management.rs:7](../../src/routes/totp_management.rs#L7)).

Naming: the crate's `Totp` and this project's `TOTP` model ([models.rs:503](../../src/data_models/models.rs#L503)) differ only in casing, which is easy to misread — alias one at the import (`use totp_rs::Totp as TotpGenerator;`).

**Broken plumbing you will hit on the way.** These are real, in the tree today:
- `RecoveryCodes::from_row` reads the column `"rocovery_id"` — [db_from_row.rs:111](../../src/database_related_scripts/db_from_row.rs#L111). A typo; every recovery-code read fails until it is fixed.
- `Sessions::from_row` reads `"last§"` for `last_used_at` — [db_from_row.rs:126](../../src/database_related_scripts/db_from_row.rs#L126). Another typo, and it means **every** session read currently fails, so `AuthTOTPLoginSession` cannot succeed until it is fixed (tracked in [auth](03-auth-users-and-sessions.md)).
- `Update`'s SQL is hard-coded `WHERE id = ?` ([db_update.rs:23](../../src/database_related_scripts/db_update.rs#L23), [:52](../../src/database_related_scripts/db_update.rs#L52)), but `totp`'s PK is `user_id` and `recovery_codes`' is `recovery_id`. So "mark this recovery code used" and "rotate the secret" both need either a widened `Update` trait or a dedicated `Database` method.
- `Extract` has the same hard-coded `WHERE id = ?1` ([extract.rs:19-29](../../src/database_related_scripts/extract.rs#L19)), so `Extract for TOTP` is unusable as written. Reach both tables through the `Search` trait instead — that is how `log_in` already does it ([auth.rs:287](../../src/routes/auth.rs#L287)).
- `DatabaseTypes` ([models.rs:18-26](../../src/data_models/models.rs#L18)) has no variant for `totp` or `recovery_codes`, so `Database::remove_entry` cannot address them. Disable-2FA needs new variants (and their table names) or its own delete methods.
- `setup_new_transaction` logs the inner error and returns a generic `DatabaseError::OperationFailure` ([db.rs:388-409](../../src/database_related_scripts/db.rs#L388)), so a specific "invalid TOTP code" cannot currently travel from inside a transaction closure out to the client. Verify *outside* the transaction, or fix the error passthrough.

**Request bodies.** Add `#[derive(Debug, Deserialize, Clone)]` to the three structs in [totp_model.rs](../../src/data_models/totp_model.rs), and take them as `Json<T>` as the **last** handler argument — Axum only lets the final argument consume the body, which is the same mistake several existing auth handlers make. Note `DisableTOTPRequest` already carries the `password` field the disable decision requires.

**Cookies vs JSON.** The half-session extractor reads a `session_token` **cookie** ([api_caller.rs:153](../../src/routes/api_caller.rs#L153)), but `create_auth_response` hands tokens back in a **JSON body** ([api_caller.rs:643](../../src/routes/api_caller.rs#L643)) and no `CookieManagerLayer` is installed on the router at all. Until those two halves meet, `AuthTOTPLoginSession` cannot receive a token — resolve it in [auth](03-auth-users-and-sessions.md) before wiring the TOTP endpoints.

**Reuse, don't reinvent.** Recovery codes can use `create_crypto_code(n)` ([auth.rs:124](../../src/routes/auth.rs#L124)) for generation and `create_new_token`'s BLAKE3 pattern ([auth.rs:519-530](../../src/routes/auth.rs#L519)) for hashing. Both `create_crypto_code` and `hash_string_securely` ([auth.rs:532](../../src/routes/auth.rs#L532)) are private to `auth.rs` today, so calling them from `totp_management.rs` means marking them `pub(crate)` first. Session issuance on successful verification is already written: `create_new_session` ([auth.rs:466](../../src/routes/auth.rs#L466)) — for the TOTP completion path you want to *flip* the existing half-session's `authentication_completed` to true rather than create a second row, since `sessions.device_id` is globally `UNIQUE` ([0001_initial_schema.sql:83](../../migrations/0001_initial_schema.sql#L83)) and a second insert for the same device would violate it.

**One live-code bug adjacent to this feature.** `verify_email_logic` hard-codes `user_has_totp_enabled: false` ([api_caller.rs:463-465](../../src/routes/api_caller.rs#L463)), so the session created by email verification is fully authenticated regardless of whether the user has 2FA. Once 2FA is real, that is a bypass — it should run the same probe `log_in` does.

**Encrypt-at-rest requires an `AppState`.** If you take the encrypted option, the key must be reachable from handlers. Today `main.rs` builds it and drops it, and router state is `Arc<Database>`. The Rust shape is a small `struct AppState { db: Arc<Database>, secrets: Arc<Secrets> }` used as the router state, with `FromRef<AppState>` impls so existing `State<Arc<Database>>` extractors keep working. `Secrets` is also currently a **private** struct returned from a `pub fn` ([config.rs:14](../../src/config.rs#L14)) — it needs to be `pub` before it can live in shared state. The declared AEAD crate is low-level: `ChaCha20Poly1305::new(key, nonce)` then `encrypt(&mut bytes, aad)` encrypts in place and returns a 16-byte tag, so *you* generate a fresh 12-byte nonce per write and the stored column has to carry nonce + ciphertext + tag, not just ciphertext. Reusing a nonce with the same key breaks the cipher, so that part is not optional.

**Never serialize the secret.** `TOTP` derives `Serialize` ([models.rs:502](../../src/data_models/models.rs#L502)), so returning one from a handler would put `authentication_secret` on the wire. The only value a client should ever see is the `otpauth://` URI during enrollment.

## Related

- [Authentication, Users & Sessions](03-auth-users-and-sessions.md) — the prerequisite: password login, sessions, the token pair, the cookie layer and the extractors this feature extends
- [Operations & Security](12-operations-and-security.md) — the signing key's one concrete job is encrypting this secret; also carries the rate-limiting stance, TLS, session revocation and the security audit log
- [Cross-Cutting Backend Polish](07-cross-cutting-backend-polish.md) — the `Deserialize` derives the TOTP DTOs still need, correct status codes, and generic error bodies for failed verification
- [Backend Admin Web View](06-admin-web-view.md) — where an admin-side 2FA reset for a locked-out user would live
- [Internationalization (i18n)](13-internationalization.md) — the enrollment/settings screen's strings go through `t("key")` like every other screen
- [Email & Notifications](14-email-and-notifications.md) — deliberately *not* involved: needing no delivery service is the reason TOTP was chosen over email or SMS codes
