# Expanded table mode

## Scope
Implement the requested table-only expanded view without API changes or new dependencies.

## Task — table viewport and density controls
**Files:** `frontend/src/App.svelte`, `frontend/src/app.css`

- Add a small table toolbar with Compact, Default, and Comfortable row-height buttons.
- Add an Expand table button; while expanded, the existing data stage covers the viewport and the control becomes Back.
- Reuse the current table component, query/result state, header sort/filter/hide/profile actions, inspector, and pagination. Do not copy data or rerun a query when entering/exiting.
- Escape closes the inspector/SQL first, then exits expanded mode.

## Acceptance
- Entering and exiting keeps the same rows, page, filters, sorts, dedupe keys, hidden columns, and scrollable table.
- Density buttons change body row height only and identify the active setting.
- Existing per-column controls and profile inspector work in expanded mode.
- `npm run check && npm run build` passes in `frontend/`.
- Live smoke: expand/back, density changes, sort, and profile all work with no console errors.
