# Video metadata

State: blocked

**Blocked:** needs the video library from [20](20-video.md) — there is nothing to enrich yet.

Posters, synopsis, cast and season/episode data for videos, from TMDB / TVDB-style providers.
Separate from [book metadata](09-metadata-enrichment.md) because video needs different
providers and a movie/show data model, but it plugs into the same `MetadataProvider`
abstraction, routed by content type.

## Build
- [ ] ? h · Fetch poster, synopsis, cast and season/episode data through the `MetadataProvider` abstraction, routed by content type — posters come from the provider, never from the file (extracting a frame would mean decoding video on the server) [BE]
- [ ] ? h · Store the provider's id on the video entry, so its metadata can be fetched again later [DB] [BE]
- [ ] ? h · Show the poster and synopsis in the app [FE]

## Missing information
- [ ] Which provider (TMDB, TVDB, …), and do its terms allow this use? — answered by: the providers' API terms
