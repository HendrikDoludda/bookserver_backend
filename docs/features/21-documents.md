# Collaborative documents

State: not started

Editable markdown / plaintext documents on the server, synced live between devices. A document
is an ordered list of nodes with unique ids; edits are small append-only ops anchored to
neighbour ids, and the server serializes them into one order — deliberately not a CRDT,
because there is a central server to do the ordering.

## Build
- [ ] ? h · Schema for documents, nodes and ops, per the where-documents-live decision [DB]
- [ ] ? h · Node model: each node has a unique id and its neighbours' ids; edits anchor to those ids, never to character offsets [BE]
- [ ] ? h · Ops as one serde enum (`#[serde(tag = "type")]`, one variant per op kind), stored as text and sent over the wire unchanged — the same representation for storage, wire and sync [BE]
- [ ] ? h · Apply ops one at a time per document with a per-document lock (e.g. a `tokio::sync::Mutex` keyed by document id) — the pool runs up to 4 connections, so a transaction alone gives atomicity, not one order; concurrent edits of the same node resolve last-writer-wins [BE]
- [ ] ? h · Tombstone deleted nodes, so edits anchored to them still resolve [BE]
- [ ] ? h · Make applying an op idempotent with a client-generated op id, so an op re-sent after a flaky reconnect isn't applied twice [BE]
- [ ] ? h · History and undo from the op log: replaying it from empty rebuilds the document, and stepping back one op gives the previous state [BE]
- [ ] ? h · Endpoints to create a document, read it and submit ops, per the permissions decision — `user_id` from the session, recorded on every op [BE]
- [ ] ? h · Push ops to everyone with the document open, per the transport decision — the per-document broadcast channels (`tokio::sync::broadcast`) need router state to grow beyond `Arc<Database>` [BE]
- [ ] ? h · Editor with autosave and an offline queue that flushes in order on reconnect [FE]
- [ ] ? h · Show other clients' edits live [FE]
- [ ] ? h · Create new documents in the app [FE]
- [ ] ? h · Check by hand: two clients with the same document open see each other's edits without a refresh, and edits made offline land in order after reconnecting

## Open decisions
- [ ] Where does a document live: its own `documents` table with node and op tables, or `books` rows in a `Documents` library? — blocks: the schema. Suggested: its own tables; `LibraryType::Documents` stays only the library's label (`books` is built around an immutable file on disk).
- [ ] WebSocket or server-sent events? — blocks: pushing ops. Suggested: SSE (`axum::response::Sse`, no extra feature) for fan-out plus normal POSTs for submitting ops; WebSocket (axum's `ws` feature) only if POST-per-edit latency hurts.
- [ ] Node granularity: sentence or block? — blocks: the node model. Suggested: paragraph / block for v1 — fewer ids and ops, and last-writer-wins is accepted anyway.
- [ ] Keep the whole op log, or snapshot and prune? — blocks: the schema. Suggested: keep everything for v1; documents are small.
- [ ] Who may open and edit a document? — blocks: the endpoints. Suggested: every logged-in user for v1, with each op recording its author so sharing can come later.
- [ ] If same-node auto-merge is ever needed: `yrs`, `automerge`, or a hand-rolled WOOT / RGA? — blocks: nothing unless last-writer-wins proves insufficient. Suggested: leave it unchosen until then.
