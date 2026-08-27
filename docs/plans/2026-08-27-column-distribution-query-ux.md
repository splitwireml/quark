# Column distribution and compact query UX implementation plan

> **For Hermes:** Use subagent-driven-development skill to implement this plan task-by-task.

**Goal:** Make Quark’s filtered column profiles readable as counts, percentages, or cumulative distributions; compact applied-query chips; make column visibility type-aware; and replace Boolean free-text filtering with a simple choice.

**Architecture:** Keep the existing FastAPI/DuckDB profile contract unchanged. `POST /columns/{column}/stats` already compiles the same validated filter/dedupe relation used by the table, and `App.svelte` already sends the applied builder query. Do all requested rendering and view-state work in the existing Svelte screen: no new endpoint, component, dependency, or frontend test runner.

**Tech Stack:** FastAPI, DuckDB, Svelte 5, TypeScript, pytest, Vite.

**Worktree rule:** The repository is already dirty. Do not stage, commit, reset, revert, or overwrite unrelated edits. Only modify the owned files below.

---

## Existing contract to preserve

- `backend/app.py:537-645` computes categorical top values and date year counts from `filtered_relation(...)`.
- `frontend/src/App.svelte:504-521` posts `{ filters, sorts, dedupe_columns }` when opening a profile.
- Percent denominator is `stats.non_null_count`, not all rows. Nulls remain reported separately.
- Cumulative values follow the server response order: categorical values are descending frequency; dates are ascending year. A categorical top-20 list may not reach 100% when unshown categories remain.
- Raw SQL has no profile endpoint. Its requested compact chip is an `SQL` label with the active SQL as its native tooltip.

## Task 1: Preserve and prove the filter-aware profile contract

**Objective:** Verify that the feature extends the existing canonical filtered relation rather than duplicating or weakening it.

**Files:**
- Read: `backend/app.py:264-302,535-645`
- Read/Test: `tests/test_backend.py:742-774`
- Modify: none

**Step 1: Run the focused regression test**

Run:
```bash
uv run pytest tests/test_backend.py::test_filtered_deduped_query_metadata_and_stats_share_rows -q
```

Expected: `1 passed`.

**Step 2: Keep the existing API shape**

Do not add a distribution endpoint or percentage fields. The browser can derive each display value from the existing `{count, non_null_count}` response.

**Step 3: Run the backend suite**

Run:
```bash
uv run pytest -q
```

Expected: all tests pass.

## Task 2: Add profile display controls without changing profile data

**Objective:** In the existing side inspector, let categorical (including Boolean) and date distributions switch between count/percentage and normal/cumulative values.

**Files owned:**
- Modify: `frontend/src/App.svelte`
- Modify: `frontend/src/app.css`

**Step 1: Establish the missing UI behavior**

Current categorical and date profile rows always render `count(value.count)` / `count(year.count)` and expose no mode control. Confirm this in `App.svelte:685-690` before editing.

**Step 2: Add minimal inspector state and pure helpers**

Add a local display mode and cumulative flag beside the existing inspector state:

```ts
type DistributionMode = 'count' | 'percent';
let distributionMode = $state<DistributionMode>('count');
let cumulativeDistribution = $state(false);
```

Use one helper for rows shaped as `{ count: AggregateCount }`:

```ts
function distributionText(value: AggregateCount, values: { count: AggregateCount }[], index: number, total: AggregateCount): string {
  const amount = values.slice(0, cumulativeDistribution ? index + 1 : index + 1)
    .reduce((sum, item) => cumulativeDistribution ? sum + Number(item.count) : Number(value), 0);
  return distributionMode === 'percent'
    ? `${total === 0 ? 0 : (amount * 100 / Number(total)).toFixed(1)}%`
    : count(cumulativeDistribution ? amount : value);
}
```

Keep the real implementation shorter/clearer if possible; do not create a new utility module for this one screen. Reset both controls to count/non-cumulative when a profile opens.

**Step 3: Render native controls and the value labels**

Above categorical `top_values` and date `year_counts`, add a compact button group:

```svelte
<div class="distribution-controls" role="group" aria-label="Distribution display">
  <button class="secondary-button" aria-pressed={distributionMode === 'count'} onclick={() => distributionMode = 'count'}>Count</button>
  <button class="secondary-button" aria-pressed={distributionMode === 'percent'} onclick={() => distributionMode = 'percent'}>Percent</button>
  <button class="secondary-button" aria-pressed={cumulativeDistribution} onclick={() => cumulativeDistribution = !cumulativeDistribution}>Cumulative</button>
</div>
```

