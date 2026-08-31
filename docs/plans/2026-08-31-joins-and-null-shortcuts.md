# Joins and null shortcuts

## Constraints
- Base the work on current `origin/main` and obey the repo's Atomic Design rule.
- Preserve the existing dirty changes in `backend/app.py` and `tests/test_backend.py` without restoring the pre-redesign monolithic frontend.
- No new dependency, backend endpoint, database mutation, or commit.
- Reuse the existing read-only SQL endpoint and browser-local saved-query storage.

## Task 1 — join SQL contract
Ownership: `frontend/src/lib/join-sql.ts`, `frontend/tests/join-sql.test.js`

- Build one read-only DuckDB `INNER JOIN` statement between the selected current dataset and one other dataset in the same source.
- Support one or more equality key pairs.
- Select user-chosen columns from each side.
- Quote all identifiers and make duplicate output names unambiguous.
- Reject incomplete keys and an empty output selection.
- Check: run the new Node test red before implementation, then green.

## Task 2 — join menu and saved view
Ownership: `frontend/src/App.svelte`, `frontend/src/components/organisms/JoinMenuPopover.svelte`, `frontend/src/components/organisms/QueryConditionBar.svelte`

- Add a `Join` query menu in builder mode.
- Select another dataset/sheet, add/remove multiple left/right key pairs, choose output columns per dataset, and provide All/None controls for each side.
- Run the generated join through the existing SQL query flow.
- Offer a `Save view` checkbox and optional name; persist through existing localStorage saved queries.
- Keep native controls and existing query-menu behavior.

## Task 3 — categorical null shortcuts
Ownership: `frontend/src/App.svelte`, `frontend/src/components/organisms/FilterInspector.svelte`

- Put `Is null` and `Isn't null` actions directly in the categorical picker, outside Advanced condition.
- Reuse the existing filter request path.

## Task 4 — column-header quality of life
Ownership: `frontend/src/App.svelte`, `frontend/src/components/molecules/ColumnHeaderCell.svelte`, `frontend/src/components/organisms/DataGridTable.svelte`, `frontend/src/components/organisms/QueryConditionBar.svelte`

- Right-clicking a column header opens a larger native-style context menu with the same available actions as its header buttons: sort, filter, profile when supported, and hide when allowed.
- Header action hover tooltips are one word: `Sort`, `Filter`, `Profile`, `Hide`.
- Filter-chip hover text is a short readable condition summary, never the full generated SQL query.
- Close the context menu after an action, on outside click, and on Escape.

## Verification
- `node --test frontend/tests/*.test.js`
- `cd frontend && npm run check && npm run build`
- `uv run pytest -q`
- Rebuild/relaunch Docker Compose, verify readiness, query a real two-dataset fixture, and exercise the Join menu plus both null shortcuts in the live browser.
