# DuckScope table usability fixes

## Scope
Fix the six reported UI defects without introducing dependencies or changing the API.

## Task 1 — Source rail and column selector layout
**Files:** `frontend/src/App.svelte`, `frontend/src/app.css`

- Make the expanded Add source controls stay inside the rail.
- Give the Columns dropdown a constrained, scrollable body so every column remains reachable.
- Keep full column names available through a non-competing header treatment; use native hover/title rather than an animation.

**Acceptance:** narrow rail has no horizontal overflow; a long column list scrolls inside its popover; header action buttons never overlap the column title.

## Task 2 — Cell expansion and column navigation
**Files:** `frontend/src/App.svelte`, `frontend/src/app.css`

- Store a selected cell by stable row index + column name.
- Double-click a clipped data cell to expand only that cell vertically; single click the same expanded cell to collapse it. No sort/filter/column state is mutated.
- Add an incremental column-name search in the query bar. Match case-insensitively, then horizontally scroll the existing `.table-scroll` to the matching header and focus it.

**Acceptance:** expanding/collapsing a cell leaves visible column order unchanged; typing each search character updates the current match and scrolls it into view; no table-wide reordering.

## Task 3 — Numeric filter readability
**Files:** `frontend/src/App.svelte`

- Normalize numeric filter text on blur and before apply: remove grouping punctuation, preserve decimals/negative values, then reformat with locale grouping. The submitted filter value remains an unambiguous plain numeric string.

**Acceptance:** entering `1000000`, `1,000,000`, or spaced grouping displays a human-readable localized number and submits `1000000`; invalid numeric text remains editable and is not silently changed.

## Verification
1. `cd frontend && npm run check && npm run build`
2. Rebuild served static assets, start DuckScope, and verify `GET /` is HTTP 200.
3. Smoke the rendered page with a wide multi-column CSV: open Add source, open and scroll Columns, search a right-side column, expand/collapse a clipped cell, and apply a numeric filter.
