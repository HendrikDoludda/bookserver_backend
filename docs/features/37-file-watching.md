# File watching

State: not started

New and changed files in the scannable directories are picked up on their own: filesystem
events, debounced, trigger a re-scan of just the folders that changed. It needs
[incremental scanning](34-directory-scanning.md) — otherwise every event costs a full pass —
and the [directories table](35-scannable-directories.md) to know what to watch.

## Build
- [ ] ? h · Watch the scannable directories with `notify` (not a dependency yet) — it calls back on its own thread, so forward the events into a `tokio::sync::mpsc` channel and debounce in one async task, or use `notify-debouncer-full` [BE]
- [ ] ? h · After the quiet period, re-scan only the folders the events touched; removed files are pruned by [file reconciliation](53-file-reconciliation.md) [BE]
- [ ] ? h · Start and stop watching a directory when it is added to or removed from the table [BE]
- [ ] ? h · Check by hand: a supported file dropped into a watched directory shows up in the catalog within the debounce window, without a scan request
