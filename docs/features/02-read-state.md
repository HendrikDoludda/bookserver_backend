# Read state

State: not started

Each user's place in each book — where they got to, and whether they finished — so any
client can resume, list what the user is in the middle of, and mark what's done. It is the
first truly per-user feature, so it needs a logged-in user.

## Build
- [ ] 2 h · Read-state table keyed by `(user_id, book_id)`, holding the position (shaped per [reading position](41-reading-position.md)), a finished flag and `last_read_at` — no `id` column, so it gets dedicated `Database` methods instead of `Extract`, like `library_elements`; never reuse `books.page_number`, which is parsed from the file name [DB]
- [ ] 1 h · Save progress in one statement — `INSERT … ON CONFLICT(user_id, book_id) DO UPDATE` — so there is no read-then-write race; read `last_read_at` as `Option<i64>`, because `convert_to_system_time` fails on NULL [BE]
- [ ] 2 h · `GET` and `PATCH /reading_progress/{book_id}` (now `ping_server`), with `user_id` from the session [BE]
- [ ] 1 h · Set "finished" per the finished decision [BE]
- [ ] 2 h · "Continue reading": the calling user's started, unfinished books, most recently read first, from a collection endpoint of its own [BE]
- [ ] 3 h · "Continue reading" surface [FE]
- [ ] 3 h · A checkmark on finished books and unread-count badges on series and libraries, counted per the unread-count decision [BE]
- [ ] 2 h · Checkmarks and unread-count badges [FE]

## Open decisions
- [ ] Is "finished" set by the user, or when the reader reaches the end? — blocks: setting "finished". Suggested: both — the reader reports a real end-of-content event, and the API also takes an explicit toggle so a user can mark or unmark anything.
- [ ] Unread counts: one grouped query per list request, or counters stored on `series` / `library`? — blocks: checkmarks and badges. Suggested: the grouped query — a home library is small, and a counter that drifts is worse than a join.
