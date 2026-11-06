# Series picker for uploads

State: not started

The series field in the upload dialogs auto-completes against existing series. A name that
doesn't exist yet can be created on the spot: the user picks which scannable root it lives
under, and the backend creates the folder and names it per the convention.

## Build
- [ ] ? h · Series-name lookup for the auto-complete, through `Search` on `BookSeriesMetadata` (not the legacy `get_id_from_table`), case-insensitive — series names are stored lowercased [BE]
- [ ] ? h · Create a series inline: the `series` row and its `library_elements` link in one `setup_new_transaction`, reusing `create_series_entry` and `link_series_to_default_library`, so a failed link can't leave an orphan series [BE]
- [ ] ? h · Create the new series' folder under the chosen scannable root, named per the convention, with the same path containment as [uploads](05-uploads-and-ingestion.md) [BE]
- [ ] ? h · Auto-complete in the series field [FE]
- [ ] ? h · Offer to create a typed series that isn't found [FE]
- [ ] ? h · Ask which scannable root a new series lives under [FE]
