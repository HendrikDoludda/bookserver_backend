# Office documents

State: blocked

**Blocked:** builds on the markdown documents in [21](21-documents.md), which come first.

Editing docx / xlsx / pptx in the app. These are zipped XML trees, not text, so an edit is an
XML-tree change rather than a text op — meaningfully more work than markdown. The zip half is
already a dependency (`zip`, used for CBZ covers); the XML half is the work.

## Build
- [ ] ? h · Import an Office file into the document model, per the Office-model decision [BE]
- [ ] ? h · Export back to the Office file, only on an explicit save, per the Office-model decision [BE]
- [ ] ? h · Edit Office documents in the app [FE]

## Open decisions
- [ ] Intermediate model or fidelity-preserving overlay? — blocks: import, export. Suggested: decide after markdown v1 ships, then start with the intermediate model — reuse v1's op store; confine XML surgery to import (assign ids to paragraphs, runs and cells) and export (replay ops → XML → rezip; inserted images ride in the op as a blob and are wired into `word/media/`, the rels and `<w:drawing>` only at export) — and accept that formatting the importer doesn't model is lost. The overlay (ids anchored onto the untouched original XML) is the later escape hatch.
