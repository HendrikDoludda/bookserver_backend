# Video library & direct play

State: blocked

**Blocked:** needs the modular core ([94](94-modular-core.md)) — video is its second module.

The same server that streams books also streams video: one machine, one copy of the files, no
second media server. The app plays files directly with range requests and decodes them on the
device, so the server never transcodes. The second door, for Infuse, is
[70](70-webdav-for-infuse.md).

## Build
- [ ] ? h · Video in the data model, per the data-model decision — today `LibraryType` (`Books | Comics | Magazines | Documents | Others`) and `BookFormat` have no video variants, and no table has season or episode columns [DB] [BE]
- [ ] ? h · Upload dialog for video: kind `movie` / `series` / `original`; `series` adds series, season and episode, and a movies library pre-fills neither [FE]
- [ ] ? h · Batch titles for video uploads: in a series batch the global title is the series name and each file's title the episode name; in a movies batch each file's title is the movie name [BE]
- [ ] ? h · Scanner accepts video files: `is_valid_file_type` and `get_book_format` accept only `pdf`, `epub`, `cbz`, `jpg`, `jpeg` and `png` today, and match case-sensitively, so a `.MKV` would be skipped [BE]
- [ ] ? h · Identify video files without hashing them whole — `hash_file` runs BLAKE3 over the entire file for every new file, which means reading gigabytes per video; use size plus modification time, or hash only a prefix [BE]
- [ ] ? h · Direct-play endpoint for video, built like `request_file`: `streaming_file` hands the request to `tower_http`'s `ServeFile`, which already gives Range / 206 / `If-Range` / `HEAD` — no hand-rolled range code, no transcoding [BE]
- [ ] ? h · In-app video player that decodes on the device (e.g. Flutter's `media_kit`, which bundles ffmpeg — an app dependency, not a Rust crate) [FE]
- [ ] ? h · Check by hand: the app plays an MKV and an MP4 end to end while the server's CPU stays flat (no transcode process)

## Open decisions
- [ ] Where does video live in the data model: nullable video columns on `books` / `series`, or its own table and struct? — blocks: video in the data model. Suggested: decide when video starts, designing against the upload fields above (`movie` / `series` / `original`, plus season and episode).