Replace each right-hand count with the helper’s output. Keep the original value/year labels, null completeness row, native focus behavior, and empty states.

**Step 4: Add only the CSS needed for readable controls**

Add a small flex/wrap rule scoped to `.distribution-controls`; reuse `.secondary-button` and `[aria-pressed="true"]` styling already present. Do not introduce charts, new visual components, or libraries.

**Step 5: Verify frontend compilation**

Run:
```bash
cd frontend && npm run check && npm run build
```

Expected: zero Svelte errors/warnings and a production bundle.

## Task 3: Compact query chips and type-based column visibility

**Objective:** Stop large selected-category values from stretching the query bar, expose full SQL on hover, show data types in the Columns list, and allow visibility selection by type.

**Files owned:**
- Modify: `frontend/src/App.svelte`
- Modify: `frontend/src/app.css`

**Step 1: Make applied chips short and inspectable**

In the existing `filters` token loop around `App.svelte:655`:

- Render only `filter.column` as the visible chip label.
- Set its native `title` to `result?.sql` (the backend-generated executable SQL for all clicked filters).
- Retain the existing remove button and its accessible label.
- In SQL result mode replace the plain `SQL result` text with a compact `SQL` chip whose native `title` is `activeSql`.

Do not parse SQL, copy filter values into custom popovers, or add a tooltip dependency.

**Step 2: Show types and select visibility by type**

In the existing Columns menu around `App.svelte:652`:

- Display `column.type` beside its null percentage in every row.
- Derive the distinct current DuckDB type strings from `result?.columns`.
- Add native type checkboxes. Toggling them applies one type selection to `hiddenColumns`: selected types are visible, unselected types are hidden, dedupe keys remain visible, and at least one column remains visible.
- `Show all columns` restores all columns and resets the type selection.
- Preserve individual column checkboxes and all pre-existing nullity actions.

Use existing local `hiddenColumns`, `visibleColumns`, and `isColumnProtected(...)`; do not add a persisted preference, backend field, or separate state store.

**Step 3: Add a scoped layout rule only if needed**

Use the existing menu panel. Add CSS only for a compact, wrapping type-checkbox row; keep the menu scrollable and mobile-safe.

**Step 4: Verify frontend compilation**

Run:
```bash
cd frontend && npm run check && npm run build
```

Expected: zero Svelte errors/warnings and a production bundle.

## Task 4: Use a Boolean selector instead of a free-text filter

**Objective:** Make Boolean filters pickable without requiring users to type `true` or `false`.

**Files owned:**
- Modify: `frontend/src/App.svelte`

**Step 1: Preserve the request contract**

- Add a small `isBooleanType(...)` predicate next to the existing type helpers.
- In `addFilter()`, convert the selected text to an actual Boolean before it enters `FilterCondition.value`.
- Keep `is_null` / `not_null` available through the existing advanced condition path; do not add an API special case.

**Step 2: Replace the Boolean text box**

For a Boolean column's direct filter form, render a native `<select>` with `True` and `False`, then an Apply button. Do not render a text input or force users to type SQL spellings.

**Step 3: Verify**

Run:
```bash
cd frontend && npm run check && npm run build
```

Expected: zero Svelte errors/warnings and a production bundle.

## Task 5: Vertical verification and review

**Objective:** Exercise the live app against a real local source and protect the existing dirty worktree.

**Files:**
- Modify: none unless a focused verification defect requires the smallest correction in the Task 2/3 owned files.

**Step 1: Run all static checks**

```bash
uv run pytest -q
cd frontend && npm run check && npm run build
```

**Step 2: Rebuild and verify the local service**

Use the repository’s existing launch path. Confirm the backend serves the rebuilt `frontend/dist` and `GET /` responds successfully.

**Step 3: Browser smoke flow**

1. Load a small source with a categorical field, Boolean field, and date field.
2. Apply an `IN` category filter with several values; verify its chip only shows the column name and its native tooltip contains executable SQL.
3. Open each profile after filtering; verify row totals/distributions reflect the filtered subset.
4. Toggle Percent, then Cumulative; verify Boolean and date labels update without a second backend call.
5. Open Columns; verify each row shows its DuckDB type, uncheck one type, and verify matching table columns hide without hiding a protected/last column. Use Show all to reverse it.
6. Run a SQL query; verify the `SQL` chip remains compact and its native tooltip contains the active SQL.

**Step 4: Review gates**

- Spec review first: exact requested controls, subset semantics, compact chips, type selector, no backend duplication.
- Code-quality review second: Svelte reactivity, accessibility, preserved dirty work, no new dependency/scope creep.
- Do not commit; report the final diff and verification evidence.
