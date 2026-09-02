# Versioning Engine Implementation Plan

> **For Hermes:** Use subagent-driven-development skill to implement this plan task-by-task.

**Goal:** Replace saved-query product semantics with metadata-only Versions and Views, with staged edits, finalization, history/diff, version restore, smooth column reordering, and regex column selection.

**Architecture:** Keep DuckDB data immutable and reuse the SQL already produced by Quark. Persist only small frontend metadata records in `localStorage`: finalized version SQL plus column visibility/order, pending change descriptions, and saved view SQL. Version 1 is the source baseline; mutations, cell edits, joins, hiding, renaming, additions, and reordering stage a version change until **Stop recording** finalizes it. Aggregate and explicit SQL runs create Views and never advance the version number.

**Tech Stack:** Svelte 5, TypeScript, native `localStorage`, native HTML drag events, native View Transitions when available, existing FastAPI/DuckDB query endpoints. No new dependency and no backend data copies.

---

## Task 1: Metadata-only versioning engine

**Objective:** Add tested, deterministic helpers for histories, pending changes, views, restore metadata, diffs, and legacy saved-query migration.

**Files:**
- Create: `frontend/src/lib/versioning.ts`
- Create: `frontend/tests/versioning.test.js`
- Modify: `frontend/package.json`
- Modify: `frontend/src/lib/types.ts`

**Steps:**
1. Write failing Node tests covering: source v1 creation, multiple staged changes finalized as one v2, no-op finalization, ordered version restore metadata, view creation without version increment, regex matching/invalid regex, and legacy `quark.savedQueries` conversion.
2. Run `node --experimental-strip-types --test tests/versioning.test.js`; verify RED.
3. Implement the smallest pure functions and serializable types. Store SQL/JSON metadata only; never rows.
4. Add a `test` script that runs both frontend Node test files.
5. Run `npm test`; verify GREEN.

## Task 2: Replace Queries with Versions and Views

**Objective:** Wire the engine into the existing app state and every transformation path without changing backend query safety.

**Files:**
- Modify: `frontend/src/App.svelte`
- Create: `frontend/src/components/organisms/VersionsViewsPane.svelte`
- Create: `frontend/src/components/organisms/VersionDiffDialog.svelte`
- Modify: `frontend/src/components/organisms/DatasetTabsBar.svelte`
- Modify: `frontend/src/components/organisms/DatasetHead.svelte`
- Modify: `frontend/src/components/organisms/QueryConditionBar.svelte`
- Modify: `frontend/src/components/organisms/SqlEditorPanel.svelte`
- Modify: `frontend/src/components/organisms/JoinMenuPopover.svelte`
- Remove: `frontend/src/components/organisms/SavedQueriesPane.svelte`
- Remove: `frontend/src/components/molecules/SavedQueryCard.svelte`
- Remove: `frontend/src/components/molecules/SavedQueryListItem.svelte`

**Steps:**
1. Load or create v1 whenever a sheet/dataset is selected.
2. Route column mutation, rename, duplicate, one-cell edit, hide/show/type visibility, and join completion through one `stageVersionChange` call.
3. Add a visible recording control showing pending change count; **Stop recording** finalizes one version and opens the structured diff dialog.
4. Replace the Queries tab/pane and all user-facing “query” save terminology with a Versions/Views history pane. Preserve SQL execution as an internal backend operation.
5. Explicit SQL Run and aggregate creation save a View. A join stages a Version. Restoring a version re-runs its SQL and restores its column visibility/order; opening a View runs its SQL without changing version history.
6. Migrate valid legacy saved queries to Views once and remove `quark.savedQueries`.
7. Run `npm run check && npm test && npm run build`.

## Task 3: Reorder columns and select with regex

**Objective:** Make column order a versioned metadata transformation in both table headers and the Columns menu.

**Files:**
- Modify: `frontend/src/App.svelte`
- Modify: `frontend/src/components/organisms/DataGridTable.svelte`
- Modify: `frontend/src/components/molecules/ColumnHeaderCell.svelte`
- Modify: `frontend/src/components/organisms/ColumnsMenuPopover.svelte`
- Modify: `frontend/src/app.css` only if shared View Transition CSS is necessary

**Steps:**
1. Use existing result columns plus a single `columnOrder` array; do not duplicate row data.
2. Add native drag handles to headers and menu rows. Reorder continuously across neighbors, animate with native View Transitions when available, and edge-scroll the table/list while dragging.
3. Stage one `reorder` change per drag completion, not one history event per neighbor crossed.
4. Add a regex input with Show matches / Hide matches actions, inline invalid-pattern feedback, and protection against hiding the last visible or dedupe-key columns.
5. Stage regex visibility changes as one version change.
6. Run `npm run check && npm test && npm run build`.

## Task 4: Integration verification

**Objective:** Prove the real metadata-only workflow works and nothing regressed.

**Files:**
- Modify: `docs/SPEC.md`
- Modify: `README.md`

**Steps:**
1. Run `uv run pytest -q`.
2. Run `cd frontend && npm run check && npm test && npm run build`.
3. Start the built app with an isolated `QUARK_DATA_DIR`, upload a CSV, and verify via browser: v1 exists; rename + add + reorder + hide stay pending; Stop recording makes one v2; diff opens; v1/v2 restore; aggregate and SQL create Views without v3; regex selection works; drag edge-scroll works.
4. Inspect browser console for errors and `localStorage` to confirm records contain SQL/JSON metadata and no row arrays.
5. Commit the verified feature on `feat/versioning-engine`.

## Deliberate ponytail limits

- Browser-local metadata matches Quark’s existing single-user saved-query behavior; add backend-shared metadata only when multi-browser/device sync is required.
- Cell edits remain row-position SQL patches; add key-based patches when datasets expose primary keys.
- Native drag/View Transitions are used instead of a drag library; add a dependency only if measured browser behavior is inadequate.
