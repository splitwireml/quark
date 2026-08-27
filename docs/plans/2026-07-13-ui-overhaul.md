# DuckScope UI Overhaul Implementation Plan

> **For Hermes:** Implement task by task. Keep the existing API and Svelte state flow. No new runtime dependencies.

**Goal:** Make DuckScope readable, responsive, and easier to operate without changing its backend contract or analytical capabilities.

**Architecture:** Recompose the existing single-page Svelte interface into a responsive source rail, dominant data canvas, and contextual inspector. Reuse current query, filter, sort, pagination, and statistics functions. Keep the shortest viable production diff in `App.svelte` and `app.css`; extract a component only if repeated markup makes the final code smaller or clearer.

**Tech stack:** Svelte 5, TypeScript, plain CSS, Material Symbols Outlined, existing FastAPI JSON API.

**Reference:** `PRODUCT.md`, `DESIGN.md`, `docs/UI_UX_EVALUATION.md`, `docs/ui-overhaul-prototype/index.html`.

---

## Acceptance baseline

Before changing UI code:

1. Run `npm run check && npm run build` from `frontend/`.
2. Run `pytest -q` from the project root.
3. Capture desktop, tablet, and mobile screenshots of the existing live app.
4. Record core flow checks: select source, select dataset, sort twice and clear, apply/remove category filter, apply/remove advanced filter, open/close numeric profile, change page size, jump page, next/previous page.

Expected: current checks pass; baseline screenshots show the documented 720 px mobile overflow.

## Task 1: Establish tokens and readable density

**Objective:** Replace one-off styling with a small design vocabulary before moving layout.

**Files:**
- Modify: `frontend/src/app.css`

**Steps:**

1. Add the color, type, spacing, radius, and control-size tokens from `DESIGN.md` under `:root`.
2. Replace hard-coded surface, text, rule, accent, warning, and error colors with those tokens.
3. Set 14 px default UI text, 12 px minimum metadata/table text, 34 to 36 px desktop rows, and 36 px pointer controls.
4. Keep 4 px control radii and 2 px condition-token radii. Remove pill styling.
5. Preserve visible focus indicators and reduced-motion behavior.
6. Run `npm run check && npm run build`.
7. Verify common text contrast reaches WCAG AA.

**Commit:** `style: establish readable DuckScope design tokens`

## Task 2: Simplify the shell and source navigation

**Objective:** Give routine analysis more room and remove duplicate navigation.

**Files:**
- Modify: `frontend/src/App.svelte`
- Modify: `frontend/src/app.css`

**Steps:**

1. Keep source selection only in the left rail; remove duplicate top node tabs.
2. Replace the top tabs with a compact current-source breadcrumb and one connection indicator.
3. Replace permanent upload and attach controls with one **Add source** action that reveals the existing controls on demand.
4. Keep upload and attach functions unchanged.
5. Preserve empty, mutation-loading, and error states.
6. Verify source selection, upload, attach, and recovery behavior.
7. Run `npm run check && npm run build`.

**Commit:** `refactor: simplify source navigation and setup`

## Task 3: Rebuild the data toolbar and table header

**Objective:** Make query state and column actions legible without reducing table density.

**Files:**
- Modify: `frontend/src/App.svelte`
- Modify: `frontend/src/app.css`

**Steps:**

1. Place dataset name, row count, query time, and refresh in one compact header.
2. Keep dataset tabs only when more than one dataset exists; otherwise show the current dataset as a breadcrumb segment.
3. Replace “No filters or sorts applied” with explicit **Filter** and **Sort** actions plus active condition tokens.
4. Add one **Clear query** action when filters or sorts exist.
5. Standardize each column header: name, type, null rate, sort action, filter action, and explicit numeric profile action.
6. Keep sticky header, sticky first column, null tokens, nullity gauge, and table-level horizontal/vertical scrolling.
7. Verify ordered multi-sort and repeated filter removal still use current functions.
8. Run `npm run check && npm run build`.

**Commit:** `refactor: clarify query toolbar and column actions`

## Task 4: Move filters into a contextual inspector

