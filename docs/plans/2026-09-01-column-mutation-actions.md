# Column mutation actions

## Scope
Frontend only. Reuse the current SQL projection engine, FormulaMenu, and DataGrid context menu. No dependencies, backend changes, or commits.

## Task 1 — SQL projection helpers
**Files:** `frontend/src/lib/mutation-sql.ts`, `frontend/tests/mutation-sql.test.js`

Add failing tests, then the minimum helpers for replacing/renaming a column and choosing a case-insensitive `<name>_2`, `<name>_3` duplicate name.

## Task 2 — Formula helpers and column actions
**Files:** `frontend/src/components/organisms/FormulaMenu.svelte`, `frontend/src/components/organisms/DataGridTable.svelte`, `frontend/src/App.svelte`

- Text: `length` and safe `to number` (`try_cast(... AS DOUBLE)`).
- Logical: `TRUE`, `FALSE`, `IS NULL`, `IS NOT NULL`; keep existing comparisons and `AND`/`OR`/`NOT`.
- Modify: open the same FormulaMenu with the chosen column preselected and replace that column in place.
- Duplicate: context-menu action applies immediately with `_2`, `_3`, etc.
- Rename: context-menu action uses a native prompt, validates nonblank/case-insensitive uniqueness, then applies immediately.
- Keep controls in the existing right-click menu so headers remain uncluttered.

## Acceptance
1. New mutation SQL tests fail before implementation and pass after it.
2. `npm run test:mutation && npm run check && npm run build` passes.
3. `uv run pytest -q` passes.
4. Rebuild/restart Docker Compose and smoke the running API/UI without losing persisted data.
