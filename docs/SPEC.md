# Quark Product Specification

## Goal
A local-first data viewer for large CSV, Parquet, JSON/NDJSON/JSONL, XLSX, and DuckDB files. FastAPI owns DuckDB connections and query safety; a Svelte 5 SPA renders fast server-paged tables.

## User flow
1. Open Quark and choose or create a project.
2. Upload a supported file or attach a local DuckDB database path as a project source.
3. Every source table or selected workbook worksheet appears as a base View.
4. Browse rows with sticky headers, horizontal/vertical scrolling, page-size control, next/previous, and direct page jump.
5. Filter any column using typed operators. Conditions across columns combine with AND; repeated conditions on one column are supported.
6. Apply ordered multi-column sorting.
7. See null percentage as a faint gauge under every column title.
8. Click a numeric column header to inspect count/nulls/min/max/mean/stddev/quantiles and a histogram.
9. Create read-only SQL, aggregate, or join Views; joins may use any Views in the project.
10. Record cell/column edits, visibility, and ordering as metadata-only Versions of the active View; finalize a batch with **Stop recording** and inspect its diff.
11. Restore prior Versions without copying source rows.

## MVP semantics

### Projects and sources
A project is the top-level local tenant. Its sources share an isolated DuckDB execution workspace so any Views in that project can be joined.
- Upload: saves a supported non-XLSX file under the app data directory and creates a DuckDB database containing a view named `data` over it.
- XLSX upload: saves the workbook under the app data directory, then asks which worksheets to open; only confirmed worksheets become base Views.
- DuckDB upload: opens the uploaded database directly.
- Attach path: opens an existing local `.duckdb`/`.db` file read-only.
- Active nodes are persisted in a small JSON registry and reopened after restart when their source still exists.
- Existing registries migrate into a default project without rewriting source metadata merely on startup.
- “Running” means registered and connectable by this backend. Cross-process DuckDB connection discovery is not portable; no process scanning or remote SQL protocol is invented.

### Supported files
`.csv`, `.tsv`, `.parquet`, `.json`, `.ndjson`, `.jsonl`, `.xlsx`, `.duckdb`, `.db`.

### Query API
`POST /api/nodes/{node_id}/datasets/{dataset}/query`

Request:
```json
{
  "page": 1,
  "page_size": 100,
  "filters": [{"column":"price","operator":">=","value":100}],
  "sorts": [{"column":"price","direction":"desc"}]
}
```

Response:
```json
{
  "columns": [{"name":"price","type":"DOUBLE","numeric":true,"null_fraction":0.04}],
  "rows": [{"price":123.4}],
  "page": 1,
  "page_size": 100,
  "total_rows": 24012,
  "total_pages": 241,
  "elapsed_ms": 18.2
}
```

Filter operators:
- all types: `=`, `!=`, `in`, `is_null`, `not_null`
- `in` accepts a non-empty value array and ORs those values within one column
- text: `contains`, `starts_with`, `ends_with`
- ordered values: `>`, `>=`, `<`, `<=`

Every identifier is validated against DuckDB metadata and quoted. Values are bound parameters. Maximum page size: 1000.

`POST /api/nodes/{node_id}/sql` accepts one read-only `SELECT` plus `page` and `page_size`, and returns the same paged row shape. Multiple statements and mutating/DDL commands are rejected with HTTP 422. Query-builder responses include their executable SQL equivalent so filters, sorts, and dedupe queries can be saved and re-run.

### Views and Versions
- Every source table and every derived result is a View with Version 1.
- Cell edits and column add/modify/rename/hide/show/reorder are replayable Version changes on the active View.
- Multiple changes remain pending until **Stop recording** finalizes one next Version with ancestry and a structured before/after diff.
- Joins, aggregates, and explicit SQL create new Views with independent histories rather than incrementing an input View.
- Any two Views in one project may be joined because their read-only SQL executes in the same project workspace.
- Browser storage contains only SQL and JSON transformation metadata, column order/visibility, and history labels—never result rows or duplicate source data.

### Category values API
`GET /api/nodes/{node_id}/datasets/{dataset}/columns/{column}/values`

For text columns, returns a bounded page of distinct non-null values with row counts, ordered by count descending then value. Query parameters are `search` (case-insensitive, default empty), `offset` (default 0), and `limit` (default 200, maximum 500). The response is `{values, total, offset, limit, has_more}` for the filtered distinct set, so every category remains discoverable through server search and paging without one giant payload.

### Statistics API
`GET /api/nodes/{node_id}/datasets/{dataset}/columns/{column}/stats`

For numeric columns returns type, row count, non-null count, null count/fraction, min, max, mean, stddev, p25, median, p75, and up to 20 histogram bins.

## UI
- Dense, clean, neutral desktop-first layout; usable down to tablet width.
- Project picker: create, open, and leave projects.
- Left rail: add sources and browse their base Views plus derived Views; each View shows its active version at the right edge.
- Table: sticky first row, sticky first column, horizontal/vertical scrolling, compact cells, null token styling.
- Column header: name, type, ordered sort marker, filter action, nullity gauge.
- Filter bar: removable condition chips; per-column operator/value editor; supports repeated conditions.
- Text/category columns load bounded pages of distinct non-null values into a server-searchable dropdown with checkboxes and “Load more.” Selecting one value applies a single-value category filter; selecting multiple values applies one OR-within-column filter. Separate filter chips still combine with AND.
- Sort bar: ordered removable sort chips; clicking a header cycles asc → desc → off while preserving order of other sorts.
- Numeric header click opens a modal/panel with summary cards and lightweight CSS/SVG histogram.
- Loading, empty, and API error states are explicit. Keyboard focus and button labels remain accessible.
- A floating SQL View editor provides syntax highlighting, table/field completion, and inline errors.
- A Versions tab restores finalized Versions and displays diffs for the active View.
- Column headers and the Columns menu support animated drag ordering with edge scrolling; menu buttons provide keyboard/touch fallback.
- Regex column selection supports inversion, invalid-pattern feedback, and preserves dedupe keys plus at least one visible column.

## Non-goals
- Mutating SQL, DDL, multiple statements, or uploaded `.sql` scripts.
- Authentication/multi-user hosting.
- Remote DuckDB wire protocol.
- WebSocket push or distributed node orchestration.

## Acceptance criteria
- A 100k+ row CSV opens without sending the whole dataset to the browser.
- Pagination, page size, repeated filters, and ordered multi-sort are executed server-side.
- Nullity gauges appear for every column.
- Numeric stats and histogram load on demand.
- Projects and their uploaded/attached sources survive backend restart; sources remain isolated to their project.
- Backend tests pass; frontend type checks/build pass; a real upload/query smoke test passes.
- View/version browser metadata contains no result rows; derived-View creation, independent finalize/diff/restore, drag ordering, and regex selection pass a real browser smoke flow.
