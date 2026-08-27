# DuckScope Product Context

**Register:** product

## Product purpose

DuckScope is a local-first data explorer for large CSV, Parquet, JSON, NDJSON, and DuckDB sources. It helps a technical user inspect rows, narrow a dataset, and understand column quality without writing SQL or loading the entire file into a browser.

## Users

- Data analysts quickly checking unfamiliar files before deeper analysis.
- Data engineers validating shape, nulls, ranges, and category distributions.
- Technical operators inspecting local exports while keeping data on-device.

## Primary job

Open a source, understand its structure, and move from raw rows to a trustworthy slice of data with filters, ordered sorting, and column profiles.

## Use context

A focused user works for 10 to 60 minutes on a laptop or wide desktop in normal office light. They scan dense tables repeatedly, compare columns, and need to preserve context while refining a query. Tablet use is occasional. Mobile should support inspection and recovery, not pretend to be a full spreadsheet workstation.

## Product tone

Precise, calm, capable, and local. It should feel like a well-made analytical instrument, not a developer console or a generic SaaS dashboard.

## Strategic principles

1. **The data canvas wins:** rows and columns receive the largest share of the screen.
2. **Reveal tools in context:** source management, filtering, and profiling appear when invoked.
3. **Preserve orientation:** selections, active conditions, row counts, and the current dataset remain visible.
4. **Prefer recognition:** label important actions and use one consistent icon family.
5. **Dense, not tiny:** compact layout is useful; 8 to 10 px UI text is not.
6. **Local trust:** make connection and source status clear without repeating it in multiple places.

## Anti-references

- Generic neon-on-black developer dashboards.
- Spreadsheet clones that imply data editing.
- Card-grid analytics dashboards that displace the actual table.
- Permanent sidebars full of setup controls after a source is already open.

## Constraints

- Keep the current FastAPI, Svelte 5, and fixed JSON API.
- Add no UI framework or runtime dependency for the overhaul.
- Preserve server paging, repeated filters, ordered multi-sort, nullity gauges, and on-demand statistics.
- Meet WCAG AA for text, focus, keyboard navigation, and control naming.
