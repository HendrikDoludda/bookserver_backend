# Manga metadata

State: not started

Manga libraries get their metadata from AniList, MangaDex and MangaUpdates, through the
[metadata enrichment](09-metadata-enrichment.md) flow. Providers are chosen by library type,
and there is no manga library type yet.

## Build
- [ ] 2 h · A manga library type, per the manga-type decision — today `LibraryType` has no manga variant and the scanner creates every library as `LibraryType::Books` [BE]
- [ ] 4 h · AniList adapter for `MetadataProvider` [BE]
- [ ] 4 h · MangaDex adapter for `MetadataProvider` [BE]
- [ ] 4 h · MangaUpdates adapter for `MetadataProvider` [BE]

## Open decisions
- [ ] Add a `Manga` variant to `LibraryType`, or route manga through a per-library provider override? — blocks: the manga library type. Suggested: the `Manga` variant — routing already keys off library type, and unknown strings map to `Others`, so no migration is needed.

## Missing information
- [ ] Can AniList be used commercially? Believed: a licence is needed above $150/month revenue — answered by: AniList's API terms
