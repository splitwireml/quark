# Filter-Aware Columns Implementation Plan

> **For Hermes:** Use subagent-driven-development skill to implement this plan task-by-task.

**Goal:** Make DuckScope’s column controls reversible and its table metadata/profile statistics reflect the applied filter and dedupe query.

**Architecture:** Keep one canonical filtered relation in `backend/app.py`: query row counts, null fractions, and profile statistics all use it. Keep UI state local in `App.svelte`; use native `<details>`, CSS, and existing `hiddenColumns` state—no dependency or new component.

**Tech Stack:** FastAPI, DuckDB, Svelte 5, TypeScript, pytest.

---

### Task 1: Make query metadata and stats filter-aware

**Objective:** Use the same validated filter/dedupe relation for `total_rows`, per-column nullity, and column profile statistics.

**Files:**
- Modify: `backend/app.py`
- Modify: `tests/test_backend.py`

**Acceptance checks:**
1. A filtered query returns `total_rows` and `columns[*].null_fraction` from filtered rows.
2. A stats request supplied the same filters/dedupe returns matching `row_count`, null counts, top values/range/histogram.
3. Existing unfiltered GET stats stays compatible.
4. `uv run pytest -q` passes.

### Task 2: Make columns controls reversible and headers usable

**Objective:** Add a desktop sidebar collapse control, an all-columns checked visibility menu, a one-second native tooltip for truncated header names, and `U` to undo the latest hide.

**Files:**
- Modify: `frontend/src/App.svelte`
- Modify: `frontend/src/app.css`
- Modify: `frontend/src/lib/api.ts`
- Modify: `frontend/src/lib/types.ts`

**Acceptance checks:**
1. Sidebar collapses/expands without affecting mobile drawer behavior.
2. Columns menu lists every column with checked=visible; toggling unchecked hides and toggling checked restores; dedupe keys and last visible column remain protected.
3. Hovering a truncated column label for one second displays its full name using a native `title` tooltip.
4. `U` outside editable controls restores the most recently hidden column.
5. Profile calls carry current filters/dedupe.
6. `npm run check && npm run build` passes.

### Task 3: Integration verification

**Objective:** Verify the vertical flow against a running app and existing dirty worktree.

**Files:**
- No implementation files unless a verified defect requires a minimal fix.

**Acceptance checks:**
1. Preserve unrelated pre-existing modifications.
2. Run `uv run pytest -q`, `npm run check`, and `npm run build`.
3. Start the app or use its existing dev server, apply a filter, confirm header nullity/row count/profile change, hide a column, press `U`, and collapse/expand sidebar.
