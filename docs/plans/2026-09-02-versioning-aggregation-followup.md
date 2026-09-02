# Versioning and Aggregation Follow-up

**Goal:** Make filter/layout changes recordable, expose version state and navigation in the data workspace, simplify diffs, and make aggregation roles explicit without adding dependencies or backend state.

**Constraints:** Preserve metadata-only browser versioning, existing DuckDB APIs, Atomic Design, and the current `feat/versioning-engine` branch. No backend changes unless live verification proves they are necessary.

## Task 1 — Record every filter entry path

**Files:** `frontend/src/App.svelte`

- Route inspector filters, category filters, categorical-cell filters, filter removal, and clear-conditions through one small apply-and-stage path.
- Stage only after the refreshed query succeeds.
- Keep existing hide/show and reorder staging; verify those paths rather than rewriting them.

**Check:** `npm run check`; live apply/remove/cell-filter smoke confirms pending count increments and finalization replays the filtered SQL.

## Task 2 — Explicit aggregate/index roles

**Files:** `frontend/tests/aggregate-sql.test.js`, `frontend/src/lib/aggregate-sql.ts`, `frontend/src/App.svelte`, `frontend/src/components/organisms/AggregateMenuPopover.svelte`; add one focused molecule only if required for accessible interactive pills.

- RED: test grouped aggregation where aggregate fields are not position-dependent and multiple aggregate fields have collision-free output aliases.
- Replace the native add-column select with the existing custom `Checkbox` list pattern; initial selection is empty and the list has a bounded scroll height.
- First selected field defaults to Aggregate; later selections default to Index.
- Double-clicking an Index promotes it to Aggregate and focuses its metric controls; clicking an Aggregate changes metric focus.
- Store metrics per aggregate field and generate SQL from explicit indexes plus aggregate fields.

**Check:** focused aggregate SQL test, `npm test`, `npm run check`.

## Task 3 — Compact version workspace UI

**Files:** `frontend/src/App.svelte`, `frontend/src/components/organisms/DatasetHead.svelte`, `frontend/src/components/organisms/VersionDiffDialog.svelte`.

- Show `Version N of M` below the selected sheet name.
- Add compact previous/next and square Stop controls using the existing `IconButton` atom, with labels/tooltips and disabled boundary states.
- Add ambient, pointer-transparent vertical recording rails to both sides of the visible table.
- Make the diff dialog open in a human summary mode; expose SQL as a deliberate second mode instead of showing it first.

**Check:** `npm run check && npm run build`; live navigate versions, finalize recording, inspect default diff and SQL mode, verify keyboard focus and browser console.

## Task 4 — Integration and review

- Run `uv run pytest -q` from the repo root.
- Run `npm test && npm run check && npm run build` from `frontend/`.
- Rebuild/relaunch Docker only after preserving mounted state, then verify a real CSV workflow in the browser.
- Run spec-compliance review first, then code-quality review; fix all critical/important findings and repeat checks.
