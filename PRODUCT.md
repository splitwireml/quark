# Product

<!-- impeccable:product-schema 1 -->

## Platform

web

## Users

Quark is a public, open-source tool. Anyone can clone it and run it locally, so the primary user is a technical person meeting the product for the first time with no one to onboard them:

- Data analysts opening an unfamiliar CSV, Parquet, or XLSX export before committing to deeper analysis.
- Data engineers validating shape, nulls, ranges, join cardinality, and category distributions.
- Technical operators inspecting local exports on machines where the data must not leave the device.

Because the audience is public rather than a known team, first-run clarity, empty states, error recovery, and README-level self-explanation are product requirements, not polish. No user has institutional knowledge of how Quark works.

## Product Purpose

Quark opens a local data file and lets a user move from raw rows to a trustworthy, reproducible slice of that data — filtering, sorting, profiling, joining, aggregating, and transforming — without writing SQL, without loading the file into the browser, and without ever mutating or duplicating the source.

Success is a user opening an unknown 100k+ row file and, within minutes, knowing its structure and column quality, having narrowed it to the rows that matter, and holding a Version history that explains exactly how they got there.

## Positioning

Four mechanisms define Quark. All four are binding; trading any one away makes it a different product:

1. **Metadata-only versioning.** Every table and derived result is a View starting at Version 1. Cell edits, column add/modify/rename/hide/show/reorder, visibility, and ordering are recorded as replayable SQL/JSON changes, finalized in batches by **Stop recording**, with ancestry and a structured before/after diff. Browser storage holds transformation metadata and history labels — never result rows, never duplicate source data. Edits are history, not mutation.
2. **The whole file never moves.** Paging, filtering, ordered multi-sort, deduplication, category value search, and column statistics all execute server-side in DuckDB. A 100k+ row file opens immediately because only the visible page crosses the wire.
3. **Local-first, data stays on the machine.** Sources are files on the user's disk; attached DuckDB databases are opened read-only. There is no account, no upload to a service, no remote execution.
4. **No SQL required, SQL available.** Filters, joins, aggregates, dedupe, regex column selection, and profiling are reachable through the UI. A read-only SQL editor with completion and inline errors is there when the UI is not enough — it accepts one `SELECT` and nothing else.

## Operating Context

A focused user works for 10 to 60 minutes on a laptop or wide desktop in normal office light. They scan dense tables repeatedly, compare columns, and need to preserve orientation while refining a query — active conditions, sort order, row counts, and the current View must stay legible throughout.

The working unit is a **project**: a local tenant whose sources share one isolated DuckDB execution workspace, so any two Views in a project can be joined. A session typically runs: choose or create a project → add a source (upload a file, choose worksheets for XLSX, or attach a local DuckDB path) → open a base View → filter, sort, profile → derive joins, aggregates, or SQL Views → record transformations as Versions.

Projects and their sources persist in a small JSON registry and reopen after a backend restart when the source still exists.

## Capabilities and Constraints

**Confirmed capabilities**

- Sources: `.csv`, `.tsv`, `.parquet`, `.json`, `.ndjson`, `.jsonl`, `.xlsx`, `.duckdb`, `.db`. XLSX asks which worksheets to open before they become Views. Legacy `.xls` and `.sql` scripts are rejected.
- Server-paged tables with page size control, next/previous, and direct page jump.
- Typed filter operators per column, AND across columns, repeated conditions on one column, OR-within-column via `in`.
- Ordered multi-column sorting; nullity gauge under every column title; on-demand numeric statistics with histogram; bounded, server-searchable distinct category values for text columns.
- Derived Views: joins across any Views in the project, aggregates, dedupe, and read-only SQL — each with its own independent Version history.
- Export, version diff and restore, drag column reordering, regex column selection with inversion.

**Hard constraints**

- **Desktop only.** Wide screens are the real and only scene. Quark is planned to migrate to a Tauri desktop app with the FastAPI backend as a sidecar, so design decisions must hold up inside a native desktop window. Responsive work for phones is not a goal; do not spend design budget on a mobile spreadsheet experience.
- **WCAG AA and full keyboard operation are binding.** AA text contrast, visible focus, keyboard navigation, and accessibly named controls, including icon-only table actions.
- **Atomic Design is binding.** `frontend/src/components/` keeps `atoms/`, `molecules/`, `organisms/`, `templates/`. `App.svelte` owns state and API calls; components stay presentational and wired through callback props. Reuse existing components rather than duplicating or bypassing them.
- Backend contract is fixed: FastAPI owns DuckDB connections and query safety. Every identifier is validated against DuckDB metadata and quoted; values are bound parameters; maximum page size is 1000.
- Non-goals that must stay non-goals: mutating SQL, DDL, multiple statements, authentication or multi-user hosting, a remote DuckDB wire protocol, WebSocket push, distributed orchestration.

**Terminology** — use these words exactly, in the UI and in docs: *project*, *source*, *View* (capital V; a table or any derived result), *Version*, *derived View*, *recording* / *Stop recording*.

## Brand Commitments

- The product name is **Quark**. "DuckScope" is a stale earlier name; it survives in `DESIGN.md` and in dated files under `docs/plans/`, which are historical records.
- Tone: precise, calm, capable, and local. It should read like a well-made analytical instrument, not a developer console and not a generic SaaS dashboard.

## Evidence on Hand

Real, in-repo, usable as source material:

- `docs/SPEC.md` — authoritative behavior, API shapes, and acceptance criteria.
- `docs/UI_UX_EVALUATION.md` — a dated 2026-07-13 review with measured findings (8px minimum font size, 983 sub-12px elements, 720px document at 390px viewport). Historical evidence of prior problems, not current state.
- `docs/plans/` — dated implementation plans, one per feature.
- `backend/`, `frontend/src/` with a shipped Atomic Design component set; `tests/` and `frontend/tests/`.

Absences future work must not paper over: there are **no** users, testimonials, case studies, benchmarks, press mentions, pricing, logos, or brand assets. Do not invent any. There is no hosted deployment and no license claim recorded here.

## Product Principles

1. **The data canvas wins.** Rows and columns get the largest share of the screen; nothing permanent may displace the table.
2. **Reveal tools in context.** Source management, filtering, and profiling appear when invoked and recede when not. Setup controls do not hold a permanent seat once a source is open.
3. **Preserve orientation.** Active conditions, sort order, row counts, current View, and Version state stay visible while the user refines a query.
4. **Never duplicate the data.** Every feature must survive the rule that the browser holds metadata and one page of rows, and the source file is never mutated.
5. **Explain itself to a stranger.** The audience is public and unonboarded; first-run paths, empty states, and errors must each name the next action.

## Accessibility & Inclusion

WCAG AA for text contrast, focus visibility, keyboard navigation, and control naming. Icon-only actions in the table require accessible labels and tooltips. Motion respects `prefers-reduced-motion`. Density is a feature, but 8–10px UI text is not acceptable — the prior evaluation flagged it as the single largest defect.
