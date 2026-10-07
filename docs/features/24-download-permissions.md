# Download Permissions & Protected Delivery

> A per-user permission deciding who may take the original file home and who may only view it through the app, on the server's terms.

**Status:** ⬜ not started · **Phase:** 2 · **Tier:** Parked — out of focus · **Tags:** [DB] [BE] [FE]
**Depends on:** [03 Authentication, Users & Sessions](03-auth-users-and-sessions.md) (a per-user permission needs a session-derived user and a role model) · [02 Reading & Read State](02-reading-and-read-state.md) (this gates the offline-download endpoint that doc owns)
**Blocks:** Nothing
**Roadmap:** [Build order](../../ROADMAP.md#priority-order-build-sequence)

## Why

Trust in a household server is not binary. Some people you are happy to hand the actual file
to — they should be able to download a movie or a book and keep their own backup, and the
server has no business getting in the way. Other people you want to give access to the
library without giving away copies of it: they use the app, they watch or read, and when
access ends there is nothing left on their disk. Today the server makes no distinction —
every authenticated user who can reach a file gets the same bytes the same way — so the only
way to express "this person may keep it, that person may not" is to not share the library
with them at all.

The feature is the permission, not a copy-protection system. It exists so the *policy* is
expressible and enforced at the one place the server actually controls: whether it ever
hands out the original file. Parked in Phase 2 deliberately — it is a refinement on top of
auth, roles and the download endpoint, none of which exist yet.

## Current state

Nothing exists for this feature. A grep for `download`, `permission`, `drm`, `licence` /
`license`, `expires_at` on anything but auth tokens, and `Content-Disposition` across `src/`
and `migrations/` returns no hit that belongs to this feature.

What exists is the machinery it would attach to, and the holes it has to close:

- **No permission model beyond one boolean.** `users` has `is_admin BOOLEAN NOT NULL DEFAULT FALSE`
  ([0001_initial_schema.sql:57](../../migrations/0001_initial_schema.sql#L57)) and nothing
  else; `UserMetadata` mirrors it ([models.rs:259-268](../../src/data_models/models.rs#L259)).
  There is no user↔library table — the only junction in the schema is `library_elements`
  (library↔series, composite PK, no `id`) ([0001_initial_schema.sql:32](../../migrations/0001_initial_schema.sql#L32)).
- **The role gate exists but is wired to nothing.** The `AdminSession` extractor compiles
  ([api_caller.rs:71-88](../../src/routes/api_caller.rs#L71)) and is used by no route.
- **The download endpoint this would gate does not exist.** Offline download is an unbuilt
  ⬜ task in [Reading & Read State](02-reading-and-read-state.md), which still has its own
  open question about whether it is a separate route or a flag on the streaming one.
- **The streaming route has no auth at all.** `GET /api/v1/book/:id` → `request_file`
  ([api_routes.rs:55](../../src/routes/api_routes.rs#L55),
  [api_caller.rs:290-299](../../src/routes/api_caller.rs#L290)) takes `State` + `Path` + the
  raw `Request` and streams `metadata.file_path` off disk with no session extractor in
  sight. The standing note at the top of the file says as much
  ([api_caller.rs:1](../../src/routes/api_caller.rs#L1)), as does the unresolved
  path-containment TODO ([api_caller.rs:4](../../src/routes/api_caller.rs#L4)). A permission
  that gates a route anyone can call unauthenticated is decorative.
- **Delivery is a plain static file serve.** `streaming_file`
  ([stream_reader.rs:10-13](../../src/stream_reader.rs#L10)) hands the request to
  `tower_http::services::ServeFile` — three lines, no headers of its own, no hook where a
  per-user decision could be applied today.
- **A signing key is generated and consumed by nobody.** `set_up_config_file` writes a
  256-bit `token_signing_key` (plus a `previous_token_signing_key` and its expiry) into
  `secrets.toml` on first run ([config.rs:14-17](../../src/config.rs#L14),
  [config.rs:47-56](../../src/config.rs#L47)). Nothing reads it. Short-lived signed download
  URLs would be its first real consumer — see [Operations & Security](12-operations-and-security.md).
- **Two planned doors bypass any permission by design.** The WebDAV mount for Infuse
  ([Video Library](20-video.md)) publishes a directory tree over basic-auth, and the OPDS
  acquisition feed ([Interop & Client Apps](23-interop-and-clients.md)) exists to hand
  third-party readers a download link. Neither protocol has a notion of "this user may
  stream but not keep."

Roadmap drift: none — the feature is new, and nothing in the code contradicts ⬜.

## Decisions

- **Download is a permission, not a global setting (DECIDED).** The server distinguishes
  users who may take the original file from users who may only view it through the app.
  Rationale: on a shared household server the same library is handed to people at different
  levels of trust, and the alternative — one global on/off — forces the whole library to the
  strictness of its least-trusted member.
- **Two modes, and the first one is the plain file (DECIDED).** Permission granted → the
  user gets the original, unmodified file, theirs to back up. Permission denied → access is
  through the app only, optionally time-limited. Rationale: the point of granting the
  permission is that the file is genuinely usable elsewhere, so the "allowed" path must not
  quietly re-encode, watermark or wrap anything.
- **Phase 2, parked.** Not scheduled. It sits on top of auth, roles and the download
  endpoint, none of which are built, and the Phase 1 ladder has a hard gating rule of one
  tier at a time. Rationale: it is a refinement of sharing, and there is nothing to share yet.
- **The permission is read from the session, never from the request (DECIDED by inheritance).**
  The §11 hardening rule — derive the user id from the session token, never accept it in the
  body — applies here with teeth: a client-supplied "I'm allowed to download" is the whole
  feature defeated in one request. See [Operations & Security](12-operations-and-security.md).
- **API-first (DECIDED by inheritance).** The check lives in the backend handler, not in the
  app's UI. Hiding the download button is a courtesy; refusing the request is the feature.
- **Protected delivery is a policy speed bump, not a security guarantee (DECIDED, and worth
  stating plainly).** Anything the app can display, the person holding the device can
  capture — a decryption key the client holds is a key the client's owner holds, and a
  screen recorder defeats every scheme regardless. So this doc does not promise that a
  restricted user *cannot* obtain a copy; it promises the server never *hands* them one, and
  that taking one is deliberate effort rather than a button. Rationale: writing the weaker,
  true guarantee down now prevents building an expensive DRM pipeline later under the
  impression it delivers a strong one.

## Open questions

- **What is the permission attached to — the user, the library, or the pair?** Options:
  (a) one `can_download` boolean on `users` — trivial, one column, matches the "permissions
  setting for users" framing, but cannot express "may keep books, may not keep films";
  (b) per-library, on a user↔library table — expresses the movie-vs-book case naturally and
  is the same table shape three other features want; (c) per-item — maximum control, and a
  row per user per book to administer. *Suggested:* (b), with the boolean on `users` as the
  default the per-library row overrides. The motivating example in the original framing was
  a *movie*, and video is exactly the content class most likely to differ from books.
- **Is this the same mechanism as per-library visibility, or a second one?** [Sensitive
  Content Controls](10-sensitive-content-controls.md) wants per-user visibility, idea 3 in
  the [Ideas Backlog](90-ideas-backlog.md) wants per-library permissions ("a kids' library"),
  and this wants per-user download rights. That is three features reaching for one
  user↔library access table, and the backlog already carries an open question warning that
  two visibility checks which can disagree are worse than one. Options: one `user_library_access`
  table with columns per capability (`can_view`, `can_download`, …), or independent
  mechanisms per feature. *Suggested:* settle a single access model before any of the three
  is built, and make this doc's permission a column on it rather than a table of its own.
- **What exactly does the time limit apply to?** Two readings of "a time limit on it", with
  very different costs. (a) **A loan window on access:** the user may stream this title until
  a date, after which the server stops authorizing it. Fully server-enforceable, needs no
  client cooperation, and is a row with an `expires_at`. (b) **An expiry on a downloaded
  copy:** the app holds a file that stops working after N days. Requires the client to
  enforce its own restriction, which the device owner controls — see the speed-bump decision.
  *Suggested:* build (a) and call it a loan; treat (b) as a client convenience (the app
  clears its cache) that is never described as enforcement.
- **What does "protected file" mean concretely for the denied path?** Options, in rising
  cost: (i) nothing special — stream it as today, just never expose a download route or a
  stable file URL to that user; (ii) short-lived signed URLs, so the address a restricted
  user's app holds is useless minutes later and cannot be shared; (iii) encrypted-at-rest
  delivery with the app decrypting in its player pipeline — real work for video (a standard
  player cannot play an encrypted stream without a custom pipeline; `media_kit` would need
  one) and a guarantee that still ends at a screen recorder. *Suggested:* (ii). It is
  genuinely enforceable, it is cheap, and it is the first honest use of the signing key that
  already sits unused in `secrets.toml`.
- **What happens to the WebDAV door?** Infuse cannot express this permission, and the mount
  is a directory tree of real files — a restricted user with WebDAV credentials copies
  anything they can list. Options: don't issue WebDAV credentials to download-restricted
  users at all (simple, and the credential is already planned as device-scoped rather than
  the account password); or root a second, narrower mount per user (more moving parts).
  *Suggested:* the first — treat "has WebDAV access" as implying the download permission,
  and say so in the admin UI, rather than pretending the door enforces something it cannot.
- **And the OPDS feed?** Its acquisition links are download links by definition. *Suggested:*
  either omit acquisition links for restricted users (leaving a browse-only feed) or don't
  issue OPDS access to them — decide when OPDS is actually built, and record it there too.
- **What is the default for a new user?** Options: denied by default (safe, but every new
  household member needs an admin action before the app is fully useful) or allowed by
  default (friction-free, and forgetting to change it is the failure mode). *Suggested:*
  denied by default — the whole point is that granting it is a deliberate act of trust.
- **Should downloads be logged?** A per-user audit line ("who took which file, when") is
  cheap and is the only way to notice the permission being used at scale. It also means
  keeping a record of household members' reading and viewing, which is a real privacy cost
  on a personal server. *Suggested:* log downloads of the original file only, not streams,
  and keep it short-lived; the security audit log in [Operations & Security](12-operations-and-security.md)
  is the natural home.

## Tasks

- ⬜ Settle the access model — one shared user↔library table or a mechanism of its own (see Open questions, and the overlap with [Sensitive Content Controls](10-sensitive-content-controls.md) and idea 3) **[DB]**
- ⬜ Migration: the download permission, on whatever the access model settles on, plus the optional loan window (`expires_at`) **[DB]**
- ⬜ Model + trait impls for the new table/column, following the existing `FromRow` / `Insert` / `Update` / `Search` pattern **[BE]**
- ⬜ Enforce the permission on the download endpoint — a restricted user gets `403`, never bytes **[BE]**
- ⬜ Short-lived signed URLs for the restricted streaming path, consuming the `token_signing_key` that `secrets.toml` already holds **[BE]**
- ⬜ Expose the permission in the user's API response so the app can hide the download affordance (courtesy only — the server still refuses) **[BE][FE]**
- ⬜ Admin UI: set the permission per user (and per library, if the access model goes that way) **[FE]**
- ⬜ Close the WebDAV and OPDS bypasses, per whatever the open questions settle on **[BE]**
- ⬜ Optional: loan window — access to a title expires on a date, server-side **[DB][BE][FE]**

## Done when

- [ ] A user with the permission calls the download endpoint and receives the original file, byte-identical to the one on disk.
- [ ] A user without it calls the same endpoint and gets `403` with no bytes in the body — and can still stream and read/watch the same title normally.
- [ ] The permission is read from the session on every request; a request body or query parameter claiming the permission changes nothing.
- [ ] A signed streaming URL issued to a restricted user stops working after its expiry, and the same URL pasted into another client is rejected once expired.
- [ ] A download-restricted user either has no WebDAV credentials at all, or the mount they can reach exposes nothing they are restricted from — verified by actually pointing a client at it.
- [ ] Toggling the permission takes effect on the next request, with no re-login and no restart.
- [ ] *(If the loan window is built)* access to a loaned title stops being authorized after its expiry, enforced server-side with the client offline-hostile — not by asking the app to stop.

## Implementation notes

- **Nothing here is meaningful until the streaming route is authenticated.** `request_file`
  ([api_caller.rs:290-299](../../src/routes/api_caller.rs#L290)) has no session extractor,
  and the path-containment TODO ([api_caller.rs:4](../../src/routes/api_caller.rs#L4)) is
  still open. Both belong to [Auth](03-auth-users-and-sessions.md) and
  [Operations & Security](12-operations-and-security.md); this feature is a no-op until they
  land, and building it first would produce a permission check sitting in front of an open door.
- **The allowed path is one header on the response.** `Content-Disposition: attachment; filename="…"`
  on top of the existing `ServeFile` serve — set it on the returned `Response` in the
  handler, or layer `tower_http::set_header` on that one route. `tower-http` is already
  pulled with `features = ["full"]`, so nothing new is needed in `Cargo.toml`. Do not
  re-implement range handling: `ServeFile` already gives 206 / `If-Range` / `HEAD` for free
  ([stream_reader.rs:10-13](../../src/stream_reader.rs#L10)).
- **The signing key is already there, waiting.** `Secrets` holds `token_signing_key` plus
  `previous_token_signing_key` and an expiry ([config.rs:14-17](../../src/config.rs#L14)) —
  that shape is a key-rotation design, so signed URLs should verify against the current key
  *and* the previous one while it is unexpired, or every outstanding link breaks on rotation.
  The key is currently read into an `Arc<Secrets>` that nothing consumes.
- **A signed URL carries its own claims and its own expiry** — typically the book id, the
  user id and an expiry timestamp, HMAC'd with the key, with the signature checked before
  the file is touched. The reason it is worth more than a bare session check is that it can
  be handed to a media player that has no session, and it becomes worthless on its own
  schedule.
- **The permission check belongs in one place.** The repo's idiom for this is an Axum
  extractor — a struct plus `impl FromRequestParts<Arc<Database>>`, exactly as `AuthSession`
  and `AdminSession` do ([api_caller.rs:71-88](../../src/routes/api_caller.rs#L71)), with the
  new one composing on top of `AuthSession` rather than re-parsing cookies. A
  `DownloadPermitted` extractor on the download route keeps the rule in one file instead of
  scattered `if` statements across handlers.
- **The Rust shape for the new table is the one this repo repeats:** one metadata struct in
  [models.rs](../../src/data_models/models.rs), a `*DatabaseColumns` enum beside it with a
  hand-written `impl AsRef<str>`, and small `FromRow` / `Insert` / `Update` / `Search` impls
  in `src/database_related_scripts/`.
- **Name the primary key `id`.** `Extract` and `Update` hard-code `WHERE id = ?1` /
  `WHERE id = ?2` ([extract.rs:21](../../src/database_related_scripts/extract.rs#L21),
  [db_update.rs:23](../../src/database_related_scripts/db_update.rs#L23)); every auth table
  that named its key something else inherited SQL referencing a column that does not exist.
  If the access model ends up as a pure junction table with a composite key, follow
  `library_elements` instead and give it dedicated `Database` methods rather than an
  `Extract` impl.
- **Register the migration** as a new numbered `.sql` plus an entry in the `MIGRATIONS` array
  ([migrations.rs:16](../../src/database_related_scripts/migrations.rs#L16)). By Phase 2 there
  is real user data, so the early-dev habit of editing `0001` in place and wiping the DB is
  over.
- **Declare the FKs explicitly.** The pool sets `PRAGMA foreign_keys = ON` per connection
  ([db.rs:36](../../src/database_related_scripts/db.rs#L36)), but only where a `FOREIGN KEY`
  clause is actually written — `books.series` has none
  ([0001_initial_schema.sql:19](../../migrations/0001_initial_schema.sql#L19)), so the
  pattern is not uniform. A permission row should cascade from `users`.
- **Encrypted delivery is where the cost jumps, and it is worth resisting.** Wrapping files
  so only the app can open them means a key exchange, key storage on the device, and a
  decrypting pipeline inside the player. For books that is awkward; for video it means the
  in-app player can no longer be a stock component, which collides with the "no server
  transcoding, the client just plays the file" decision in [Video Library](20-video.md). The
  return is a guarantee that a screen recorder still defeats. Signed short-lived URLs get
  most of the practical benefit for a fraction of the work.
- **`403`, not `404`.** A restricted user knows the title exists — they can see it in the
  library — so hiding its existence buys nothing and makes the app's error handling worse.
  Keep the generic-body / detail-logged-server-side shape that `DatabaseError::into_response`
  established.

## Related

- [Reading & Read State](02-reading-and-read-state.md) — owns the offline-download endpoint this permission gates, including the still-open "separate route or a flag" question.
- [Authentication, Users & Sessions](03-auth-users-and-sessions.md) — hard prerequisite: supplies the session-derived user, the role model, and the extractor pattern the permission check copies.
- [Sensitive Content Controls](10-sensitive-content-controls.md) — wants per-user visibility over the same libraries; the candidate home for one shared access model.
- [Operations & Security](12-operations-and-security.md) — the "derive the user from the session, never the body" rule, the unused signing key in `secrets.toml`, and the audit log a download record would join.
- [Video Library](20-video.md) — the WebDAV door for Infuse hands out raw files with no notion of this permission; the motivating example (a movie someone may or may not keep) is this content class.
- [Interop & Client Apps](23-interop-and-clients.md) — OPDS acquisition links are download links by definition, and need the same decision.
- [Ideas Backlog](90-ideas-backlog.md) — idea 3 (per-library permissions) is the third feature reaching for the same user↔library table.
- [Uploads & File Ingestion](05-uploads-and-ingestion.md) — the other half of "who may move files in or out of this server"; both are role-gated capabilities rather than content features.
