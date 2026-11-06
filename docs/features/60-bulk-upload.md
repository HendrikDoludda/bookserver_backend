# Bulk upload

State: not started

Upload many files at once: optional global fields at the top, one row per file that can
override them, and a mandatory preview of every resolved name and target path before anything
touches disk. Files are ingested one after another through
[file upload](05-uploads-and-ingestion.md), so one bad file doesn't stop the rest.

## Build
- [ ] ? h · Accept several files in one upload and ingest them one after another, awaiting each in the handler loop — not the scanner's unjoined `tokio::spawn` per file — so the response reports each file's real outcome [BE]
- [ ] ? h · Resolve each field per file, highest wins: per-file edit → global field (only if filled) → value parsed from the filename → empty [BE]
- [ ] ? h · A filled global volume plus starting chapter auto-increments across the batch, files ordered by natural sort (`natord::compare`, already a dependency) so `c2` comes before `c10` [BE]
- [ ] ? h · Preview: return each file's resolved fields and target path and name without writing anything; confirming runs exactly that plan [BE]
- [ ] ? h · A file that fails (no free space, any other error) is dropped and reported; the rest of the batch continues [BE]
- [ ] ? h · Notify the uploader of renames and dropped files per the batch-notification decision [BE]
- [ ] ? h · Select or drop several files at once [FE]
- [ ] ? h · Global fields at the top of the batch — series, volume, starting chapter — each optional [FE]
- [ ] ? h · One row per file, each field editable to override the globals [FE]
- [ ] ? h · Preview and confirm step: each file's resolved fields and target path, editable inline; nothing is written before confirming [FE]

## Open decisions
- [ ] Where does the resolution run? — blocks: the preview. Suggested: in the backend, returned as the preview — the frontend only renders it and sends edits back, so there is one implementation and the preview is exactly the plan that runs.
- [ ] One email per renamed or dropped file, or one summary per batch? — blocks: batch notifications. Suggested: one summary after the batch.
