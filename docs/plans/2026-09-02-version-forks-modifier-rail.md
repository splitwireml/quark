# Version forks and modifier rail

## Constraints

- Preserve the existing dirty worktree; no stash/reset/checkout/commit.
- Reuse the current `ViewHistory`, `Version`, `FilterCondition`, `Chip`, and query pipeline.
- Atomic Design: keep toolbar composition in `QueryConditionBar`; only extend existing atoms when needed.
- No new dependencies.

## Task 1 — strict version branches

**Files:**
- `frontend/src/lib/types.ts`
- `frontend/src/lib/versioning.ts`
- `frontend/tests/versioning.test.js`
- `frontend/src/App.svelte`
- `frontend/src/components/organisms/SourceRail.svelte`
- `frontend/src/components/organisms/VersionsViewsPane.svelte`
- `frontend/src/components/molecules/ViewTreeItem.svelte`

**Behavior:**
- A child version is always `parent.number + 1`.
- Saving from an older parent reuses an identical existing child; a different snapshot creates the next fork (`f2 v2`, then `f3 v2`).
- Descendants retain their fork label (`f2 v3`).
- Stored linear histories remain valid and migrate without labels changing.
- Previous follows `parentId`; next is enabled only when the active version has exactly one child.

**Checks:** focused `versioning.test.js`, then frontend test/check.

## Task 2 — boolean filters and modifier rail

**Files:**
- `backend/app.py`
- `tests/test_backend.py`
- `frontend/src/lib/types.ts`
- `frontend/src/App.svelte`
- `frontend/src/components/organisms/QueryConditionBar.svelte`
- `frontend/src/components/atoms/Chip.svelte`

**Behavior:**
- Each filter after the first carries a connector to the preceding filter, default `and`, toggleable to `or`.
- Backend validates and folds the connectors left-to-right in the WHERE expression for dataset and SQL-result queries.
- Filter and sort chips use distinct tones; small accessible AND/OR buttons appear only between adjacent filters.
- Filter/sort chips live in a separated, calculated-width, single-line horizontal rail; dedupe remains outside it.
- The rail hides its scrollbar. A vertical-only wheel gesture scrolls horizontally; native horizontal trackpad input is unchanged.
- Remove the SQL toolbar button and any Cmd-K opening behavior; move Find column to the far right.

**Checks:** focused backend OR-filter test, frontend check/build, and live smoke in browser.

## Integration gate

Run full frontend tests/check/build, full backend tests, inspect the exact diff, rebuild the Docker service, and verify the live AND/OR API response. Browser smoke was explicitly skipped by Ali.