**Objective:** Let users build a filter without losing table context.

**Files:**
- Modify: `frontend/src/App.svelte`
- Modify: `frontend/src/app.css`

**Steps:**

1. Render the current filter state in a right-side inspector on wide screens.
2. Lead text columns with category search, counts, visible selection, and one Apply action.
3. Put operator/value controls behind an **Advanced condition** disclosure for text columns.
4. Show operator/value controls directly for numeric and ordered columns.
5. Auto-focus category search or value input after the inspector opens.
6. Keep `Escape`, Cancel, and close-button behavior; return focus to the invoking column action.
7. Keep current category and advanced-filter API calls and stale-request guards.
8. Verify apply, cancel, clear selection, search, request error, and loading states.
9. Run `npm run check && npm run build`.

**Commit:** `refactor: move filters into contextual inspector`

## Task 5: Move column profiling into the same inspector

**Objective:** Make numeric profiling discoverable and keep the table visible.

**Files:**
- Modify: `frontend/src/App.svelte`
- Modify: `frontend/src/app.css`

**Steps:**

1. Replace the statistics modal with the same right-side inspector region.
2. Keep the existing statistics request and stale state reset.
3. Replace the eight equal metric cards with a compact definition list grouped as completeness, range, center, and spread.
4. Keep the histogram, add visible count context, and expose the selected or focused bin range without relying on hover.
5. Label **Profile column** explicitly in numeric column actions.
6. Verify loading, error, empty histogram, keyboard close, and focus return.
7. Run `npm run check && npm run build`.

**Commit:** `refactor: integrate column profiles with inspector`

## Task 6: Make the structure responsive

**Objective:** Keep core inspection possible at tablet and mobile widths.

**Files:**
- Modify: `frontend/src/App.svelte`
- Modify: `frontend/src/app.css`

**Steps:**

1. Delete `body { min-width: 720px; }` and prevent page-level horizontal overflow.
2. At widths below 960 px, move the source rail into a drawer opened by a labeled menu button.
3. At widths below 960 px, render the inspector as a full-width overlay with a visible close action.
4. Keep horizontal overflow inside `.table-scroll`, not on the document.
5. Use 44 px minimum touch targets and 14 px minimum non-data text at the touch breakpoint.
6. Keep pagination usable: row count and previous/next remain visible; page-size and direct jump can wrap to a second line.
7. Verify at 390 x 844, 900 x 900, 960 x 900, and 1440 x 900.
8. Run `npm run check && npm run build`.

**Commit:** `fix: make DuckScope responsive and touch-safe`

## Task 7: Harden accessibility and state feedback

**Objective:** Finish the interaction quality, not just the visuals.

**Files:**
- Modify: `frontend/src/App.svelte`
- Modify: `frontend/src/app.css`

**Steps:**

1. Add accessible names and tooltips to every icon-only table action.
2. Ensure tab roles and `aria-selected` remain valid only where true tabs remain.
3. Announce query updates and result counts through a polite live region.
4. Preserve current rows during refresh and use skeleton rows or a non-blocking progress indicator.
5. Confirm logical focus order, focus return from inspector/drawer, and no keyboard trap.
6. Test 200% text zoom and reduced motion.
7. Run `npm run check && npm run build`.

**Commit:** `fix: harden DuckScope accessibility and feedback`

## Task 8: Final verification

1. Run `pytest -q`.
2. Run `npm run check && npm run build` from `frontend/`.
3. Repeat every core flow from the acceptance baseline.
4. Capture desktop, tablet, and mobile screenshots.
5. Confirm at 390 px: document width equals viewport width, rail is closed by default, table scroll is local, and no primary action is clipped.
6. Confirm no console errors during load, source selection, filter apply/remove, sort cycle, profile open/close, refresh, and pagination.
7. Run independent design critique and polish passes before merging.

**Commit:** `test: verify DuckScope UI overhaul`

## Explicit non-goals

- No backend or API redesign.
- No arbitrary SQL console.
- No editable grid.
- No theme switcher until the daylight direction is validated.
- No new component library, data-grid package, chart library, or state store.
