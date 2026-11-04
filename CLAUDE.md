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
└── features/             ← one file per feature; 99-bugfixes.md holds bugs that fit no feature
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

`Tier` is where the feature sits in the build order — `—` until one is assigned. It lives in
this table only. The numbered tiers run in order: finish every feature in one before starting
the next. `F0` / `F1` are the frontend track, `cross-cutting` rides along with whatever touches
it, `Phase 2` is parked, `optional` waits for an accept, and `ongoing` never holds anything up.

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
