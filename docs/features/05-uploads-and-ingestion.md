# Uploads & File Ingestion

> Let a user push a file (or a whole batch) into the server from the frontend, have it land in the right series folder under the right name, and get picked up by the same pipeline the scanner uses.

**Status:** ⬜ not started · **Phase:** 1 · **Tier:** Backend Tier 4 (first item in the tier) · **Tags:** [BE] [FE]
**Depends on:** [03 Authentication, Users & Sessions](03-auth-users-and-sessions.md) (uploads are attributed to a user; whether upload is admin-only or open to any authenticated user is an open question below) · [01 Library & Directory Management](01-library-and-directory-management.md) (needs the scannable-directories table to know where a new series may live) · [14 Email & Notifications](14-email-and-notifications.md) (the rename and out-of-space policies both notify by email)
**Blocks:** Nothing
**Roadmap:** [Build order](../../ROADMAP.md#priority-order-build-sequence)

## Why

Today the only way a book gets into the library is to put the file on disk yourself and trigger a scan. Uploading from the frontend closes that loop: drag a file into a series view and it lands in the correct folder, named per the convention, with its DB row, cover and hash already created. The hard part is not the HTTP transfer — it is that the *names the uploader writes* must be the exact names the scanner knows how to *read back*, or a re-scan stops recognising the server's own files. That round-trip is why this section carries so much detail about field resolution, previews and rename schemes.

## Current state

No upload code exists anywhere in the tree — no route, handler or model mentions uploads or multipart, and there is no free-space check and no ingestion path other than the directory scanner. The route table at [api_routes.rs:45-70](../../src/routes/api_routes.rs#L45) has no upload path, and the file header TODOs in [api_caller.rs:1-4](../../src/routes/api_caller.rs#L1) still list the prerequisites ("Requires a database table for directories that should be scannable", "Check if the file paths that get served are inside the database table for scannable files or the cover images folder"). What already exists is everything this feature has to reuse:

- **Live and reusable:** the scanner's per-file ingestion, [`create_book_entry`](../../src/scanner/folder_scanner.rs#L96) ([folder_scanner.rs:96-161](../../src/scanner/folder_scanner.rs#L96)) — format detection, file metadata, BLAKE3 hash ([folder_scanner.rs:295-299](../../src/scanner/folder_scanner.rs#L295)), path resolution, series resolve-or-create, cover extraction, insert. This is the "reuse the scanner pipeline" the roadmap task means.
- **Live and reusable:** the filename parser [`get_final_file_name`](../../src/scanner/folder_scanner.rs#L239) ([folder_scanner.rs:239-269](../../src/scanner/folder_scanner.rs#L239)) plus its three regexes ([folder_scanner.rs:22-29](../../src/scanner/folder_scanner.rs#L22)) — this is the read half of the round-trip spec.
- **Live and reusable:** series resolve-or-create and library linking — [`get_series_id`](../../src/scanner/folder_scanner.rs#L301) ([folder_scanner.rs:301-314](../../src/scanner/folder_scanner.rs#L301)), [`create_series_entry`](../../src/scanner/folder_scanner.rs#L316) ([:316-334](../../src/scanner/folder_scanner.rs#L316)), [`link_series_to_default_library`](../../src/scanner/folder_scanner.rs#L336) ([:336-347](../../src/scanner/folder_scanner.rs#L336)). Inline series creation is exactly this code with a user-supplied name instead of a parsed one.
- **Live and reusable:** natural sort is already a dependency and already used for picking the first page of an image folder — `natord::compare` at [cover_image_retriever.rs:3](../../src/scanner/cover_image_retriever.rs#L3), applied in [cover_image_retriever.rs:143-171](../../src/scanner/cover_image_retriever.rs#L143). The batch ordering needs the same call.
- **Live and reusable:** cover extraction [`get_cover_image`](../../src/scanner/cover_image_retriever.rs#L16) ([cover_image_retriever.rs:16-28](../../src/scanner/cover_image_retriever.rs#L16)) — PDF / CBZ / EPUB / image dispatch (EPUB path untested).
- **Live and reusable:** the email sender [`create_new_email`](../../src/routes/email_helper.rs#L7) ([email_helper.rs:7-39](../../src/routes/email_helper.rs#L7)) over `mail-send` ([Cargo.toml:30](../../Cargo.toml#L30)), with SMTP config read from env on every send ([config.rs:80-89](../../src/config.rs#L80)). The rename / out-of-space notices can go straight through it.
- **Route skeleton wired to a placeholder handler:** the generic write endpoints an upload would otherwise piggyback on are hollow — `PUT /api/v1/update_entry` and `POST /api/v1/insert_entry` both land in [`insert_database_entry`](../../src/routes/api_caller.rs#L333) / `update_database_entry` ([api_caller.rs:328-336](../../src/routes/api_caller.rs#L328)), which immediately `return failed_getting_response()` ([api_caller.rs:342-344](../../src/routes/api_caller.rs#L342)) — an HTTP **200** with the body `"Failed to get response"`. Their comment still says "models don't derive Deserialize", which is now stale (the derives landed).
- **Route skeleton wired to a placeholder handler:** `GET /api/v1/search` ([api_routes.rs:65](../../src/routes/api_routes.rs#L65)) is bound to `ping_server`, so there is no series-lookup endpoint for the auto-complete yet. The only existing series-by-name lookup is `Database::get_id_from_table`, which carries the comment `//should not be used` at [db.rs:355](../../src/database_related_scripts/db.rs#L355); `BookSeriesMetadata` implements the sanctioned `Search` trait instead ([db_search.rs:102-105](../../src/database_related_scripts/db_search.rs#L102)).
- **Missing prerequisite:** scannable roots still come from the `BOOK_DIRS` env var ([config.rs:68-74](../../src/config.rs#L68)), so "ask where the new series should live" has no set of roots to offer. Libraries created by the scanner are also hardcoded to `LibraryType::Books` ([folder_scanner.rs:363](../../src/scanner/folder_scanner.rs#L363)), so "library type drives the default field set" has nothing varied to key off yet.
- **Missing prerequisite:** the router installs no middleware at all ([api_routes.rs:45-70](../../src/routes/api_routes.rs#L45) — no `.layer(...)` anywhere), and axum's `multipart` feature is not enabled ([Cargo.toml:7](../../Cargo.toml#L7)). No free-space crate is in the tree.

Roadmap drift: the §4 markers themselves are accurate — nothing here is built. The stale bit is inside the section: the collision-policy note says the email dependency means "SMTP, e.g. Rust `lettre`", but the backend already sends mail through `mail-send` with a working `create_new_email` ([email_helper.rs:7-39](../../src/routes/email_helper.rs#L7)), so this is reuse, not a new crate. Also worth knowing before starting: the crate does not currently compile (five empty-bodied TOTP handlers at [api_caller.rs:617-625](../../src/routes/api_caller.rs#L617)) and the router panics at startup on axum 0.8's rejection of `:id` path syntax — see [Cross-Cutting Backend Polish](07-cross-cutting-backend-polish.md).

## Decisions

- **Upload writes to the correct series folder on disk, and the backend owns the naming.** The client says *what* the file is; the backend decides *where* it goes and what it is called. On new-series creation the backend "creates the folder in the correct structure + location and names everything per the convention". Rationale: the naming convention is a spec shared between upload and scan, so only one side may author it.
- **Post-upload pipeline reuses the scanner pipeline.** Create the DB entry, extract the cover, compute the hash — via the scanner's existing per-file path rather than a second implementation. Rationale: two ingestion implementations would drift, and the scanner's version is already the one that works.
- **Dialog field sets are driven by content type.** *Video* gets a **kind** dropdown — `movie` / `series` / `original`; `movie` and `original` need no extra fields, `series` gets **series**, **season** and **episode**. *Books / comics* get a **series** field plus **volume** and **chapter** — chapter only matters when the thing being uploaded is a chapter rather than a whole volume.
- **Library type drives the default mode.** Libraries are typed (series vs movies, books vs comics), so the dialog defaults to the right field set. A *movies* library does not auto-fill series/episode at all — "clear series info" is an escape hatch, not the common path. Rationale: the common case should need no typing, and the field set should not have to be chosen by hand every time.
- **Bulk upload: global defaults + per-file override (DECIDED).** Accept multiple files, ingest them **sequentially** (one after the other), with optional global fields at the top of the batch and per-file rows that can each override them. Rationale: a well-named season folder should need almost nothing typed, while a badly named one is still fixable file by file.
- **Per-field resolution cascade, highest wins (DECIDED): per-file manual edit → global field (only if filled) → value parsed from the filename → empty.** Leaving a global field **empty** means "fall through to the filename for that field", so a well-named season folder just needs the series title set (or nothing at all) and its season/episode come straight from the filenames. Rationale: resolution is per *field*, not per *file*, so the two input styles compose instead of fighting.
- **A filled global season + starting-episode overrides and auto-increments across the batch.** Rationale: the common bulk case is "these twelve files are season 2, episodes 1-12" where the filenames don't say so.
- **Auto-increment orders files by natural sort** (`ep2` before `ep10`). Rationale: plain lexicographic order would number a ten-plus-episode batch wrong.
- **"title" means different things by context.** Series batch → global title = **series name**, per-file title = **episode name**. Movies batch → each file's title = the **movie name**.
- **Mandatory preview/confirm step before anything touches disk.** Show each file's *resolved* season/episode/title **and** its target path/name, editable inline. Rationale: a wrong global increment or a naming mismatch is caught and fixed *before* files land, instead of being cleaned up afterwards.
- **Collision policy (DECIDED): auto-rename, and notify the uploader by email.** When a resolved name already exists on disk, rename rather than overwrite or fail, and email the uploader that the file was renamed — to their `users.email`. Rationale: never silently destroy an existing file, and never leave the user guessing what the file ended up called. This reintroduces an email/SMTP dependency that TOTP auth deliberately avoided; it is needed here purely for the rename notification.
- **Rename scheme (DECIDED): timestamp suffix.** The renamed file must still parse cleanly back through the scanner's filename regex (see the round-trip constraint below).
- **Insufficient-storage policy (DECIDED): pre-flight free-space check.** If the drive cannot hold the file, **drop the upload** — do not write a partial file — and notify the user by email. Check *before* writing so nothing half-lands. In a bulk batch, drop the offending file and report it; the rest of the batch is unaffected, which is possible precisely because ingest is sequential.
- **Series field behaviour — applies to both the video-series field and the book/comic-series field: auto-complete plus inline creation.** Auto-complete against existing series (needs a series-lookup/search endpoint); if the typed name isn't found, offer to create it; on new-series creation ask which scannable root it should live under; the backend then creates the folder and names everything per the convention.
- **The naming convention is a single shared spec between upload (write) and scan (read).** ⚠️ **Round-trip constraint:** the names and paths the uploader generates MUST match what the scanner's filename parser (`folder_scanner.rs` volume/chapter/page regex) expects to read back, or a re-scan won't recognise uploaded files. The read half of that spec — the three regexes and the title cleaner, quoted from the code — is the first block under *Implementation notes*.

## Open questions

- **Where does the resolution cascade actually run?** The roadmap tags it **[BE][FE]**, so both sides could compute it. Options: (a) the frontend computes the preview and the backend recomputes authoritatively on confirm — two implementations that can disagree, and the preview stops being trustworthy; (b) the backend computes the resolved plan and returns it *as* the preview payload, the frontend only renders and submits edits — one implementation, an extra round-trip before confirm. *Suggested:* (b) — it matches the API-first guiding principle ("if any behaviour lives only inside one UI, the principle is broken") and makes the preview literally the plan that will execute.
- **What happens to a rename / out-of-space notice when SMTP isn't configured?** `get_email_config()` returns `None` if *any* SMTP var is missing ([config.rs:80-89](../../src/config.rs#L80)) and `create_new_email` then fails with `EmailErrors::EmailSetUpNotFound` ([email_helper.rs:12](../../src/routes/email_helper.rs#L12)). Options: fail the whole upload; ingest anyway and report the rename only in the HTTP response; queue the notice for later. *Suggested:* ingest and report it in the response plus the log — the email is a courtesy, not the record of truth. Do **not** copy the pattern `sign_up` uses, where a missing SMTP config silently changes the security outcome ([api_caller.rs:368-382](../../src/routes/api_caller.rs#L368)).
- **Does the timestamp rename suffix survive the title parser?** Because the parser strips only the three regex matches, a bare `..._1736451200.pdf` suffix ends up in the parsed title and would resolve to a *new*, lowercased series. Options: teach the parser to strip a well-known timestamp token; place the timestamp somewhere the cleaner already removes; accept a polluted title on the (rare) collision. *Suggested:* strip a recognised timestamp token in the parser — the naming convention is already a shared spec, so both halves are allowed to know about it.
- **`LibraryType` has nothing to key the video field set off.** The enum is `Books | Comics | Magazines | Documents | Others` ([models.rs:52-58](../../src/data_models/models.rs#L52)) — no movies/series/video variant — and the scanner hardcodes `LibraryType::Books` for every library it creates ([folder_scanner.rs:363](../../src/scanner/folder_scanner.rs#L363)). Options: add the variants now against a feature that is Phase 2; ship books/comics first and add the video variants with [Video Library](20-video.md). *Suggested:* ship the books/comics field set keyed on `Books` vs `Comics`, and add the video kinds when the video feature lands — the dialog's field-set switch is the part worth building generically.
- **What does uploading a single image page mean?** The scanner treats a folder of images as one book and stores the parent directory as `file_path` ([folder_scanner.rs:218-226](../../src/scanner/folder_scanner.rs#L218)), and `books.file_path` is `UNIQUE` ([0001_initial_schema.sql:11](../../migrations/0001_initial_schema.sql#L11)). Options: reject single loose images; accept a whole folder / set of pages as one upload unit and register the folder path. *Suggested:* treat image-comic uploads as folder uploads and store exactly the path `get_final_folder_path` would compute, so a later scan skips the row instead of duplicating it.
- **Who may upload — any authenticated user, or admins only?** Registration is admin-only (see [Authentication, Users & Sessions](03-auth-users-and-sessions.md)), but §4 says nothing about upload permission. The rename email going to "the uploader"'s `users.email` implies non-admin uploaders exist. Options: any authenticated user via the `AuthSession` extractor ([api_caller.rs:80-95](../../src/routes/api_caller.rs#L80)); admin-only via `AdminSession` ([api_caller.rs:60-78](../../src/routes/api_caller.rs#L60)). *Suggested:* any authenticated user, with `AdminSession` available if you'd rather keep write access narrow at first.
- **Write-side path containment is unclaimed.** [Operations & Security](12-operations-and-security.md) tracks path-traversal protection for *served* paths and notes it "becomes real with uploads §4" — but the write side is the sharper edge, since a user-supplied series name or title becomes a path segment. *Suggested:* sanitize the segment, canonicalize the resolved path, and assert it is inside the chosen scannable root before writing — which is another reason the scannable-directories table (Tier 2) should land first.

## Tasks

### Series resolution & target folder

*These four apply to both the video-series field and the book/comic-series field.*

- ⬜ **Auto-complete** against existing series **[BE][FE]** (needs a series-lookup/search endpoint)
- ⬜ **Create a new series inline** — if the typed name isn't found, offer to create it **[BE][FE]**
- ⬜ On new-series creation, **ask where it should live** (which scannable root) **[FE]** — depends on the scannable-directories table (§1, Tier 2)
- ⬜ Backend then **creates the folder in the correct structure + location and names everything per the convention** **[BE]**

### Single-file upload & post-upload pipeline

- ⬜ Upload endpoint that writes the file to the correct series folder on disk **[BE]**
- ⬜ Post-upload pipeline: create DB entry + extract cover + hash **[BE]** (reuse the scanner pipeline)
- ⬜ "Where to store this + metadata" dialog on upload **[FE]**
- ⬜ Drag a file into a series view → triggers upload **[FE]**

### Bulk upload — global defaults + per-file override

- ⬜ Accept multiple files and ingest them **sequentially** (one after the other) **[BE][FE]**
- ⬜ **Global fields at the top** of the batch — series/title, season, starting-episode (books: series, volume, starting-chapter). Each is **optional**. **[FE]**
- ⬜ **Per-file rows**, each individually editable to override the globals **[FE]**
- ⬜ **Per-field resolution cascade** (highest wins): **per-file manual edit → global field (only if filled) → value parsed from the filename → empty** **[BE][FE]** — an empty global falls through to the filename for that field; a filled global season + starting-episode overrides and auto-increments across the batch, files ordered by **natural sort** (`ep2` before `ep10`)
- ⬜ **Mandatory preview/confirm step** before anything touches disk — show each file's *resolved* season/episode/title **and** target path/name so a wrong global increment or a naming mismatch is caught and fixed inline **[FE]**

### Collision & storage safety policies

- ⬜ **Collision policy (DECIDED): auto-rename** when a resolved name already exists on disk, and **notify the uploader by email** that the file was renamed **[BE]** — sends to the user's `users.email`; rename scheme is a **timestamp** suffix that must still parse cleanly back through the scanner's filename regex
- ⬜ **Insufficient-storage policy (DECIDED): pre-flight free-space check** — if the drive can't hold the file, **drop the upload** (don't write a partial file) and **notify the user by email** **[BE]** — check *before* writing so nothing half-lands; in a bulk batch, drop the offending file and report it, the rest of the batch is unaffected (sequential ingest)

## Done when

- [ ] Posting a single file with a series name and volume lands it under that series' folder, creates exactly one `books` row, writes a cover into `cover_images/`, stores a BLAKE3 `file_hash` — and a subsequent `POST /api/v1/scan_all_directories` adds **no** duplicate row.
- [ ] A batch with a global season + starting-episode and no per-file edits produces a preview in which `ep2` is numbered before `ep10`, and **nothing** exists on disk until the confirm call is made.
- [ ] With a global field left empty, the resolved value for that field comes from the filename; with the same global filled *and* a per-file override set, the per-file value wins.
- [ ] Uploading a file whose resolved name already exists produces a timestamp-suffixed name on disk, an email to the uploader's `users.email`, and a filename that `get_final_file_name` still parses back to the same volume/chapter/page numbers and the same series title.
- [ ] A file larger than the free space on the target drive is rejected before any bytes are written (nothing on disk, no `books` row), the uploader is emailed, and the remaining files in the same batch still ingest.
- [ ] Typing an unknown series name offers inline creation; confirming it creates the `series` row, the `library_elements` link, and the folder under the chosen scannable root.

## Implementation notes

**The read half of the round-trip spec, quoted from the code.** These three regexes ([folder_scanner.rs:22-29](../../src/scanner/folder_scanner.rs#L22)) are the whole of what the scanner can recover from a filename:

```rust
static VOLUME_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)\b(?:v|vol|volume)[ _]?(\d+)\b").unwrap());

static CHAPTER_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)\b(?:c|chap|chapter)[ _]?(\d+)\b").unwrap());

static PAGE_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)\b(?:p|pa|page)[ _]?(\d+)\b").unwrap());
```

They are applied to `path.file_stem()`, the first capture of each is parsed as `i64`, and the title is whatever is left ([folder_scanner.rs:239-269](../../src/scanner/folder_scanner.rs#L239)):

```rust
let title = cleaned
    .replace(['-', '_'], " ")
    .split_whitespace()
    .collect::<Vec<_>>()
    .join(" ");
```

What the uploader must therefore respect:

- The separator between keyword and number can only be a single space, a single underscore, or nothing (`[ _]?`). `vol.3`, `v-3` and `vol  3` do **not** match.
- `\b` word boundaries mean the number must not run into adjacent letters, and the single-letter aliases `v` / `c` / `p` are live — a stray standalone `p 12` in a title becomes a page number.
- `(\d+)` is integer-only: no zero-padding semantics, no decimals. `c01.5` parses as chapter `1`.
- The title is the stem with the three matched fragments deleted, `-` and `_` turned into spaces, and whitespace collapsed. **Nothing else is stripped** — no brackets, no tags, and no timestamps. Any suffix the rename scheme adds lands in the parsed title.
- Series names are stored lowercased ([folder_scanner.rs:322](../../src/scanner/folder_scanner.rs#L322)), while `books.title` keeps its casing — so a title that differs only in case still resolves to the same series.
- The extension must be lowercase and one of `pdf`, `epub`, `cbz`, `jpg`, `jpeg`, `png` — the match is case-sensitive ([folder_scanner.rs:278-283](../../src/scanner/folder_scanner.rs#L278)), so an uploaded `.PDF` is invisible to a re-scan.
- For image comics the scanner stores the **parent directory** as `file_path`, not the image file ([folder_scanner.rs:218-226](../../src/scanner/folder_scanner.rs#L218)) — one row per folder of pages.

**Multipart is not enabled yet.** `axum = "0.8.9"` is pulled with default features ([Cargo.toml:7](../../Cargo.toml#L7)); the `Multipart` extractor lives behind `features = ["multipart"]`. Also note axum's `DefaultBodyLimit` is 2 MB and applies to multipart too — it must be raised or disabled *on the upload route specifically*, not globally. `tower-http` is already pulled with `features = ["full"]` ([Cargo.toml:40](../../Cargo.toml#L40)), so `RequestBodyLimitLayer` is available for a deliberate, larger cap. This will be the router's first middleware of any kind — there is no `.layer(...)` in [api_routes.rs:45-70](../../src/routes/api_routes.rs#L45) today, and the cookie layer the auth extractors need is missing for the same reason.

**Rust shape — stream fields, don't buffer them.** `Multipart` hands you fields one at a time; `field.bytes().await` reads the *whole* file into memory, which is wrong for a 400 MB CBZ. The shape you want is a loop — `while let Some(chunk) = field.chunk().await? { file.write_all(&chunk).await?; }` — writing into a temp file, then `fs::rename` into the final path once the free-space check and collision resolution have both passed. `rename` within the same filesystem is atomic, which is how you guarantee "no partial file ever appears at the final path"; a rename across filesystems is a copy, so keep the temp file on the target drive.

**Pre-flight free-space check needs a new dependency.** Nothing in the tree can query filesystem free space today; `fs2::available_space` (small, sync) or `sysinfo` would be the additions — *a suggestion, not a decision*. Query the filesystem containing the **target** directory, not the process CWD, and compare against the declared `Content-Length` / field size before opening the temp file.

**Sequential means sequential — don't copy the scanner's spawn pattern.** `scan_folder` spawns a task per file and never joins them ([folder_scanner.rs:77-87](../../src/scanner/folder_scanner.rs#L77)), which is exactly why `POST /api/v1/scan_all_directories` replies `"Directory scan complete"` before the scan has finished ([api_caller.rs:307-315](../../src/routes/api_caller.rs#L307)). Await each file's ingest inside the handler's loop so the response reflects the real outcome and the per-file report is accurate.

**Cover extraction is blocking work.** pdfium, `zip` and `image` are all synchronous and CPU-heavy; the scanner already wraps its per-file work in `tokio::task::spawn_blocking` ([folder_scanner.rs:80-82](../../src/scanner/folder_scanner.rs#L80)). Do the same from the upload handler rather than calling `get_cover_image` directly in the async path. The EPUB cover path is still untested ([Library & Directory Management](01-library-and-directory-management.md)), so an EPUB upload may produce a wrong or missing cover.

**Panic hazard when reusing the insert.** `BookMetadata::insert` does `convert_system_time_to_unix_time(self.last_modified.unwrap())` at [insert.rs:49](../../src/database_related_scripts/insert.rs#L49) — an unconditional `unwrap`. An upload path must always populate `last_modified` (the freshly written file's `modified()`), or fix that `unwrap` first, or a single odd filesystem answer panics the ingest.

**Reuse `create_new_email`, and inherit none of its bugs.** [`create_new_email`](../../src/routes/email_helper.rs#L7) + `EmailInformation` ([email_helper.rs:43-48](../../src/routes/email_helper.rs#L43)) already work over `mail-send` — no need for `lettre`. Two defects to avoid carrying over: the username is passed as a second *recipient address* rather than a display name (`.to(vec![username, email])`, [email_helper.rs:15](../../src/routes/email_helper.rs#L15)), and sending happens inline in the request path with a fresh TCP + TLS + AUTH per message, so an unreachable SMTP host stalls the HTTP response (as it does in `sign_up`, [api_caller.rs:346-391](../../src/routes/api_caller.rs#L346)). For a bulk batch, one summary email after the batch and/or a send spawned off the response path would avoid both — *a suggestion, not a decision; the roadmap only says "notify the uploader by email"*.

**Derive the uploader from the session, never from the body.** The rename notice needs `users.email`, so the handler needs a user id — take it from the `AuthSession` extractor ([api_caller.rs:80-95](../../src/routes/api_caller.rs#L80)), which is the single rule that stops one user acting as another. Be aware that extractor cannot succeed today: no `CookieManagerLayer` is installed, and `Sessions::from_row` reads a mistyped column name `"last§"` ([db_from_row.rs:126](../../src/database_related_scripts/db_from_row.rs#L126)), so every session read fails.

**Store exactly the path the scanner would compute.** `books.file_path` is `UNIQUE` ([0001_initial_schema.sql:11](../../migrations/0001_initial_schema.sql#L11)) and the scanner skips any file whose path is already a row ([folder_scanner.rs:119-122](../../src/scanner/folder_scanner.rs#L119)) — that skip is what makes the round-trip work. If the uploader stores a path the scanner would normalise differently (the image-comic parent-directory rule at [folder_scanner.rs:218-226](../../src/scanner/folder_scanner.rs#L218) is the case to watch), the next scan inserts a second row for the same content. Write extensions lowercased for the same reason ([folder_scanner.rs:278-283](../../src/scanner/folder_scanner.rs#L278)).

**Series lookup should go through `Search`, not `get_id_from_table`.** `BookSeriesMetadata` implements `Search` ([db_search.rs:102-105](../../src/database_related_scripts/db_search.rs#L102)), which supports multi-row matching — the right basis for auto-complete. `Database::get_id_from_table` is explicitly marked `//should not be used` ([db.rs:355](../../src/database_related_scripts/db.rs#L355)) even though the scanner still calls it. Inline creation can reuse `create_series_entry` + `link_series_to_default_library` ([folder_scanner.rs:316-347](../../src/scanner/folder_scanner.rs#L316)); do both the `series` insert and the `library_elements` insert inside one `setup_new_transaction` so a failed link can't leave an orphan series.

**Natural sort is already available.** `natord = "1.0.9"` ([Cargo.toml:34](../../Cargo.toml#L34)) and `natord::compare` is already used for exactly this kind of ordering in [cover_image_retriever.rs:143-171](../../src/scanner/cover_image_retriever.rs#L143). Sort the batch with `compare` before assigning auto-incremented episode/chapter numbers — no new crate needed.

**The notification emails localize for free.** Per the i18n decision, the catalog lives in the backend, so rename and out-of-space emails can pull their text from it once the user's locale is stored — no separate email catalog. The verification email already carries the placeholder TODO at [auth.rs:171](../../src/routes/auth.rs#L171). See [Internationalization (i18n)](13-internationalization.md).

## Related

- [Library & Directory Management](01-library-and-directory-management.md) — owns the scanner pipeline this reuses, the filename parser that defines the round-trip spec, and the scannable-directories table that says where a new series may live
- [Authentication, Users & Sessions](03-auth-users-and-sessions.md) — the upload endpoint must be authenticated, and the uploader's `users.email` is where the rename / out-of-space notices go
- [Email & Notifications](14-email-and-notifications.md) — the SMTP dependency this section reintroduces, and the sender to reuse
- [Operations & Security](12-operations-and-security.md) — path-traversal protection becomes load-bearing here, on both the write and read sides
- [Backend Admin Web View](06-admin-web-view.md) — next in Tier 4, and another client of the same upload API
- [Browsing, Search & Discovery](11-browsing-search-and-discovery.md) — the series-lookup/search endpoint the auto-complete needs
- [Internationalization (i18n)](13-internationalization.md) — the backend-owned catalog that localizes the notification emails
- [Video Library (Phase 2)](20-video.md) — where the video field sets (`movie` / `series` / `original`, season/episode) actually become reachable
- [Cross-Cutting Backend Polish](07-cross-cutting-backend-polish.md) — the compile errors and the axum 0.8 route-syntax panic that block exercising any of this
