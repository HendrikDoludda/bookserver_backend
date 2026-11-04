# CLAUDE.md

Project guidance for Claude Code.

Personal **book / manga / comic home server** backend: scans directories of books, stores
metadata in SQLite, extracts cover images, and exposes an HTTP API (Axum) for a frontend.

## How we work

The developer is new to Rust and to writing security-sensitive code, and this is a learning
project as much as a product. These three rules come before everything else in this file.

1. **Instructions → plan → approval → build.** The developer gives instructions; Claude answers
   with a plan that says why each step is needed ([Plan before you build](#plan-before-you-build)),
   then stops. Nothing is edited until the developer approves.
2. **Explain every change in detail.** After each change: what changed, why it is done this way,
   and the Rust or library concept behind it wherever it is new to the developer. The
   explanation goes in the chat, not in code comments ([Code comments](#code-comments)).
3. **Second-guess the developer.** Check every instruction against the code, the docs and good
   practice before acting on it. If it looks like a mistake — a bug, a security hole, a
   contradiction with an earlier decision, a simpler way — say so and why, before doing
   anything. The developer decides; then follow the decision without re-arguing it.

## Docs migration — pending

The docs are still in the old format: [ROADMAP.md](ROADMAP.md) (build order in tiers) and the
feature docs in [docs/features/](docs/features/) (eight sections, ⬜ 🟡 ✅ markers), indexed in
[docs/README.md](docs/README.md). Moving them into the [Documentation](#documentation) layout
below is a task of its own — plan it before starting. Until it is done:

- Read the old docs for what to build and what was decided.
- Keep them current in their own format (conventions in [docs/README.md](docs/README.md)) —
  never mix the two formats in one doc.
- The [Project map](#project-map-until-the-docs-migration) at the bottom of this file stands in
  for `ARCHITECTURE.md` and `commands.md`.

Delete this section and the project map once the migration lands.

## Documentation

All project knowledge lives in `docs/`. One fact, one file — if you are about
to repeat something, link to it instead.

```
docs/
├── project-state.md      ← what is live for users, one line per feature
├── build-checklist.md    ← every feature: name, tier, state, link. Nothing else
├── ARCHITECTURE.md       ← project-wide code conventions, one line each
├── commands.md           ← project-specific commands
├── run-deploy/           ← one file per platform: commands, then run and deploy steps
├── setup.md              ← repositories, git permissions, default branch
├── <nn>-<slug>-found.md  ← out-of-scope findings, one per feature in progress (see Scope)
├── <nn>-<slug>-checks.md ← the developer's manual checks, one per feature in progress (see Verification gate)
└── features/             ← one file per feature
```

There is no built/unbuilt folder split: a feature's state lives in its doc's `State:` line and
in the checklist row, nowhere else.

### States

The only four. Used in feature docs and the checklist, defined nowhere else.

`blocked` · `not started` · `in progress` · `completed`

### `build-checklist.md`

One row per feature. No description, no rationale, no sub-items, no dates, no counts.

```markdown
# Build checklist

| Feature | Tier | State | Doc |
|---|---|---|---|
| <name> | <tier or phase> | <state> | [<nn>-<slug>](features/<nn>-<slug>.md) |
```

`Tier` is the build tier or phase the feature belongs to — `—` until one is assigned. It lives in
this table only. Finish every feature in a tier before starting the next.

Completed rows stay until the developer says that work is released to users — then they go,
together with the feature doc (see [Deleting](#deleting)).

### `features/<nn>-<slug>.md`

What to build, what is unknown, what is undecided. Everything else belongs in the code, the
commit, or the chat.

```markdown
# <Feature name>

State: <state>

**Blocked:** <why, 1–2 lines — what has to happen before work can continue.>

<1–3 sentences: what it does and why. Add a Mermaid diagram only if the flow is not obvious
from the checklist.>

## Build
- [ ] <n> h · <task — one change, specific enough to implement without asking>
- [ ] <n> h · <task>

## Open decisions
- [ ] <question> — blocks: <which build task>

## Missing information
- [ ] <what is unknown> — answered by: <who or what>
```

Drop empty sections, and the `Blocked` line the moment it is not. No implementation history, no
as-built narrative — the code and the diff are the record.

- **One task, one checkbox.** A task is one change that can be done and verified on its own; if
  it has parts that could be done separately, it is several tasks.
- **Estimate first.** Every task starts with its estimate in hours — `? h` until there is one.
  The feature's total is the sum; it is not written down.
- **Tests are part of the task.** A task is done when its code and its automated tests are
  ([Tests](#tests)). Device and manual checks are tasks of their own.
- **Tick a task** once it is verified ([Verification gate](#verification-gate)).

**Creating one:** when the developer hands over a task or a list of tasks with no context, ask
whether they should become features before writing anything — they may be one feature, several,
or one-off work that belongs nowhere. If the developer says to add them, add them without
asking.

### Deleting

Anything that no longer meets these rules is deleted — not archived, not struck through, not
kept "for reference".

- An **answered** open decision → fold the answer into `Build` — either a new task, or a
  clarification of an existing one — then delete the question. The answer survives only as
  something to build.
- A **supplied** missing-information item → same: it lands in `Build` as a new or a clarified
  task, and the item goes.
- A **released** feature — live to users → delete the doc and its checklist row, and rewrite
  `project-state.md`. What users have is described there, nowhere else.
- A **cancelled** feature (dropped, approach abandoned) → delete the doc and its checklist row.
- Anything **irrelevant, or contradicted by the code** → delete it. The code is right; correct
  the doc in that pass.

### `project-state.md`

**One line per feature that is live for users**, plus one line for the next milestone. Rewrite
it at every release — never append to it.

This is the answer when anyone — the developer, or a recruiter looking at the repo — asks what
the project does today: the single source of truth for what users actually have. What is still
coming is the checklist's job, not this file's.

### `ARCHITECTURE.md`

The code rules that hold across the whole project: where things live, how tests are built, the
patterns new code follows, the hosting setup. One line each, grouped under a heading per area.
Rules only — why a rule exists goes in a code comment where it applies. Versions belong in
[Stack](#stack); a mistake that already cost us goes in [Rules learned](#rules-learned).

### `commands.md`

One table: command · what it does · destructive (yes/no). Commands used **while developing**
(tests, generators, maintenance) — nothing else. Running and deploying live in `run-deploy/`;
no command appears in both.

### `run-deploy/<platform>.md`

One file per platform or environment (local, Docker, per OS). Each command is written **once**,
at the top, and the steps link to it by key.

```markdown
# <Platform>

## Commands
| Key | Command |
|---|---|
| `<key>` | `<command>` |

## Run
1. <Step> — [`<key>`](#commands)
2. <Step> — [`<key>`](#commands). <One sentence, only where the step needs it.>

## Deploy
1. <Step> — [`<key>`](#commands)
```

The steps are the whole document — first-time setup is simply the first few of them. A step
gets a sentence only when the command does not already say it. Nothing else in the file.

## Writing rules

- **Short.** Fewest words that still let someone implement the thing. Cut every sentence that
  does not change what the reader does.
- **No duplication.** A fact appears in one file; other files link to it.
- **Rationale lives in the code.** Why a technical choice was made goes in a short comment where
  it applies ([Code comments](#code-comments)), never in a doc. The reasons behind product
  decisions are kept outside the repository. What the code used to do goes in the commit. Docs
  explain features and project state.
- **Docs are part of the work.** Update the checklist row and feature doc in the same pass as
  the code — at the start of a task and at the end.
- **A wrong doc is corrected at once.** When the code contradicts a doc, make the doc match the
  code right away, even mid-task — this overrides [Scope](#scope). If the code is what's broken,
  that part still goes to the found file.

## Code comments

Extend this section as the project teaches us more. Default is **no comment**.

1. **Why, not what. 1–2 sentences, hard maximum.** Concise, plain language.
2. **Only when the reason is not visible in the code:** a constraint from outside it (an old app
   version, a store rule, an external API) · a choice that looks wrong but is deliberate · a
   magic value — why *that* value and not another.
3. **Technical reasons only, drawn from the code.** Never a product decision — why the product
   works the way it does is kept outside the repository. What the code used to do belongs in
   the commit.
4. **If two sentences are not enough, stop and ask** how to write it — or whether the code
   should be restructured instead.

## Scope

Do what was asked, nothing adjacent. Add rather than alter: if existing behaviour has to change
for the new thing to work, say so in the plan ([Plan before you build](#plan-before-you-build)).

Something unrelated looks broken, risky or ugly — **do not fix it**. Append one line to
`docs/<nn>-<slug>-found.md`, named after the feature doc (file · what · why it matters), and
carry on — one file per feature in progress, so findings never mix.

**Clear it at every stopping point** — a task finished, or you are about to wait for the
developer. Go through the entries one by one and ask what to do with each:

`add to <feature doc>` (an existing one, or a new one) · `implement immediately` · `ignore`

Say plainly which entries are **security or stability** problems when asking — those are not a
matter of taste. Remove each entry once the developer has decided; delete the file when empty.

## Before you edit

Read the relevant code, tests, configuration and feature doc before changing them. Do not infer
architecture from filenames or assume a conventional implementation exists. For a change with
non-trivial behaviour, identify the current behaviour and the intended behaviour before editing.

## Conflicts

What to build, highest first: the developer's instruction in this task · this file · the
feature doc · other docs. Before following an instruction that overrides this file or a doc,
name the rule it overrides; afterwards correct the doc. Nothing decides it → ask, or add it
under Missing information; never assume what the code or docs do not support. What the code
does today is not on this list — the code is the truth for that. Git follows
[its own order](#git).

## Tests

Every task comes with automated tests that prove it works — thorough: the real paths, the edge
cases, the failure cases. Not a smoke test. They are written with the task, not after the
feature. No tests exist yet: the first task that needs one also sets up the test layout, and
plans it like any other change.

## Plan before you build

**Every change is planned and approved before editing — no exemptions.** Scale the plan to the
change: a one-line fix gets two lines; a schema change gets the full treatment.

For a change that adds a migration or schema change · adds or alters an API endpoint · adds a
service or model · changes a rule the app enforces (what is allowed, shown, blocked,
generated): enter plan mode, read the actual code (not guesses), name the files and migrations
touched, and list unknowns instead of assuming them.

Every plan says why each step is needed. Present it and **stop** until the developer confirms.
Approval of one plan is not approval of the next.

## Verification gate

Two checks, two owners.

- **Claude ticks a task** only after verifying it against the working tree — reading the code
  and running its tests — never on someone's say-so. If you cannot (unmerged branch, cannot run
  it), the task stays open and the feature is `blocked` — say why on its `Blocked` line.
- **The developer tests by hand.** Write what the developer should check themselves — in the
  browser, with real mail, against a real `.epub` — into `docs/<nn>-<slug>-checks.md`, one
  checkbox per check, and tick each as the developer reports it. Before starting the next task,
  say if items are still open, then carry on. Checks that cannot be done yet move to the
  feature doc as check tasks; delete the file when nothing is left in it.

A feature is `completed` only when every task is ticked and no checks file is left for it.

## Destructive commands — warn first, every time

Anything that drops rows, files or history — wiping `data/databases/app_data.sqlite`, clearing
`cover_images/`, mass deletes, resets, force pushes — gets stated plainly and confirmed before
running. Not as a step inside a larger task, not to get a clean slate for a test. Flag them in
`commands.md`.

## Git

`docs/setup.md` lists which of **commit · push · branch · checkout** are
Claude's, and it outranks everything: first `setup.md`, then the developer's request, then your
own judgement. Within what it lists the developer can still say no; outside it nobody can say
yes. Asked for an unlisted action — even explicitly — refuse, and say the `Git:` line in
`setup.md` is where that changes. No `setup.md` means no git at all.

If **branch** is Claude's: before starting a feature, ask whether to branch from the current
branch or from the default branch named in `setup.md`.

If **commit** is not Claude's: say when the work reaches a good commit point — a step done and its
tests green — and what it would contain. The developer decides.

## Stack

Rust 2021 edition (rustc 1.96) · Axum 0.8 on Tokio · SQLite via rusqlite 0.40 with an r2d2
pool · Argon2 passwords, totp-rs, mail-send (SMTP) · pdfium-render, zip, epub-parser and image
for covers. Code lives in `src/`, schema in `migrations/`. Commands go in `commands.md`.

### Rules learned

One line each. A rule that needs a paragraph is two rules.

**Only rules that hold everywhere in the project.** If something is true sometimes, or depends
on the situation, it is not a rule — ask the developer to sharpen it into one, or leave it out.

| Rule | Why it bites |
|---|---|
| Axum route params are `{param}`, never `:param` | Axum 0.8 panics while building the router, before the port opens |
| A column name in `Extract::COLUMNS` must match the `row.get("…")` in `FromRow` exactly | A mismatch compiles fine and fails only at runtime |
| Never commit `data/` | The SQLite file holds local file paths and user rows; git history had to be rewritten once |

## Project map (until the docs migration)

### Commands
- Type-check: `cargo check`
- Build: `cargo build`   ·   Run: `cargo run`
- Tests: `cargo test` (no tests yet — see [docs/features/07-cross-cutting-backend-polish.md](docs/features/07-cross-cutting-backend-polish.md))
- Runtime config via env vars: `PORT` (default 8080), `BOOK_DIRS` (`:`-separated), `API_KEY`,
  and `SMTP_HOST` / `SMTP_PORT` / `SMTP_USERNAME` / `SMTP_PASSWORD` / `SMTP_FROM` / `SMTP_FROM_NAME`

> The crate compiles and the server boots. Remaining `cargo clippy` warnings are confined to
> the in-progress auth/TOTP code. Defects that still break individual endpoints are listed
> under "Blocked right now" in [ROADMAP.md](ROADMAP.md).

### Entry & wiring
- `src/main.rs` — binary entry. Inits tracing, opens the DB, calls `start_server`. Returns `anyhow::Result`.
- `src/lib.rs` — module declarations + re-exports. Note `pub use data_models::models` and
  `pub use database_related_scripts::db`, which is why `crate::models::…` and `crate::db::…`
  resolve despite the nested directories.
- `src/config.rs` — env-var config readers (`get_port`, `get_books_dirs`, `get_api_key`,
  `get_email_config`) plus `set_up_config_file`, which generates a 256-bit signing key into
  `secrets.toml` on first run. **Nothing consumes that key yet** — see
  [docs/features/12-operations-and-security.md](docs/features/12-operations-and-security.md).

### HTTP layer — `src/routes/`
- `api_routes.rs` — the **route table** (`set_up_routes`) and `start_server`. Router state is `Arc<Database>`.
- `api_caller.rs` — the **handler functions** (what each endpoint does). Also holds the auth **extractors** (`AuthSession`, `AdminSession`, `AuthTOTPLoginSession`, `AuthRefreshSession`). Mix of implemented + stubbed; stubs note *why* they're blocked, and the TOTP handlers return 501. `ping_server` doubles as the liveness check and the placeholder for routes with no handler yet.
- `auth.rs` — real auth logic (~550 lines): user creation, email verification, login, password change/reset, Argon2 hashing, and the session + refresh token pair. **Not yet wired to routes.**
- `totp_management.rs` — comment-only stubs for the TOTP flows.
- `email_helper.rs` — `create_new_email`: builds and sends mail over SMTP via `mail-send`.

### Database layer — `src/database_related_scripts/`
- `db.rs` — the **`Database` struct** (r2d2 SQLite pool) and all DB operations: `get_entry`, `get_entries_batch`, `insert`, `update_value`, `remove_entry`, library/series queries, `setup_new_transaction`.
- `extract.rs` — **`Extract` trait**: read a row → metadata struct. Each type supplies `TABLE` / `COLUMNS` / `from_row`; gets `extract` (by id) + `extract_batch` (IN-list → `Vec<WithId<T>>`) as default methods.
- `insert.rs` — **`Insert` trait**: metadata struct → `INSERT`.
- `db_update.rs` — **`Update` trait**: update one column by id (plus a multi-column variant).
- `db_from_row.rs` — the **`FromRow` impls**: `rusqlite::Row` → metadata struct, by column name.
- `db_search.rs` — **`Search` trait**: find a row by arbitrary columns + values, `And`/`Or` separated. Also used to prove a value *doesn't* exist (uniqueness checks).
- `migrations.rs` — versioned migration runner; loads `migrations/*.sql` via `include_str!`. To add schema: new numbered `.sql` + register in the `MIGRATIONS` array; never edit an applied migration. *(Early dev exception: migration 0001 is still edited in place and the DB wiped.)*
- `mod.rs` — module declarations.

### Domain models & errors — `src/data_models/`
- `models.rs` — the domain structs & enums: `BookMetadata`, `BookSeriesMetadata`, `LibraryMetadata`, `UserMetadata`, `SeriesLibraryConnection`, `Sessions`, `TOTP`, `EmailVerification`, `RecoveryCodes`, `EmailConfig`; the `*DatabaseColumns` enums; `LibraryType` / `BookFormat` / `BookLanguage`; `DatabaseTypes`; `ColumnSelector`; `QuerySeparator` / `SelectionMethod`; **`WithId<T>`** (pairs row id + metadata, flattened in JSON); `ParsedName`, `FileExtractedMetadata`, `DatabaseEntry`.
- `authentication_model.rs` — the auth **request/response bodies** (`UserCreationRequest`, `LoginRequest`, `VerifyEmailRequest`, `ChangePasswordRequest`, `DeviceInformation`, `AuthResponse`, …).
- `totp_model.rs` / `information_retrieval_models.rs` — TOTP and library-request bodies. *(These don't derive `Deserialize` yet, so they can't back a `Json<T>` extractor.)*
- `src/error_types.rs` — 10 typed error enums per domain (`DatabaseError`, `FolderScannerError`, `CoverImageError`, `StreamReaderErrors`, `RequestErrors`, `RoutingErrors`, `AuthenticationError`, `EmailErrors`, `AppErrors`, `ApplicationSetUpErrors`). Only `DatabaseError` and `RequestErrors` implement Axum's `IntoResponse`.

### Scanner — `src/scanner/`
- `folder_scanner.rs` — walks configured dirs, parses filenames (regex for volume/chapter/page), inserts entries. Entry point: `scan_all_folders`.
- `cover_image_retriever.rs` — extracts a cover per format (PDF / CBZ / EPUB / image). Entry point: `get_cover_image`. (EPUB path untested.)
- `metadata_fetcher.rs` — **placeholder** (module doc only); online metadata fetching lives in [docs/features/09-metadata-enrichment.md](docs/features/09-metadata-enrichment.md).
- `mod.rs` — module declarations.

### Reading & misc
- `src/stream_reader.rs` — `streaming_file`: serves a file with HTTP range support via `tower-http` `ServeFile`.
- `src/utils.rs` — small helpers (`convert_file_path_to_blob`).

### Data & assets (runtime, not source)
- `migrations/0001_initial_schema.sql` — 10 tables: `books`, `library`, `library_elements`
  (junction: composite PK `(library_id, series_id)`, **no `id`**), `series`, `users`,
  `email_verification`, `reset_password_requests`, `sessions`, `recovery_codes`, `totp`.
- `data/databases/app_data.sqlite` — the SQLite database. Created on first run; git-ignored.
- `cover_images/` — extracted cover images. Git-ignored except the `example-cover.png` fallback.

### Key patterns & conventions
- **DB traits, one per operation:** `Extract` (read), `Insert` (write), `Update` (modify), `Search` (find by column), `FromRow` (row → struct). Each metadata type implements the relevant ones; `Database` methods wrap them with a pooled connection.
- **Two flavours of every `Database` method:** the plain one takes a connection from the pool; the `*_with_connection` variant takes a `&Connection` so several operations can share one transaction (`setup_new_transaction`).
- **`WithId<T>`** is how list endpoints return id + metadata together (the metadata structs don't store their own id).
- **Errors:** `thiserror` enums per domain. `DatabaseError::into_response` is the reference shape: `EntryNotFound` → 404, everything else → 500 with the detail logged server-side and a generic body to the client.
- **SQL safety:** values are always bound parameters (`?1`, `params!`, `params_from_iter`). Table/column names are only ever interpolated from trusted enums/consts, never from request input.
- **Handlers** use Axum extractors: `State<Arc<Database>>`, `Path<i64>`, `Json<T>`, `Request<Body>`.
- **Junction table** `library_elements` deliberately does *not* implement `Extract` (no `id` column); it's queried by FK via dedicated `Database` methods.
