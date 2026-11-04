# A–Z fast-scroll bar

State: not started

Letters and numbers down the side of the series list jump to the series starting with that
character; characters without series are greyed out and can't be tapped. The frontend works
the greying out from the series list — there is no backend endpoint for it.

## Build
- [ ] ? h · A–Z bar on the series list, with empty characters greyed out [FE]

## Open decisions
- [ ] Once the series list is paged, the frontend no longer sees every series and can't tell which letters are empty — keep the series list unpaged, or add a small "first letters in use" query? — blocks: the A–Z bar. Suggested: keep the series-per-library list unpaged — a home library's series count is small; revisit only if one gets big.
