# Collaborative Documents

> Editable, collaboratively-synced text documents on the same server, built as an ordered list of id-anchored nodes with append-only ops that the backend serializes into a single total order.

**Status:** ⬜ not started · **Phase:** 2 · **Tier:** Parked — out of focus · **Tags:** [BE] [DB] [FE]
**Depends on:** [03 Authentication, Users & Sessions](03-auth-users-and-sessions.md) (edits are attributed to a user)
**Blocks:** Nothing
**Roadmap:** [Build order](../../ROADMAP.md#priority-order-build-sequence)

## Why

The long-term ambition is a *complete* home server, not just a book reader — documents you can actually edit in the app, from more than one device, without a second service to run. The design is already worked out (server-serialized, id-anchored append-only ops) precisely so that the hard-looking part — concurrent editing — reduces to something a single central backend can do correctly. Phase 2 is explicitly parked while the Phase 1 backend ladder is built; this doc exists so the reasoning behind the decided design isn't lost in the meantime.

## Current state

Nothing exists. There is no document, node, or op code, table, model, route, or dependency anywhere in the tree.

The only thing in the codebase that so much as mentions documents:

- [models.rs:56](../../src/data_models/models.rs#L56) — `LibraryType::Documents`, one variant of the library-type enum, with its string form `"documents"` at [models.rs:66](../../src/data_models/models.rs#L66) and [models.rs:75](../../src/data_models/models.rs#L75). It is a label for `library.library_type` and carries no document behaviour. It is also never *assigned* today: the scanner hardcodes `LibraryType::Books` when it creates a library ([folder_scanner.rs:363](../../src/scanner/folder_scanner.rs#L363)), and there is no working create-library endpoint yet (`insert_entry` returns a hollow 200, [api_caller.rs:333](../../src/routes/api_caller.rs#L333) — see [Library & Directory Management](01-library-and-directory-management.md)).

What is *absent*, specifically:

- No tables. [0001_initial_schema.sql](../../migrations/0001_initial_schema.sql#L2) creates exactly ten: `books`, `library`, `library_elements`, `series`, `users`, `email_verification`, `reset_password_requests`, `sessions`, `recovery_codes`, `totp`. No `documents`, no node table, no op log.
- No `DatabaseTypes` variant for documents ([models.rs:17](../../src/data_models/models.rs#L17)), so nothing document-shaped can be addressed by the generic `Database` helpers.
- No routes. The route table ([api_routes.rs:45](../../src/routes/api_routes.rs#L45)) has no document endpoint — not even a skeleton wired to the `ping_server` placeholder, unlike `/search` or `/scan/status`.
- No real-time transport. `axum = "0.8.9"` ([Cargo.toml:7](../../Cargo.toml#L7)) is pulled with default features, which do **not** include `ws`; there is no `WebSocketUpgrade` or `Sse` usage in `src/`.
- No CRDT dependency (`yrs` / `automerge` are not in [Cargo.toml](../../Cargo.toml)).

## Decisions

- **Data model: a document is an ordered list of nodes (a linked list).** Each node is a sentence/block with a **unique id** and references to its **neighbour ids**. Rationale: it gives every piece of the document a stable identity that survives the document changing around it.

- **Edits anchor to neighbour ids, not character positions.** An edit says "after node X" / "before node Y", never "at offset 412". Rationale: the document can shift underneath an in-flight edit without breaking that edit's anchor — which is the whole reason positions are avoided.

- **Edits are small append-only ops (deltas).** Rationale: the same op is the unit of *local storage*, *wire payload*, and *sync* — one representation, no translation between layers. It also gives **history and undo for free**, because the op log *is* the history.

- **Unconfirmed ops queue locally and flush on reconnect.** Offline-first, and it ties into offline reading ([Reading & Read State](02-reading-and-read-state.md)).

- **Concurrency: server-serialized, NOT a CRDT.** All edits funnel through the one backend and are processed one at a time, so the server **is** the total order (by arrival) and there is nothing to reconcile. This is the simple, *correct* choice **because there is a central backend**: CRDTs exist to solve the **decentralised / no-arbiter** case, which this project does not have.

- **Tombstone deleted nodes.** A deleted node is marked dead but keeps its id, so edits anchored to it still resolve. This is the first of the two edges that survive server-serialization.

- **Same-node concurrent edits are last-writer-wins.** The server *orders* but does not *merge*, so two people editing the same node at once means the later arrival wins. This is the second surviving edge, and it is the one genuinely hard part the central-server approach cannot handle. Auto-merging the same node is the **only** thing that would need a CRDT; everything else is achievable with serialized writes.

- **CRDT only as an escape hatch.** If true offline-concurrent / same-node auto-merge is ever actually needed, reach for a CRDT lib (`yrs` / `automerge`) — or hand-roll WOOT/RGA-style as a learning exercise. Not before.

- **Real-time delivery via WebSocket or SSE.** Clients with the doc open get pushed updates rather than polling.

- **v1 is markdown/plaintext only; Office formats are a separate, later effort.** docx/xlsx/pptx are zipped XML trees, not text, so deltas become XML-tree mutations rather than text ops — meaningfully more work. Start collaboration on markdown/plaintext. Office-format fidelity is one of the two genuinely hard parts that remain after this approach (the other being same-node auto-merge, above).

- **Office formats — intermediate-model idea (explicitly NOT committed).** Rather than editing the zip in place, reuse the v1 op-store as a **neutral intermediate model**. XML-tree surgery is confined to two boundaries: **import** (docx → node list, assigning ids to paragraphs/runs/cells) and **materialize/export** (replay ops → XML → rezip, **only on explicit save**). Inserted assets such as images live in the op as a blob and only get wired into `word/media/` + rels + `<w:drawing>` at export. **Tradeoff (the warning to keep):** import/export is **lossy** for any formatting the importer doesn't model — "edit a docx" ≠ "round-trip a docx perfectly". A fidelity-preserving overlay (anchor ids onto untouched original XML) is the later escape hatch if that lossiness turns out to matter.

## Open questions

- **How does a document relate to `library` / `series` / `books`?** The decided design covers node ordering and ops but never says where a document *lives*. Options: a dedicated `documents` table with its own node/op tables (clean separation, but a second content hierarchy to browse and permission); or reuse `books` rows with `LibraryType::Documents` on the containing library (reuses browsing, covers and search, but `books` is built around an immutable file on disk with a hash and a page count, which a live document isn't). **Suggested:** a dedicated `documents` table plus node and op tables, with `LibraryType::Documents` kept as nothing more than the library's label.

- **Real-time transport: WebSocket or SSE?** The roadmap lists both without picking. SSE (`axum::response::Sse`) is one-way server→client, plain HTTP, needs no extra axum feature, and survives proxies easily — ops would still go *up* as ordinary POSTs. WebSocket (`features = ["ws"]`) is bidirectional so ops travel on the same connection, at the cost of reconnect/heartbeat handling. **Suggested:** SSE for fan-out plus normal POSTs for submitting ops, and only move to WebSocket if POST-per-edit latency actually hurts.

- **Node granularity — sentence or block?** The design says "sentences/blocks" without choosing. Sentence-level nodes narrow the last-writer-wins collision window (two people in the same paragraph collide less often) but multiply node count, ids and ops. Block/paragraph-level means far fewer ops but coarser collisions. **Suggested:** paragraph/block-level for v1, since last-writer-wins is accepted anyway and fewer ids is less to get wrong.

- **Op-log retention.** History comes free from the append-only log, but the log grows forever. Options: keep everything (simplest, unbounded); or snapshot the node list periodically and prune ops behind the snapshot (bounded, but now there are two sources of truth to keep consistent). **Suggested:** keep everything for v1 — this is a personal server with small documents — and revisit only if the log actually becomes a problem.

- **Who may open and edit a document?** The design is entirely about *ordering* and never addresses permissions or ownership, while every other per-user feature waits on auth. Options: owner-only (trivial, but then it isn't collaborative); all authenticated users (matches the few-user home-server threat model in [Operations & Security](12-operations-and-security.md)); or per-document sharing (most work). **Suggested:** all authenticated users for v1, with the op recording *which* user wrote it so attribution and history are there if sharing is added later.

- **Office formats: intermediate model or fidelity-preserving overlay?** The roadmap marks the intermediate-model idea as not committed and names the overlay as the later escape hatch. Intermediate model: one op representation for everything, but lossy on unmodelled formatting. Overlay: preserves the original XML untouched, but is substantially more complex. **Suggested:** don't decide until markdown v1 has shipped; if Office ever happens, start with the intermediate model and accept the documented lossiness.

- **If the CRDT escape hatch is ever taken, which route?** `yrs`, `automerge`, or hand-rolled WOOT/RGA as a learning exercise. **Suggested:** leave unchosen — it only becomes a real question if same-node auto-merge turns out to be needed, and picking now would be deciding on a problem that hasn't happened.

## Tasks

- ⬜ v1: serialized sync on **markdown/plaintext** — node list + unique ids + neighbour refs, small ops, autosave, history, offline queue **[BE][FE]**
- ⬜ Tombstones for deleted nodes so anchored edits still resolve **[BE]**
- ⬜ Real-time delivery — push updates to clients with the doc open (WebSocket / SSE) **[BE][FE]**
- ⬜ Create new documents in-app **[FE][BE]**
- ⬜ Escape hatch (only if ever needed): true offline-concurrent / same-node auto-merge → a CRDT lib (`yrs` / `automerge`), or hand-rolled WOOT/RGA-style as a learning exercise **[BE][FE]**
- ⬜ Later: **Office formats** (docx/xlsx/pptx) — zipped XML trees, so deltas are XML-tree mutations, not text → meaningfully more work **[BE][FE]**
  - *Idea (not committed):* reuse the v1 op-store as a neutral intermediate model instead of editing the zip in place. Confine XML-tree surgery to two boundaries — **import** (docx → node list, assign ids to paragraphs/runs/cells) and **materialize/export** (replay ops → XML → rezip, only on explicit save). Inserted assets (images) live in the op as a blob and only get wired into `word/media/` + rels + `<w:drawing>` at export. Tradeoff: import/export is **lossy** for formatting the importer doesn't model — "edit a docx" ≠ "round-trip a docx perfectly". Fidelity-preserving overlay (anchor ids onto untouched original XML) is the later escape hatch.

## Done when

- [ ] Two clients with the same document open: an edit made in one appears in the other without a manual refresh.
- [ ] An op anchored to a node that another client deleted still applies — the tombstone resolves the anchor instead of the op being rejected or dropped.
- [ ] Two edits to the *same* node submitted at once leave the later-arriving value stored, and both clients end up showing that same value (last-writer-wins, no divergence).
- [ ] A client edited while disconnected: on reconnect its queued ops flush in order and the server's document matches the client's.
- [ ] Replaying the stored op log from empty reproduces the current document exactly, and stepping back one op yields the previous state (history/undo comes from the same representation).
- [ ] A document created in the app is readable through the API by a second client.

## Implementation notes

**Do not start this before the Phase 1 backend ladder is done.** Phase 2 (video, documents, PDF tooling) is parked and out of focus in the roadmap; this is written down so the design survives, not so it gets built next.

**Tagging note:** the roadmap tags the v1 task **[BE][FE]**, but storing nodes and ops means new tables, so the work is also **[DB]** — hence the header tags.

**Schema shape.** Nodes and ops are new tables: a new numbered `.sql` file registered in the `MIGRATIONS` array in [migrations.rs:16](../../src/database_related_scripts/migrations.rs#L16) (or, per the current early-dev practice, edited into `0001` with the DB wiped). Each new table needs a struct with a `FromRow` impl ([db_from_row.rs:12](../../src/database_related_scripts/db_from_row.rs#L12)) plus `Insert` ([insert.rs:10](../../src/database_related_scripts/insert.rs#L10)) and, for anything looked up by something other than its id, `Search` ([db_search.rs:12](../../src/database_related_scripts/db_search.rs#L12)).

**Gotcha the auth tables already tripped over:** the default methods on `Extract` and `Update` hardcode `WHERE id = ?` ([extract.rs:19](../../src/database_related_scripts/extract.rs#L19), [db_update.rs:16](../../src/database_related_scripts/db_update.rs#L16)), so a table whose primary key is called anything other than `id` cannot use them — that is exactly why `sessions` (`session_id`) and `totp` (`user_id`) are broken through those paths. Name the document/node/op primary keys `id`.

**One representation across storage, wire and sync** means the op type is a `serde` type with both `Serialize` and `Deserialize`. In Rust the natural shape is an enum with one variant per op kind (insert-after-node, delete-node, replace-node-text), tagged so it round-trips as self-describing JSON — `#[derive(Serialize, Deserialize)] #[serde(tag = "type")] enum DocumentOp { … }` — stored as a TEXT column and sent as the request/response body unchanged. `serde_json = "1.0.151"` is already a dependency ([Cargo.toml:14](../../Cargo.toml#L14)) though currently unused in `src/`.

**Serializing edits is not automatic.** [`Database::setup_new_transaction`](../../src/database_related_scripts/db.rs#L388) runs a closure inside a SQLite transaction on one pooled connection, but the pool has `max_size(4)` ([db.rs:54](../../src/database_related_scripts/db.rs#L54)), so four requests can be in flight at once. "The server *is* the total order" needs an actual per-document lock in shared state (a `tokio::sync::Mutex` keyed by document id is the usual shape) — the DB transaction alone gives atomicity, not a single global arrival order per document.

**Router state has to grow first.** Today it is `Arc<Database>` and nothing else ([api_routes.rs:45](../../src/routes/api_routes.rs#L45)). Real-time fan-out needs a per-document broadcast channel (`tokio::sync::broadcast`) living somewhere both the write handler and the subscribe handler can see, so state must become a struct holding the DB plus that map. This is the same widening the unused signing key already needs — see [Cross-Cutting Backend Polish](07-cross-cutting-backend-polish.md) and [Operations & Security](12-operations-and-security.md).

**WebSocket needs a Cargo change.** `axum = "0.8.9"` uses default features, which don't include `ws`; SSE (`axum::response::Sse`) works with what's already there. Whichever is chosen, note the route table currently uses axum 0.7-style `:id` path params, which axum 0.8 rejects — new routes must use `{id}` (tracked in [Cross-Cutting Backend Polish](07-cross-cutting-backend-polish.md)).

**Office formats: the zip half is already solved.** `zip = "8.6.0"` ([Cargo.toml:38](../../Cargo.toml#L38)) is in the tree for CBZ cover extraction, so reading a docx container reuses a known dependency; the XML-tree half is the actual work, and the lossiness warning above stands.

**Suggestion (not a decision):** since ops arrive over an unreliable link and get replayed from an offline queue, give each op a client-generated id and make application idempotent, so a re-sent op after a flaky reconnect doesn't get applied twice.

**Testing.** There are no tests in the repo at all. Op replay is unusually easy to unit-test (apply a list of ops to an empty document, assert the result; then assert replay-from-log equals live state), so this is a good candidate for the testing strategy in [Cross-Cutting Backend Polish](07-cross-cutting-backend-polish.md).

## Related

- [Video Library (Phase 2)](20-video.md) — the other parked Phase 2 media pillar; same "out of focus until Phase 1 lands" status.
- [PDF Annotations (Phase 2)](22-pdf-annotations.md) — the other Phase 2 document-adjacent feature (per-user comments + highlighting).
- [Reading & Read State](02-reading-and-read-state.md) — offline download/reading is what the offline op queue ties into.
- [Authentication, Users & Sessions](03-auth-users-and-sessions.md) — edits need an authenticated user to attribute and authorize them.
- [Library & Directory Management](01-library-and-directory-management.md) — `LibraryType::Documents` is a library type; libraries/series are how content is organised today.
- [Cross-Cutting Backend Polish](07-cross-cutting-backend-polish.md) — router state widening, axum 0.8 path syntax, and the testing strategy this feature would use.
- [Interop & Client Apps (Phase 2)](23-interop-and-clients.md) — the clients that would open and edit documents.
- [Operations & Security](12-operations-and-security.md) — the LAN + Tailscale threat model that makes "all authenticated users can edit" defensible.
