# Directory scanning

State: in progress

The scanner walks the configured directories, parses volume / chapter / page numbers out of
file names, and creates the book, series and library rows plus a cover for each supported
file. It works, but it can report success before it is done, swallows per-file errors, and
never picks up a file that changed.

## Build
- [ ] 6 h · Run a scan as a job, per the scan-job decision: `POST /scan_all_directories` answers 202, and `GET /scan/status` (now `ping_server`) reports progress and per-file failures — router state grows from `Arc<Database>` into an `AppState` that also holds the scan status [BE]
- [ ] 1 h · Wait for every spawned file task before a scan counts as done — `scan_folder` drops the `JoinHandle`s, so the last file can still be in progress when the scan reports success (`futures::future::join_all`; `futures` is already a dependency) [BE]
- [ ] 2 h · Report per-file failures instead of discarding them — only a task panic is logged; `create_book_entry`'s own errors disappear [BE]
- [ ] 1 h · One missing directory aborts the whole scan, so the directories after it are never scanned — report it and carry on [BE]
- [ ] 4 h · Incremental scanning: skip files whose stored `file_hash` and `last_modified` are unchanged, and update the row of a file that changed — today a file already in the database is skipped even when its contents changed [BE]
- [ ] 1 h · Match file extensions case-insensitively (`.PDF` and `.JPG` are skipped today), and accept `.webp` pages, which the cover code already handles [BE]
- [ ] 3 h · A database error while looking up a series or library counts as "not found" and creates a duplicate (`get_series_id`, `get_library_id`) — propagate the error, and move these lookups off the legacy `get_id_from_table` onto `search_for_single_row` [BE]
- [ ] 2 h · Keep a series' original casing for display — names are stored lowercased because the lowercased name is the duplicate check; compare case-insensitively instead (e.g. `COLLATE NOCASE`) [DB] [BE]
- [ ] 3 h · File-name parsing: accept decimal chapters (`c01.5` parses as chapter 1 today) and `.` / `-` separators (`vol.3` and `v-3` don't match) — [uploads](05-uploads-and-ingestion.md) name files to this same rule set [BE]
- [ ] 2 h · Strip bracketed tags such as `[Group]` or `(Digital)` from parsed titles, so they don't split one series into several [BE]
- [ ] 1 h · Use `walkdir` for the directory walk, or drop the unused dependency [BE]

## Open decisions
- [ ] Is a scan a request or a job? Today the handler awaits the scan inline, yet `GET /scan/status` exists as a placeholder — blocks: running a scan as a job. Suggested: a job — spawn the scan, answer 202, report progress through `/scan/status`; that also makes scanning a single directory natural.
