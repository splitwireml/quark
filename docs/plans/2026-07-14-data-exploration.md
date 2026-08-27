# DuckScope Data Exploration Implementation Plan

> **For Hermes:** Use subagent-driven-development skill to implement this plan task-by-task.

**Goal:** Add useful profiles for every inspectable column, a complete no-source onboarding path, client-side column hiding, and safe multi-column deduplication without changing DuckScope’s local-first architecture.

**Architecture:** Extend the existing `GET .../stats` endpoint into one discriminated profile response and extend the existing query body with an optional list of dedupe key columns. Keep source/dataset query state server-side and keep presentation-only hidden-column state in `App.svelte`. Reuse the one existing inspector; no new endpoint, component, dependency, or persistence layer.

**Tech Stack:** FastAPI, DuckDB, pytest, Svelte 5, TypeScript, native HTML/CSS.

## Contract decisions

- `Query` gains `dedupe_columns: list[str] = []`.
  - Every name is validated against dataset metadata; duplicate names are rejected with `422`.
  - Pipeline is **filter → dedupe selected key tuple → sort → page**. This makes dedupe operate on the user’s current result set.
  - Dedupe returns one representative row for each selected-value tuple. The no-key case remains byte-for-byte existing behavior. The representative is intentionally the input scan’s first row; DuckScope has no universal primary key or edit model.
  - `total_rows` and null fractions remain based on the deduped result and source dataset respectively, matching their existing meanings.
- `GET .../stats` returns a shared base (`type`, `kind`, `row_count`, `non_null_count`, `null_count`, `null_fraction`) and one kind-specific payload:
  - `numeric`: existing min/max/mean/stddev/p25/median/p75/histogram fields.
  - `categorical`: `distinct_count` and bounded `top_values: [{value, count}]`, including text/category-like values; no unbounded cardinality payload.
  - `date`: `min`, `max`, `distinct_count`, and a bounded histogram of date/time ranges. Values use the existing JSON-safe ISO representation.
- Query column metadata gains nullable `profile_kind` (`numeric`, `categorical`, `date`, or `null`) so the UI offers Profile only for types the server supports. Hidden columns are a browser-only view preference and are never sent to the backend.

---

### Task 1: Add typed profile and dedupe API behavior

**Objective:** Make the shared backend contract correct and injection-safe.

**Files:**
- Modify: `backend/app.py`
- Modify: `tests/test_backend.py`

**Steps:**
1. Write failing backend tests covering text/category and date profile responses plus filtered multi-column dedupe, invalid/missing/repeated keys, and unchanged no-key behavior.
2. Run `uv run pytest -q`; observe failures from the unsupported profile kinds/dedupe request field.
3. Add the smallest `Query.dedupe_columns` validation and build the filtered relation once for count/query. Use metadata-derived quoted identifiers only; values stay bound parameters.
4. Add small type predicates and branch the existing stats endpoint into numeric, categorical, and date implementations. Keep numeric output and exact-large-integer histogram behavior unchanged.
5. Re-run `uv run pytest -q`.

**Commit:** Defer: the worktree already contains unrelated uncommitted UI-overhaul changes.

### Task 2: Wire the expanded API contract into the Svelte client

**Objective:** Give the existing UI a typed, minimal way to request dedupe and render all profile variants.

**Files:**
- Modify: `frontend/src/lib/types.ts`
- Modify: `frontend/src/lib/api.ts`
- Modify: `frontend/src/App.svelte`

**Steps:**
1. Add the discriminated profile types and optional `dedupe_columns` to the existing request type.
2. Keep `queryDataset` unchanged except for passing the now-typed request body.
3. In `App.svelte`, track `dedupeColumns` with filters/sorts and send it in `loadData`.
4. Replace numeric-only profile gating with one profile action and render a compact numeric/category/date body inside the existing inspector, preserving loading/error/focus-return behavior.
5. Add a dedupe token to the existing query bar, with one remove/clear path that reloads page one.
6. Run `npm run check && npm run build`.

### Task 3: Add quick hidden-column and dedupe controls

**Objective:** Let users remove visual noise or collapse duplicates without inventing a data-grid framework.

**Files:**
- Modify: `frontend/src/App.svelte`
- Modify: `frontend/src/app.css`

**Steps:**
1. Derive visible columns from `result.columns` minus browser-only `hiddenColumns`; do not mutate server data or rows.
2. Add one native Hide action per visible column header and a compact `Columns` disclosure in the query bar that restores individually hidden columns or all columns.
3. Add a native `details` dedupe disclosure listing visible columns as checkboxes. The selected ordered keys define the dedupe tuple; Apply triggers one reload; Clear removes the query state and reloads.
4. Preserve a usable state when every column is hidden: show an explicit restore action instead of an empty malformed table.
5. Do not hide columns already selected as dedupe keys until they are removed from dedupe, so the current query remains understandable.
6. Run `npm run check && npm run build`.

### Task 4: Complete the no-source onboarding path

**Objective:** Make the first useful action and local-data boundaries obvious without a tutorial system.

**Files:**
- Modify: `frontend/src/App.svelte`
- Modify: `frontend/src/app.css`
- Modify: `README.md`

**Steps:**
1. Replace the single welcome upload button with clear upload and attach paths that reuse the existing handlers/state.
2. Explain supported formats, read-only attached databases, and the three actual next steps: add source → choose dataset → filter/profile columns.
3. Keep the empty/error/retry path and keyboard-accessible labels intact. Do not add onboarding persistence, accounts, tooltips, tours, analytics, or a dependency.
4. Add a brief README sentence pointing at the same first-run flow.
5. Run `npm run check && npm run build`.

### Task 5: Integration verification and review

**Objective:** Prove the complete flow works as one system.

**Files:**
- No planned production code changes.

**Steps:**
1. Run `uv run pytest -q`.
2. Run `npm run check && npm run build` from `frontend/`.
3. Start the API against a disposable data dir and test upload/query/profile/dedupe with a real CSV containing text, dates, nulls, and duplicate key tuples.
4. In a browser, verify onboarding, attach/upload affordances, hide/restore, dedupe apply/clear, text/date/numeric profiles, inspector Escape/focus return, and query pagination totals.
5. Run spec-compliance review, then quality review. Fix any critical/important findings before declaring done.

## Explicit non-goals

- No editable/deleted source data; dedupe is a query view only.
- No automatic/heuristic duplicate detection, merge rules, or record survivor editor.
- No global saved view preferences or onboarding completion tracker.
- No dependency, new endpoint, component framework, or SQL console.
