# Date-aware summaries and column controls implementation plan

> **For Hermes:** Use subagent-driven-development task-by-task; this worktree is already dirty. Do not commit, revert, stage, or overwrite unrelated changes.

**Goal:** Treat reliably recognized date fields as dates in Quark summaries, show their overall ranges and yearly breakdowns, and make the Columns control a searchable submenu with nullity-based visibility actions.

**Architecture:** Keep date recognition and aggregate summaries in the existing FastAPI/DuckDB response path. Use the existing local `hiddenColumns` array in `App.svelte`; the menu only changes that array and never alters source data. Extend the existing date profile payload rather than create another endpoint.

**Tech stack:** FastAPI, DuckDB, Svelte 5, TypeScript, pytest; no dependencies.

---

### Task 1: Typed date inference and date summary contract

**Files owned:**
- Modify: `backend/app.py`
- Modify: `tests/test_backend.py`

**Work:**
1. Preserve native DuckDB date/time types.
2. For XLSX `all_varchar` input, infer a date/timestamp only when at least 90% of nonblank values successfully `TRY_CAST`; keep the column as text otherwise. This extends the existing uncommitted XLSX inference helper instead of duplicating parsing elsewhere.
3. Extend the existing `kind == "date"` stats response with `year_counts`, ordered by year, while retaining the current `min`, `max`, null counts, distinct count, and histogram.
4. Add one focused backend test that proves text-like valid dates are typed, date profile summary reports min/max and correct annual counts, and ordinary identifiers that happen to resemble dates are not falsely coerced.

**Acceptance:** date query metadata is `profile_kind: "date"`; profile stats contain JSON-safe `min`, `max`, and `[{'year': 'YYYY', 'count': n}]`; existing date stats remain compatible.

### Task 2: Columns submenu and nullity visibility controls

**Files owned:**
- Modify: `frontend/src/App.svelte`
- Modify: `frontend/src/app.css`
- Modify: `frontend/src/lib/types.ts`

**Work:**
1. Replace the bare native Columns disclosure contents with one compact styled submenu using native `<details>`.
2. Add a search input that filters the column checklist with the same case-insensitive substring matching behavior as the existing table column search. It only filters menu rows; it never hides table columns by itself.
3. Add a `Hide 100% null` action and a percentage input + `Hide ≥ N% null` action. Exclude protected dedupe columns and retain at least one visible column.
4. Add `Show all` as the reversible escape hatch. Keep individual checkbox visibility behavior.
5. For date profiles, render the overall range and compact year breakdown beneath the existing completeness summary.
6. Keep keyboard/focus behavior native and preserve existing mobile sizing.

**Acceptance:** opening Columns exposes search, nullity controls, and matching column rows; 100% and threshold actions hide only eligible columns; Show all restores them; date profile visibly presents range and per-year counts.

### Task 3: Verify and relaunch

**Files owned:** no implementation files unless a test reveals a minimal defect.

**Checks:**
1. `uv run pytest -q`
2. `(cd frontend && npm run check && npm run build)`
3. `docker compose up -d --build` then `docker compose ps`
4. Real HTTP response from `http://127.0.0.1:8000/`
5. Browser smoke: load a source, open Columns, search, run 100%-null and threshold hide, Show all, then open a date profile and confirm range/year breakdown.
