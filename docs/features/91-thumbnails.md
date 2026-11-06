# Thumbnails

State: not started

Small resized covers for grid views, so the frontend doesn't download a full-size cover for
every card. Optional — not scheduled.

## Build
- [ ] ? h · Generate thumbnails with `image::DynamicImage::thumbnail` (`image` is already a dependency), per the generation decision [BE]
- [ ] ? h · Serve a thumbnail or the full cover through the [cover route](36-covers.md), chosen by size [BE]
- [ ] ? h · Use thumbnails in grid views [FE]

## Open decisions
- [ ] Generate thumbnails at scan time, or resize on each request? — blocks: generating thumbnails. Suggested: at scan time, next to the existing cover write in `cover_image_retriever.rs` — the scan already reads every file once and already writes a cover.
