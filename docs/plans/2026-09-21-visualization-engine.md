# Visualization Engine Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give the visualization engine user-controlled encoding roles, series-aware bar charts, real scatter scales for size/color/shape, a date-axis line chart with time-grain controls, a pie chart, and grouped histograms.

**Architecture:** Encodings stay inferred by `suggestCharts` as the default; a new pure `applyRoles` layer lets the user override one role per column, and `chartRoles` constrains which roles each chart accepts. The backend gains series-aware bar SQL, a line branch with automatic time-bucketing, scatter scale metadata computed over the whole filtered relation, a pie variant of the bar query, and grouped histogram counts sharing the ungrouped profile's bin edges. Every new response field is additive and optional, so existing consumers keep working untouched.

**Tech Stack:** Svelte 5 runes, TypeScript, `node:test` with `--experimental-strip-types` for frontend lib tests, FastAPI + Pydantic + DuckDB for the backend, pytest + `TestClient` for backend tests. No new dependency in either layer.

**Spec:** `docs/plans/2026-09-21-visualization-engine-design.md`

## Global Constraints

- No new runtime dependency in `frontend/package.json` or `pyproject.toml`.
- Backend `ChartSpec` keeps `model_config = ConfigDict(extra="forbid")`. An unknown spec field must stay a 422.
- Every new backend response field is optional. Existing ungrouped responses must serialize byte-identically to today, so no existing frontend consumer breaks.
- Follow Atomic Design: primitives in `atoms/`, composites in `molecules/`, page composition in `organisms/`. Do not duplicate an existing component.
- Every new interaction gets a `@media (prefers-reduced-motion: reduce)` path that applies the end state directly.
- `SERIES_LIMIT = 6` everywhere series are capped — it matches the `series` array length in every palette in `frontend/src/lib/chartThemes.ts`.
- Series and categories beyond a cap fold into a single `Other` bucket. `Other` is never clickable for filtering.
- Frontend tests run with `cd frontend && npm test`. Backend tests run with `uv run pytest tests/test_backend.py`.
- End every commit message with:

```
Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>
```

---

## File Structure

**Created:**

| File | Responsibility |
|---|---|
| `frontend/src/components/molecules/ChipToggleGroup.svelte` | The chip-row toggle used by metric, bar layout, and time grain. Replaces inline `.metric-chip` markup. |
| `frontend/src/components/molecules/RoleChip.svelte` | A selected column's chip that expands in place into its role list. |
| `frontend/src/components/molecules/ChartLegend.svelte` | One legend for every channel: bar/line/histogram series, scatter categorical color, scatter shape, scatter size, scatter continuous color. |
| `frontend/src/components/molecules/LineChart.svelte` | Multi-series line over a date axis. |
| `frontend/src/components/molecules/PieChart.svelte` | Slices from bar-shaped rows, click-to-filter. |

**Modified:**

| File | Change |
|---|---|
| `frontend/src/lib/types.ts` | `ChartType` gains `pie`; new `EncodingRole`, `BarLayout`, `TimeGrain`; `ChartSpec` gains `layout`/`grain`; response shapes gain series and scale fields; `ChartMark` category gains `series`. |
| `frontend/src/lib/visualize.ts` | `chartRoles`, `applyRoles`, `autoGrain`, `barLayouts`, `timeGrains`, `pie` in `chartMeta`, drop numeric-x line suggestion, `filtersFromMark` series condition. |
| `frontend/src/lib/icons.ts`, `frontend/src/components/atoms/Icon.svelte` | `pie` icon. |
| `frontend/src/lib/dashboard.ts` | `chartTitle` includes group/size/color/shape (Task 6). |
| `frontend/src/components/molecules/ChartOptionsPane.svelte` | Role chips, layout toggle, grain buttons, `ChipToggleGroup` adoption. |
| `frontend/src/components/molecules/BarChart.svelte` | Series bands, stacking, 100% normalization. |
| `frontend/src/components/molecules/ScatterPlot.svelte` | Size, shape, continuous color, legends. |
| `frontend/src/components/molecules/HistogramPlot.svelte` | Overlaid series. |
| `frontend/src/components/molecules/ChartView.svelte` | `line` and `pie` branches, legend placement, updated empty-state copy. |
| `frontend/src/components/organisms/VisualizeStage.svelte` | Pass-through props, `line`/`pie` status lines, updated empty-state copy. |
| `frontend/src/components/organisms/DashboardStage.svelte` | Role/layout/grain pass-through, drop the `item.chart !== 'line'` filter. |
| `frontend/src/App.svelte` | `visualizeRoles`/`visualizeLayout`/`visualizeGrain` state, `implementedCharts` gains `line` and `pie`. |
| `backend/app.py` | `ChartSpec` fields, new limits, grouped bar, line branch, scatter scales, pie, grouped histogram. |
| `frontend/tests/visualize.test.js` | Tests for every new pure helper. |
| `frontend/tests/dashboard.test.js` | Chart title covers the new channels. |
| `tests/test_backend.py` | Tests for every new backend branch. |

**Task order:** Task 1 and Task 2 are the foundation — everything else depends on them. Tasks 3–10 are independent of each other and may be done in any order once Task 2 lands.

---

## Task 1: Spec model and role resolution

**Objective:** Add the types and the two pure functions that turn inferred encodings plus user overrides into a final `ChartSpec`.

**Files:**
- Modify: `frontend/src/lib/types.ts`
- Modify: `frontend/src/lib/visualize.ts`
- Modify: `frontend/tests/visualize.test.js`

**Interfaces:**
- Consumes: nothing from earlier tasks.
- Produces:
  - `type EncodingRole = keyof ChartEncodings`
  - `type BarLayout = 'grouped' | 'stacked' | 'stacked100'`
  - `type TimeGrain = 'hour' | 'day' | 'week' | 'month' | 'quarter' | 'year'`
  - `ChartSpec.layout?: BarLayout`, `ChartSpec.grain?: TimeGrain`
  - `chartRoles(chart: ChartType): EncodingRole[]`
  - `applyRoles(encodings: ChartEncodings, chart: ChartType, overrides: Record<string, EncodingRole>, selected: ColumnInfo[]): ChartEncodings`
  - `barLayouts: { value: BarLayout; label: string; tip: string }[]`
  - `timeGrains: { value: TimeGrain; label: string; tip: string }[]`

- [ ] **Step 1: Write the failing tests**

Append to `frontend/tests/visualize.test.js`, and add `applyRoles` and `chartRoles` to the import list at the top of that file:

```js
test('chartRoles lists only the roles each chart accepts', () => {
  assert.deepEqual(chartRoles('bar'), ['category', 'value', 'group']);
  assert.deepEqual(chartRoles('pie'), ['category', 'value']);
  assert.deepEqual(chartRoles('scatter'), ['x', 'y', 'size', 'color', 'pattern']);
  assert.deepEqual(chartRoles('line'), ['x', 'y', 'group']);
  assert.deepEqual(chartRoles('histogram'), ['value', 'group']);
  assert.deepEqual(chartRoles('box'), ['value', 'group']);
});

test('applyRoles swaps two columns between roles', () => {
  const encodings = { x: 'price', y: 'amount' };
  const result = applyRoles(encodings, 'scatter', { price: 'y', amount: 'x' }, [price, amount]);
  assert.deepEqual(result, { y: 'price', x: 'amount' });
});

test('applyRoles displaces the previous occupant of a role to unassigned', () => {
  const result = applyRoles({ x: 'price', y: 'amount' }, 'scatter', { price: 'y' }, [price, amount]);
  assert.deepEqual(result, { y: 'price' });
});

test('applyRoles leaves untouched roles to inference', () => {
  const result = applyRoles({ x: 'price', y: 'amount', color: 'region' }, 'scatter', { region: 'pattern' }, [price, amount, region]);
  assert.deepEqual(result, { x: 'price', y: 'amount', pattern: 'region' });
});

test('applyRoles drops an override whose column left the selection', () => {
  const result = applyRoles({ x: 'price', y: 'amount' }, 'scatter', { region: 'color' }, [price, amount]);
  assert.deepEqual(result, { x: 'price', y: 'amount' });
});

test('applyRoles drops an override the active chart does not accept', () => {
  const result = applyRoles({ category: 'region', value: 'price' }, 'bar', { price: 'size' }, [region, price]);
  assert.deepEqual(result, { category: 'region', value: 'price' });
});
```

- [ ] **Step 2: Run the tests and verify they fail**

```bash
cd frontend && npm test
```

Expected: FAIL — `chartRoles is not a function` / `applyRoles is not a function`.

- [ ] **Step 3: Add the types**

In `frontend/src/lib/types.ts`, change the `ChartType` alias on line 7 and add the new aliases beside it:

```ts
export type ChartType = 'bar' | 'histogram' | 'box' | 'scatter' | 'line' | 'pie';
export type BarLayout = 'grouped' | 'stacked' | 'stacked100';
export type TimeGrain = 'hour' | 'day' | 'week' | 'month' | 'quarter' | 'year';
```

Below the existing `ChartEncodings` interface, add:

```ts
export type EncodingRole = keyof ChartEncodings;
```

Extend `ChartSpec`:

```ts
export interface ChartSpec {
  chart: ChartType;
  encodings: ChartEncodings;
  metric?: AggregateMetric;
  density?: boolean;
  layout?: BarLayout;
  grain?: TimeGrain;
}
```

- [ ] **Step 4: Implement `chartRoles` and `applyRoles`**

In `frontend/src/lib/visualize.ts`, add `ChartEncodings`, `EncodingRole`, `BarLayout`, and `TimeGrain` to the existing `import type { ... } from './types'` block, then append:

```ts
const CHART_ROLES: Record<ChartType, EncodingRole[]> = {
  bar: ['category', 'value', 'group'],
  pie: ['category', 'value'],
  histogram: ['value', 'group'],
  box: ['value', 'group'],
  scatter: ['x', 'y', 'size', 'color', 'pattern'],
  line: ['x', 'y', 'group']
};

export function chartRoles(chart: ChartType): EncodingRole[] {
  return CHART_ROLES[chart];
}

export function applyRoles(
  encodings: ChartEncodings,
  chart: ChartType,
  overrides: Record<string, EncodingRole>,
  selected: ColumnInfo[]
): ChartEncodings {
  const legal = new Set(chartRoles(chart));
  const present = new Set(selected.map((column) => column.name));
  const result: ChartEncodings = { ...encodings };
  for (const [name, role] of Object.entries(overrides)) {
    if (!present.has(name) || !legal.has(role)) continue;
    for (const key of Object.keys(result) as EncodingRole[]) {
      if (result[key] === name) delete result[key];
    }
    result[role] = name;
  }
  return result;
}

export const barLayouts: { value: BarLayout; label: string; tip: string }[] = [
  { value: 'grouped', label: 'Grouped', tip: 'One bar per series, side by side' },
  { value: 'stacked', label: 'Stacked', tip: 'Series stacked into one bar per category' },
  { value: 'stacked100', label: '100%', tip: 'Each bar fills the height, showing shares' }
];

export const timeGrains: { value: TimeGrain; label: string; tip: string }[] = [
  { value: 'hour', label: 'Hour', tip: 'One point per hour' },
  { value: 'day', label: 'Day', tip: 'One point per day' },
  { value: 'week', label: 'Week', tip: 'One point per week' },
  { value: 'month', label: 'Month', tip: 'One point per month' },
  { value: 'quarter', label: 'Quarter', tip: 'One point per quarter' },
  { value: 'year', label: 'Year', tip: 'One point per year' }
];
```

Later overrides win when two columns claim the same role, because `Object.entries` preserves insertion order and the second assignment overwrites the first. The UI only ever writes one override at a time, so this is not reachable from the pane.

- [ ] **Step 5: Run the tests and verify they pass**

```bash
cd frontend && npm test
```

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add frontend/src/lib/types.ts frontend/src/lib/visualize.ts frontend/tests/visualize.test.js
git commit -m "$(printf 'feat(charts): add encoding roles and role override resolution\n\nCo-Authored-By: Claude Opus 5 <noreply@anthropic.com>')"
```

---

## Task 2: Role chips, chip toggle group, and App wiring

**Objective:** Let the user set a column's role from its chip, and make the shared chip-row a real component instead of inline markup.

**Files:**
- Create: `frontend/src/components/molecules/ChipToggleGroup.svelte`
- Create: `frontend/src/components/molecules/RoleChip.svelte`
- Modify: `frontend/src/components/molecules/ChartOptionsPane.svelte`
- Modify: `frontend/src/components/organisms/VisualizeStage.svelte`
- Modify: `frontend/src/components/organisms/DashboardStage.svelte`
- Modify: `frontend/src/App.svelte`

**Interfaces:**
- Consumes: `chartRoles`, `applyRoles` (Task 1).
- Produces:
  - `ChipToggleGroup` props: `{ options: { value: string; label: string; tip?: string }[]; selected: string | null; label: string; onSelect: (value: string) => void }`
  - `RoleChip` props: `{ name: string; role: EncodingRole | null; roles: EncodingRole[]; onSetRole: (role: EncodingRole) => void; onRemove: () => void }`
  - `ChartOptionsPane` gains props `roles: EncodingRole[]`, `roleOf: (name: string) => EncodingRole | null`, `onSetRole: (name: string, role: EncodingRole) => void`
  - `App.svelte` exposes `setVisualizeRole(name: string, role: EncodingRole)`

- [ ] **Step 1: Create `ChipToggleGroup.svelte`**

```svelte
<script lang="ts">
  type Option = { value: string; label: string; tip?: string };
  type Props = {
    options: Option[];
    selected: string | null;
    label: string;
    onSelect: (value: string) => void;
  };
  let { options, selected, label, onSelect }: Props = $props();
</script>

<div class="chips" role="group" aria-label={label}>
  {#each options as option (option.value)}
    <button
      type="button"
      class="chip-toggle"
      class:on={selected === option.value}
      aria-pressed={selected === option.value}
      data-tip={option.tip}
      data-tip-position="top"
      onclick={() => onSelect(option.value)}
    >{option.label}</button>
  {/each}
</div>

<style>
  .chips { display: flex; flex-wrap: wrap; gap: 5px; }
  .chip-toggle {
    height: 26px;
    padding: 0 9px;
    border-radius: var(--radius-sm);
    border: 1px solid var(--control-border);
    background: var(--surface);
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--muted);
  }
  .chip-toggle.on { border-color: var(--ink-fill); background: var(--ink-fill); color: var(--on-fill); }
  .chip-toggle:hover:not(.on) { border-color: var(--faint); color: var(--ink); }
</style>
```

- [ ] **Step 2: Create `RoleChip.svelte`**

The chip itself becomes the role list. A popover would appear beside the chip, which the project's motion rule forbids — the control has to grow out of what the user clicked.

```svelte
<script lang="ts">
  import type { EncodingRole } from '../../lib/types';

  type Props = {
    name: string;
    role: EncodingRole | null;
    roles: EncodingRole[];
    onSetRole: (role: EncodingRole) => void;
    onRemove: () => void;
  };
  let { name, role, roles, onSetRole, onRemove }: Props = $props();
  let open = $state(false);

  const labels: Record<EncodingRole, string> = {
    x: 'X axis', y: 'Y axis', category: 'Category', value: 'Value',
    group: 'Group', size: 'Size', color: 'Color', pattern: 'Shape'
  };

  function pick(next: EncodingRole) {
    onSetRole(next);
    open = false;
  }
</script>

<div class="role-chip" class:open>
  <div class="head">
    <button
      type="button"
      class="label"
      aria-expanded={open}
      title={name}
      onclick={() => open = !open}
    >
      <span class="name">{name}</span>
      <span class="role">{role ? labels[role] : 'unused'}</span>
    </button>
    <button type="button" class="remove" aria-label={`Remove ${name}`} onclick={onRemove}>×</button>
  </div>
  <div class="roles" inert={!open}>
    <div class="roles-inner">
      {#each roles as option (option)}
        <button
          type="button"
          class="role-option"
          class:on={role === option}
          aria-pressed={role === option}
          onclick={() => pick(option)}
        >{labels[option]}</button>
      {/each}
    </div>
  </div>
</div>

<style>
  .role-chip {
    display: grid;
    grid-template-rows: auto 0fr;
    width: 100%;
    border: 1px solid var(--action-tint-border);
    border-radius: var(--radius-sm);
    background: var(--action-tint);
    color: var(--action-dark);
    overflow: hidden;
  }
  .role-chip.open { grid-template-rows: auto 1fr; }
  .head { display: flex; align-items: center; gap: 4px; padding: 0 4px 0 8px; }
  .label {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: baseline;
    gap: 6px;
    height: 24px;
    padding: 0;
    border: 0;
    background: transparent;
    color: inherit;
    text-align: left;
    font-family: var(--font-mono);
    font-size: 11px;
  }
  .name { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .role { flex: none; font-size: 10px; opacity: .7; }
  .remove {
    flex: none;
    width: 18px;
    height: 18px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    border: 0;
    border-radius: 2px;
    background: transparent;
    color: inherit;
    font-size: 12px;
    line-height: 1;
    padding: 0;
    opacity: .55;
  }
  .remove:hover { opacity: 1; background: color-mix(in srgb, currentColor 8%, transparent); }
  .roles { min-height: 0; overflow: hidden; }
  .roles-inner { display: flex; flex-wrap: wrap; gap: 4px; padding: 4px 8px 6px; }
  .role-option {
    height: 22px;
    padding: 0 7px;
    border: 1px solid var(--control-border);
    border-radius: var(--radius-sm);
    background: var(--surface);
    color: var(--ink-2);
    font-family: var(--font-mono);
    font-size: 10.5px;
  }
  .role-option.on { border-color: var(--ink-fill); background: var(--ink-fill); color: var(--on-fill); }

  @media (prefers-reduced-motion: no-preference) {
    .role-chip { transition: grid-template-rows 200ms cubic-bezier(0.22, 1, 0.36, 1); }
  }
</style>
```

- [ ] **Step 3: Rewire `ChartOptionsPane.svelte`**

Add to the `Props` type and the destructuring:

```ts
    roles: EncodingRole[];
    roleOf: (name: string) => EncodingRole | null;
    onSetRole: (name: string, role: EncodingRole) => void;
```

Import `ChipToggleGroup` and `RoleChip`, add `EncodingRole` to the type import, and replace the `Selected` block's chip loop:

```svelte
      {#if selected.length}
        <div class="pane-block">
          <Eyebrow>Selected</Eyebrow>
          <div class="chips">
            {#each selected as column (column.name)}
              <RoleChip
                name={column.name}
                role={roleOf(column.name)}
                {roles}
                onSetRole={(role) => onSetRole(column.name, role)}
                onRemove={() => onToggleColumn(column.name)}
              />
            {/each}
          </div>
        </div>
      {/if}
```

Change `.chips` from a wrapping row to a column, since role chips are full-width:

```css
  .chips { display: flex; flex-direction: column; gap: 4px; }
```

Replace the `Aggregate` block's inline `.metrics` markup with the new component:

```svelte
      {#if showAggregate}
        <div class="pane-block">
          <Eyebrow>Aggregate</Eyebrow>
          <ChipToggleGroup
            label="Aggregation"
            options={visualizeMetrics.map((metric) => ({ value: metric.value, label: metric.label, tip: metric.tip }))}
            selected={spec?.metric ?? null}
            onSelect={(value) => onSelectMetric(value as AggregateMetric)}
          />
        </div>
      {/if}
```

Delete the now-unused `.metrics` and `.metric-chip` rules from the pane's `<style>` block.

- [ ] **Step 4: Wire the state in `App.svelte`**

Beside `visualizeColumns`, add:

```ts
  let visualizeRoles = $state<Record<string, EncodingRole>>({});
```

Add `EncodingRole` and `applyRoles`/`chartRoles` to the existing imports. Change `visualizeSpec` to apply the overrides:

```ts
  let visualizeSpec = $derived.by((): ChartSpec | null => {
    const suggestions = visualizeSuggestions;
    if (!suggestions.length) return null;
    const pick = (visualizeChart && suggestions.find((item) => item.chart === visualizeChart)) || suggestions[0];
    const encodings = applyRoles(pick.encodings, pick.chart, visualizeRoles, visualizeColumns);
    const metric = pick.chart === 'bar' && encodings.value
      ? (visualizeMetric ?? pick.metric ?? 'avg')
      : pick.metric;
    return metric
      ? { chart: pick.chart, encodings, metric }
      : { chart: pick.chart, encodings };
  });
```

Add the setter and the helper the pane needs:

```ts
  function setVisualizeRole(name: string, role: EncodingRole) {
    visualizeRoles = { ...visualizeRoles, [name]: role };
    void loadVisualize();
  }

  function visualizeRoleOf(name: string): EncodingRole | null {
    const encodings = visualizeSpec?.encodings;
    if (!encodings) return null;
    const entry = (Object.entries(encodings) as [EncodingRole, string][]).find(([, value]) => value === name);
    return entry ? entry[0] : null;
  }
```

In `resetVisualizeDraft`, add `visualizeRoles = {};`. In `toggleVisualizeColumn`, drop the override for a column being removed, right before `visualizeSearch = '';`:

```ts
    if (visualizeRoles[name]) {
      const { [name]: _dropped, ...rest } = visualizeRoles;
      visualizeRoles = rest;
    }
```

Pass the three new props to both `VisualizeStage` and `DashboardStage`:

```svelte
                    roles={visualizeSpec ? chartRoles(visualizeSpec.chart) : []}
                    roleOf={visualizeRoleOf} onSetRole={setVisualizeRole}
```

- [ ] **Step 5: Thread the props through both organisms**

Add `roles`, `roleOf`, and `onSetRole` to the `Props` type and the destructuring in `VisualizeStage.svelte`, and forward them to `ChartOptionsPane`. Do the same in `DashboardStage.svelte`, where the forwarded values must respect the edit path:

```svelte
      roles={editingChart ? (editSpec ? chartRoles(editSpec.chart) : []) : roles}
      roleOf={editingChart ? editRoleOf : roleOf}
      onSetRole={editingChart ? setEditRole : onSetRole}
```

Add the matching edit state and helpers to `DashboardStage.svelte`, beside the existing `editType`/`editMetric`:

```ts
  let editRoles = $state<Record<string, EncodingRole>>({});

  function editRoleOf(name: string): EncodingRole | null {
    const encodings = editSpec?.encodings;
    if (!encodings) return null;
    const entry = (Object.entries(encodings) as [EncodingRole, string][]).find(([, value]) => value === name);
    return entry ? entry[0] : null;
  }

  function setEditRole(name: string, role: EncodingRole) {
    editRoles = { ...editRoles, [name]: role };
    saveEdit();
  }
```

and apply them in `editSpec`, replacing the existing spread:

```ts
  let editSpec = $derived.by((): ChartSpec | null => {
    const pick = editSuggestions.find((item) => item.chart === editType) ?? editSuggestions[0];
    if (!pick || !editingChart) return null;
    const encodings = applyRoles(pick.encodings, pick.chart, editRoles, editColumns);
    return { ...editingChart.spec, ...pick, encodings,
      metric: pick.chart === 'bar' && encodings.value ? editMetric ?? pick.metric : pick.metric };
  });
```

Reset `editRoles = {}` wherever `editType` is reset when editing starts or finishes.

- [ ] **Step 6: Typecheck and run the app**

```bash
cd frontend && npx svelte-check --threshold error
```

Expected: no errors.

- [ ] **Step 7: Verify in the browser**

Start the dev server, open a dataset, switch to the chart canvas, select two numeric columns, and confirm: each selected column shows a chip naming its current role; clicking a chip expands the role list inside the chip rather than beside it; picking "Y axis" on the column currently holding X swaps the axes and the chart redraws.

- [ ] **Step 8: Commit**

```bash
git add frontend/src
git commit -m "$(printf 'feat(charts): let a column chip set its encoding role\n\nCo-Authored-By: Claude Opus 5 <noreply@anthropic.com>')"
```

---

## Task 3: Grouped bar backend

**Objective:** Make the bar branch honour `encodings.group`, returning aligned series and values.

**Files:**
- Modify: `backend/app.py:112-130` (`ChartSpec`), `backend/app.py:49-52` (limits), `backend/app.py:1440-1476` (bar branch)
- Modify: `frontend/src/lib/types.ts`
- Test: `tests/test_backend.py`

**Interfaces:**
- Consumes: `BarLayout` (Task 1).
- Produces: `BarVisualizeResponse.series: string[] | null`, `BarVisualizeRow.values?: AggregateCount[]`.

- [ ] **Step 1: Write the failing tests**

Append to `tests/test_backend.py`:

```python
def test_visualize_bar_groups_into_aligned_series(client):
    node = upload(client, "grouped.csv", b"region,quarter,amount\neast,q1,10\neast,q2,20\nwest,q1,30\nwest,q2,40\n")
    base = f"/api/nodes/{node['id']}/datasets/{dataset(client, node, 'grouped')['id']}"
    response = client.post(base + "/visualize", json={
        "spec": {"chart": "bar", "encodings": {"category": "region", "value": "amount", "group": "quarter"}, "metric": "sum"},
    })
    assert response.status_code == 200, response.text
    payload = response.json()
    assert payload["series"] == ["q1", "q2"]
    rows = {row["label"]: row["values"] for row in payload["rows"]}
    assert rows["east"] == [10, 20]
    assert rows["west"] == [30, 40]


def test_visualize_bar_folds_series_beyond_the_cap_into_other(client):
    body = b"region,tag,amount\n" + b"".join(f"east,t{i},{i}\n".encode() for i in range(10))
    node = upload(client, "manyseries.csv", body)
    base = f"/api/nodes/{node['id']}/datasets/{dataset(client, node, 'manyseries')['id']}"
    payload = client.post(base + "/visualize", json={
        "spec": {"chart": "bar", "encodings": {"category": "region", "value": "amount", "group": "tag"}, "metric": "sum"},
    }).json()
    assert len(payload["series"]) == 7
    assert payload["series"][-1] == "Other"
    assert len(payload["rows"][0]["values"]) == 7


def test_visualize_bar_layout_does_not_change_the_rows(client):
    node = upload(client, "layout.csv", b"region,quarter,amount\neast,q1,10\neast,q2,20\nwest,q1,30\n")
    base = f"/api/nodes/{node['id']}/datasets/{dataset(client, node, 'layout')['id']}"
    encodings = {"category": "region", "value": "amount", "group": "quarter"}
    grouped = client.post(base + "/visualize", json={"spec": {"chart": "bar", "encodings": encodings, "metric": "sum", "layout": "grouped"}}).json()
    stacked = client.post(base + "/visualize", json={"spec": {"chart": "bar", "encodings": encodings, "metric": "sum", "layout": "stacked100"}}).json()
    assert grouped["rows"] == stacked["rows"]
    assert grouped["series"] == stacked["series"]


def test_visualize_bar_without_group_keeps_the_old_shape(client):
    node = upload(client, "plain.csv", b"region,amount\neast,10\nwest,30\n")
    base = f"/api/nodes/{node['id']}/datasets/{dataset(client, node, 'plain')['id']}"
    payload = client.post(base + "/visualize", json={"spec": {"chart": "bar", "encodings": {"category": "region"}}}).json()
    assert payload["series"] is None
    assert all("values" not in row for row in payload["rows"])


def test_visualize_bar_rejects_an_unknown_group_column(client):
    node = upload(client, "badgroup.csv", b"region,amount\neast,10\n")
    base = f"/api/nodes/{node['id']}/datasets/{dataset(client, node, 'badgroup')['id']}"
    assert client.post(base + "/visualize", json={
        "spec": {"chart": "bar", "encodings": {"category": "region", "group": "missing"}},
    }).status_code == 422
```

- [ ] **Step 2: Run the tests and verify they fail**

```bash
uv run pytest tests/test_backend.py -k "bar_groups or folds_series or layout_does_not or keeps_the_old_shape or unknown_group" -v
```

Expected: FAIL — `series` missing from the response, unknown group accepted.

- [ ] **Step 3: Add the limits and spec fields**

In `backend/app.py`, beside the existing limits on lines 49–52:

```python
SERIES_LIMIT = 6
PIE_LIMIT = 8
LINE_POINT_LIMIT = 400
```

Extend the backend `ChartSpec`:

```python
class ChartSpec(BaseModel):
    model_config = ConfigDict(extra="forbid")
    chart: Literal["bar", "histogram", "box", "scatter", "line", "pie"]
    encodings: ChartEncodings = ChartEncodings()
    metric: Literal["count", "distinct", "min", "max", "sum", "avg", "median", "stddev"] | None = None
    density: bool = False
    layout: Literal["grouped", "stacked", "stacked100"] | None = None
    grain: Literal["hour", "day", "week", "month", "quarter", "year"] | None = None
```

- [ ] **Step 4: Implement the grouped branch**

In `visualize_response`, inside the `if spec.chart == "bar":` block, after the existing `rows` and `non_null` queries and before the `return`, insert:

```python
            group_name = spec.encodings.group
            series_labels: list[str] | None = None
            series_values: dict[Any, list[Any]] = {}
            if group_name:
                if group_name not in columns:
                    raise HTTPException(422, "Bar group column was not found")
                group_field = quote(group_name)
                labels = [label for label, _value, _n in rows]
                ranked = con.execute(f"""
                    SELECT {group_field} AS series, count(*) AS n
                    FROM {source} WHERE {field} IS NOT NULL AND {group_field} IS NOT NULL{extra_filter}
                    GROUP BY 1 ORDER BY n DESC, series
                    LIMIT {SERIES_LIMIT}
                """, values).fetchall()
                top = [row[0] for row in ranked]
                if labels and top:
                    label_marks = ", ".join("?" for _ in labels)
                    series_marks = ", ".join("?" for _ in top)
                    cells = con.execute(f"""
                        SELECT {field} AS label, {group_field} AS series, {value_sql} AS value
                        FROM {source}
                        WHERE {field} IN ({label_marks}) AND {group_field} IN ({series_marks}){extra_filter}
                        GROUP BY 1, 2
                    """, [*values, *labels, *top]).fetchall()
                    other = con.execute(f"""
                        SELECT {field} AS label, {value_sql} AS value
                        FROM {source}
                        WHERE {field} IN ({label_marks}) AND {group_field} IS NOT NULL
                          AND {group_field} NOT IN ({series_marks}){extra_filter}
                        GROUP BY 1
                    """, [*values, *labels, *top]).fetchall()
                    series_labels = [str(item) for item in top]
                    if other:
                        series_labels.append("Other")
                    grid = {(label, str(series)): value for label, series, value in cells}
                    other_by_label = {label: value for label, value in other}
                    for label in labels:
                        cells_for_label = [safe(grid.get((label, series), 0)) for series in series_labels[:len(top)]]
                        if other:
                            cells_for_label.append(safe(other_by_label.get(label, 0)))
                        series_values[label] = cells_for_label
```

Series are ranked by `count(*)` rather than by the metric: ranking series by an average would order them by magnitude rather than by weight, and put a one-row series ahead of a thousand-row one.

Change the return to carry the new fields:

```python
            return {
                "chart": "bar",
                "rows": [
                    {"label": safe(label), "value": safe(value), "n": safe(n)}
                    | ({"values": series_values[label]} if label in series_values else {})
                    for label, value, n in rows
                ],
                "series": series_labels,
                "other_count": safe(non_null - shown),
                "elapsed_ms": round((time.perf_counter() - started) * 1000, 3),
            }
```

- [ ] **Step 5: Add the frontend types**

In `frontend/src/lib/types.ts`:

```ts
export interface BarVisualizeRow {
  label: string | boolean | number;
  value: AggregateCount;
  n?: AggregateCount;
  values?: AggregateCount[];
}

export interface BarVisualizeResponse {
  chart: 'bar';
  rows: BarVisualizeRow[];
  series: string[] | null;
  other_count: AggregateCount;
  elapsed_ms: number;
}
```

- [ ] **Step 6: Run the tests and verify they pass**

```bash
uv run pytest tests/test_backend.py -k visualize -v
```

Expected: PASS, including every pre-existing visualize test.

- [ ] **Step 7: Commit**

```bash
git add backend/app.py tests/test_backend.py frontend/src/lib/types.ts
git commit -m "$(printf 'feat(charts): return aligned bar series for a group encoding\n\nCo-Authored-By: Claude Opus 5 <noreply@anthropic.com>')"
```

---

## Task 4: Bar series rendering, layout toggle, and the shared legend

**Objective:** Draw grouped, stacked, and 100% stacked bars, add the layout toggle, and build the legend every later task reuses.

**Files:**
- Create: `frontend/src/components/molecules/ChartLegend.svelte`
- Modify: `frontend/src/components/molecules/BarChart.svelte`
- Modify: `frontend/src/components/molecules/ChartView.svelte`
- Modify: `frontend/src/components/molecules/ChartOptionsPane.svelte`
- Modify: `frontend/src/components/organisms/VisualizeStage.svelte`, `frontend/src/components/organisms/DashboardStage.svelte`, `frontend/src/App.svelte`
- Modify: `frontend/src/lib/types.ts`, `frontend/src/lib/visualize.ts`
- Modify: `frontend/tests/visualize.test.js`

**Interfaces:**
- Consumes: `BarVisualizeResponse.series`/`values` (Task 3), `ChipToggleGroup` and the pane props (Task 2), `barLayouts` (Task 1).
- Produces:
  - `ChartLegend` props: `{ kind: 'series' | 'gradient' | 'size'; title: string; items?: { label: string; color: string }[]; from?: string; to?: string; min?: string; max?: string }`
  - `ChartMark` category gains `series?: string | boolean | number`
  - `App.svelte` exposes `selectVisualizeLayout(layout: BarLayout)`

- [ ] **Step 1: Write the failing test for the series mark**

Append to `frontend/tests/visualize.test.js`:

```js
test('clicking a series bar filters on both the category and the group', () => {
  const spec = { chart: 'bar', encodings: { category: 'region', value: 'price', group: 'flag' }, metric: 'sum' };
  assert.deepEqual(filtersFromMark(spec, { kind: 'category', value: 'east', series: 'true' }), [
    { column: 'region', operator: '=', value: 'east' },
    { column: 'flag', operator: '=', value: 'true', connector: 'and' }
  ]);
});

test('a category mark without a series filters on the category alone', () => {
  const spec = { chart: 'bar', encodings: { category: 'region', group: 'flag' } };
  assert.deepEqual(filtersFromMark(spec, { kind: 'category', value: 'east' }), [
    { column: 'region', operator: '=', value: 'east' }
  ]);
});
```

- [ ] **Step 2: Run the tests and verify they fail**

```bash
cd frontend && npm test
```

Expected: FAIL — the series condition is not emitted.

- [ ] **Step 3: Extend `ChartMark` and `filtersFromMark`**

In `frontend/src/lib/types.ts`:

```ts
export type ChartMark =
  | { kind: 'category'; value: string | boolean | number; series?: string | boolean | number }
  | { kind: 'bin'; lower: NumericValue; upper: NumericValue; last?: boolean }
  | { kind: 'region'; xMin: NumericValue; xMax: NumericValue; yMin: NumericValue; yMax: NumericValue };
```

In `frontend/src/lib/visualize.ts`, replace the `mark.kind === 'category'` branch of `filtersFromMark`:

```ts
  if (mark.kind === 'category') {
    const column = spec.encodings.category ?? spec.encodings.group;
    if (!column) return [];
    const filters: FilterCondition[] = [{ column, operator: '=', value: mark.value, ...and(0) }];
    if (mark.series !== undefined && spec.encodings.group && spec.encodings.group !== column) {
      filters.push({ column: spec.encodings.group, operator: '=', value: mark.series, ...and(1) });
    }
    return filters;
  }
```

- [ ] **Step 4: Run the tests and verify they pass**

```bash
cd frontend && npm test
```

Expected: PASS.

- [ ] **Step 5: Create `ChartLegend.svelte`**

```svelte
<script lang="ts">
  type Item = { label: string; color: string };
  type Props = {
    kind: 'series' | 'gradient' | 'size';
    title: string;
    items?: Item[];
    from?: string;
    to?: string;
    min?: string;
    max?: string;
  };
  let { kind, title, items = [], from = '', to = '', min = '', max = '' }: Props = $props();
</script>

<div class="legend" role="group" aria-label={title}>
  <span class="legend-title">{title}</span>
  {#if kind === 'series'}
    <div class="items">
      {#each items as item (item.label)}
        <span class="item"><span class="swatch" style:background={item.color}></span>{item.label}</span>
      {/each}
    </div>
  {:else if kind === 'gradient'}
    <div class="ramp">
      <span class="bound">{min}</span>
      <span class="bar" style:background={`linear-gradient(to right, ${from}, ${to})`}></span>
      <span class="bound">{max}</span>
    </div>
  {:else}
    <div class="sizes">
      <span class="bound">{min}</span>
      <span class="dot small"></span>
      <span class="dot large"></span>
      <span class="bound">{max}</span>
    </div>
  {/if}
</div>

<style>
  .legend { display: flex; align-items: center; flex-wrap: wrap; gap: 4px 10px; padding: 2px 8px; font-size: 10.5px; color: var(--muted); }
  .legend-title { font: 10px var(--font-mono); color: var(--faint); text-transform: uppercase; letter-spacing: .04em; }
  .items { display: flex; flex-wrap: wrap; gap: 4px 10px; }
  .item { display: inline-flex; align-items: center; gap: 5px; font-family: var(--font-mono); }
  .swatch { width: 9px; height: 9px; border-radius: 2px; }
  .ramp, .sizes { display: inline-flex; align-items: center; gap: 6px; }
  .bar { width: 96px; height: 9px; border-radius: 2px; }
  .bound { font: 10px var(--font-mono); color: var(--faint); }
  .dot { border-radius: 50%; background: var(--chart-mark-strong); }
  .dot.small { width: 5px; height: 5px; }
  .dot.large { width: 12px; height: 12px; }
</style>
```

- [ ] **Step 6: Render series in `BarChart.svelte`**

Add `series` and `layout` to `Props` and the destructuring:

```ts
    series?: string[] | null;
    layout?: BarLayout;
```

with `let { rows, xTitle, yTitle, count, compact, aggregated, series = null, layout = 'grouped', onSelect }: Props = $props();` and `onSelect: (label: string | boolean | number, series?: string | boolean | number) => void`.

Replace the `values`/`yScale` derivations so the domain accounts for stacking:

```ts
  let stacked = $derived(!!series && layout !== 'grouped');
  let normalized = $derived(!!series && layout === 'stacked100');
  let cells = $derived(rows.map((row) => (row.values ?? [Number(row.value)]).map(Number)));
  let rowTotals = $derived(cells.map((row) => row.reduce((sum, value) => sum + value, 0)));
  let values = $derived(
    normalized ? [0, 1]
      : stacked ? rowTotals
      : cells.flat()
  );
```

The existing `total` derivation reads the old flat `values`, which now means something different. Repoint it at the cells so the "% of this chart" hover line keeps its meaning:

```ts
  let total = $derived(cells.flat().reduce((sum, value) => sum + value, 0));
```

`yTicks` for the 100% case format as percentages; keep `formatTick` otherwise:

```ts
  let yTicks = $derived(yScale.ticks.map((value) => ({
    label: normalized ? `${Math.round(value * 100)}%` : formatTick(value),
    t: (value - yScale.min) / ySpan
  })));
```

Replace the `{#each rows ...}` body with a nested loop over the series. Grouped divides the band; stacked accumulates:

```svelte
      {#each rows as row, index (String(row.label))}
        {@const band = plot.width / rows.length}
        {@const cellValues = cells[index]}
        {@const total = normalized ? (rowTotals[index] || 1) : 1}
        {@const slots = series?.length ?? 1}
        {@const bandX = plot.x + index * band + band * 0.12}
        {@const bandWidth = Math.max(1, band * 0.76)}
        {#each cellValues as raw, seriesIndex}
          {@const value = raw / total}
          {@const below = stacked ? cellValues.slice(0, seriesIndex).reduce((sum, item) => sum + item / total, 0) : 0}
          {@const top = yAt(plot.y, plot.height, below + value)}
          {@const base = yAt(plot.y, plot.height, below)}
          {@const y = Math.min(top, base)}
          {@const barHeight = Math.max(stacked ? 0 : 2, Math.abs(base - top))}
          {@const x = stacked ? bandX : bandX + (seriesIndex * bandWidth) / slots}
          {@const w = stacked ? bandWidth : Math.max(1, bandWidth / slots)}
          {@const seriesLabel = series?.[seriesIndex]}
          {@const clickable = !series || seriesLabel !== 'Other'}
          <rect
            {x} {y} width={w} height={barHeight} rx="1"
            class="bar"
            class:flat={!clickable}
            style={`--chart-bar-fill:var(--chart-series-${((series ? seriesIndex : index) % 6) + 1}-fill)`}
            tabindex={clickable ? 0 : -1}
            role={clickable ? 'button' : 'presentation'}
            aria-label={`${row.label}${seriesLabel ? ` · ${seriesLabel}` : ''}: ${compact(raw)}. Filter to this value.`}
            onclick={() => { if (clickable) onSelect(row.label, seriesLabel); }}
            onkeydown={(event) => { if (clickable && (event.key === 'Enter' || event.key === ' ')) { event.preventDefault(); onSelect(row.label, seriesLabel); } }}
            onpointerenter={() => hover = describe(row, seriesIndex)}
            onfocus={() => hover = describe(row, seriesIndex)}
          />
        {/each}
      {/each}
```

Extend `describe` to name the series:

```ts
  function describe(row: BarVisualizeRow, seriesIndex: number): ChartHover {
    const label = series?.[seriesIndex];
    const raw = (row.values ?? [row.value])[seriesIndex];
    const title = label ? `${row.label} · ${label}` : String(row.label);
    if (aggregated || series) {
      return { title, lines: [`${yTitle}: ${compact(raw)}`], hint: label === 'Other' ? 'Folded from smaller series' : 'Click to filter this value' };
    }
    const share = total ? `${((Number(raw) * 100) / total).toFixed(1)}%` : '0%';
    return { title, lines: [`${count(raw)} rows`, `${share} of this chart`], hint: 'Click to filter this value' };
  }
```

Add to the style block, so the layout change tweens position rather than re-rendering, and so the `Other` bar reads as inert:

```css
  .bar.flat { cursor: default; }
  .bar.flat:hover { fill: var(--chart-bar-fill, var(--chart-mark)); }
  @media (prefers-reduced-motion: no-preference) {
    .bar { transition: x 220ms cubic-bezier(0.22, 1, 0.36, 1), y 220ms cubic-bezier(0.22, 1, 0.36, 1), width 220ms cubic-bezier(0.22, 1, 0.36, 1), height 220ms cubic-bezier(0.22, 1, 0.36, 1); }
  }
```

Keep the existing `grow` animation for the initial draw; the transition above handles grouped ↔ stacked.

- [ ] **Step 7: Show the legend and pass the layout through**

In `ChartView.svelte`, import `ChartLegend`, wrap the bar branch, and forward the new props:

```svelte
  {:else if data?.chart === 'bar'}
    <div class="stack">
      <BarChart rows={data.rows} series={data.series} layout={spec.layout ?? 'grouped'} {xTitle} {yTitle} {count} {compact} {aggregated}
        onSelect={(value, series) => onMark({ kind: 'category', value, series })} />
      {#if data.series}
        <ChartLegend kind="series" title={spec.encodings.group ?? 'Series'}
          items={data.series.map((label, index) => ({ label, color: `var(--chart-series-${(index % 6) + 1})` }))} />
      {/if}
    </div>
```

with `.stack { flex: 1; min-width: 0; min-height: 0; display: flex; flex-direction: column; }` added to its styles.

In `ChartOptionsPane.svelte`, add a layout block below the aggregate block, visible only when the spec carries a group encoding on a bar chart:

```svelte
      {#if spec?.chart === 'bar' && spec.encodings.group}
        <div class="pane-block">
          <Eyebrow>Layout</Eyebrow>
          <ChipToggleGroup label="Bar layout" options={barLayouts} selected={spec.layout ?? 'grouped'}
            onSelect={(value) => onSelectLayout(value as BarLayout)} />
        </div>
      {/if}
```

Add `barLayouts` to the pane's `import { groupColumns, visualizeMetrics } from '../../lib/visualize'` line and `BarLayout` to its `import type` line. Add `onSelectLayout: (layout: BarLayout) => void` to the pane's props, and thread it through `VisualizeStage.svelte` and `DashboardStage.svelte` the same way `onSelectMetric` already is — add it to each `Props` type, to each destructuring, and to each `<ChartOptionsPane ... />` call site. In `App.svelte`, add `let visualizeLayout = $state<BarLayout | null>(null);`, reset it in `resetVisualizeDraft`, set it in `selectVisualizeLayout`, and include `layout: visualizeLayout ?? undefined` in `visualizeSpec` when the chart is `bar` and the encodings have a group.

- [ ] **Step 8: Typecheck and verify in the browser**

```bash
cd frontend && npx svelte-check --threshold error
```

Then select a categorical column, a numeric column, and a second categorical column; set the second categorical to Group; confirm bars split into series with a legend, that the layout toggle appears, and that switching grouped → stacked → 100% tweens the bars rather than snapping.

- [ ] **Step 9: Commit**

```bash
git add frontend/src frontend/tests
git commit -m "$(printf 'feat(charts): draw grouped, stacked, and 100%% bar series\n\nCo-Authored-By: Claude Opus 5 <noreply@anthropic.com>')"
```

---

## Task 5: Scatter scale backend

**Objective:** Select the size and shape channels, and return stable scale metadata computed over the whole filtered relation.

**Files:**
- Modify: `backend/app.py:1574-1608` (scatter branch)
- Modify: `frontend/src/lib/types.ts`
- Test: `tests/test_backend.py`

**Interfaces:**
- Consumes: nothing from earlier tasks.
- Produces: `ScatterPoint.size`/`shape`, and `ScatterVisualizeResponse.color_kind`/`color_domain`/`color_labels`/`shape_labels`/`size_domain`.

- [ ] **Step 1: Write the failing tests**

```python
def test_visualize_scatter_returns_size_and_color_domains_over_the_whole_relation(client, monkeypatch):
    body = b"x,y,weight,region\n" + b"".join(f"{i},{i * 2},{i},r{i % 3}\n".encode() for i in range(1, 101))
    node = upload(client, "scales.csv", body)
    monkeypatch.setattr(backend_app, "SCATTER_LIMIT", 5)
    base = f"/api/nodes/{node['id']}/datasets/{dataset(client, node, 'scales')['id']}"
    payload = client.post(base + "/visualize", json={
        "spec": {"chart": "scatter", "encodings": {"x": "x", "y": "y", "size": "weight", "color": "weight"}},
    }).json()
    assert len(payload["points"]) == 5
    assert payload["size_domain"] == [1, 100]
    assert payload["color_kind"] == "numeric"
    assert payload["color_domain"] == [1, 100]


def test_visualize_scatter_labels_categorical_color_and_shape(client):
    node = upload(client, "shapes.csv", b"x,y,region,tier\n1,2,east,gold\n3,4,west,silver\n5,6,east,gold\n")
    base = f"/api/nodes/{node['id']}/datasets/{dataset(client, node, 'shapes')['id']}"
    payload = client.post(base + "/visualize", json={
        "spec": {"chart": "scatter", "encodings": {"x": "x", "y": "y", "color": "region", "pattern": "tier"}},
    }).json()
    assert payload["color_kind"] == "categorical"
    assert sorted(payload["color_labels"]) == ["east", "west"]
    assert sorted(payload["shape_labels"]) == ["gold", "silver"]
    assert all("shape" in point for point in payload["points"])


def test_visualize_scatter_rejects_a_non_numeric_size(client):
    node = upload(client, "badsize.csv", b"x,y,region\n1,2,east\n")
    base = f"/api/nodes/{node['id']}/datasets/{dataset(client, node, 'badsize')['id']}"
    assert client.post(base + "/visualize", json={
        "spec": {"chart": "scatter", "encodings": {"x": "x", "y": "y", "size": "region"}},
    }).status_code == 422
```

- [ ] **Step 2: Run the tests and verify they fail**

```bash
uv run pytest tests/test_backend.py -k scatter -v
```

Expected: FAIL — no `size_domain`, `size` encoding silently ignored.

- [ ] **Step 3: Implement the channels**

Replace the body of the `if spec.chart == "scatter":` block from `color_name = spec.encodings.color` down to its `return`:

```python
            color_name = spec.encodings.color
            size_name = spec.encodings.size
            shape_name = spec.encodings.pattern
            for role, name in (("color", color_name), ("size", size_name), ("shape", shape_name)):
                if name and name not in columns:
                    raise HTTPException(422, f"Scatter {role} column was not found")
            if size_name and profile_kind(columns[size_name]) != "numeric":
                raise HTTPException(422, "Scatter size requires a numeric column")
            x_field, y_field = quote(x_name), quote(y_name)
            x_ok = f"{x_field} IS NOT NULL"
            y_ok = f"{y_field} IS NOT NULL"
            if profile_kind(columns[x_name]) == "numeric":
                x_ok += f" AND isfinite({x_field}::DOUBLE)"
            if profile_kind(columns[y_name]) == "numeric":
                y_ok += f" AND isfinite({y_field}::DOUBLE)"
            where = f"WHERE {x_ok} AND {y_ok}"
            total_points = con.execute(f"SELECT count(*) FROM {source} {where}", values).fetchone()[0]

            def domain(name: str) -> list[Any]:
                field_sql = quote(name)
                low, high = con.execute(
                    f"SELECT min({field_sql}), max({field_sql}) FROM {source} {where}"
                    f" AND {field_sql} IS NOT NULL AND isfinite({field_sql}::DOUBLE)",
                    values,
                ).fetchone()
                return [safe(low), safe(high)]

            def top_labels(name: str) -> list[str]:
                field_sql = quote(name)
                ranked = con.execute(f"""
                    SELECT {field_sql} AS label, count(*) AS n
                    FROM {source} {where} AND {field_sql} IS NOT NULL
                    GROUP BY 1 ORDER BY n DESC, label
                    LIMIT {SERIES_LIMIT}
                """, values).fetchall()
                return [str(row[0]) for row in ranked]

            color_kind = profile_kind(columns[color_name]) if color_name else None
            color_numeric = color_kind == "numeric"
            extras = [(name, alias) for name, alias in (
                (color_name, "color"), (size_name, "size"), (shape_name, "shape")
            ) if name]
            select_extra = "".join(f", {quote(name)} AS {alias}" for name, alias in extras)
            points = con.execute(f"""
                SELECT {x_field} AS x, {y_field} AS y{select_extra}
                FROM {source} {where}
                ORDER BY random()
                LIMIT {SCATTER_LIMIT}
            """, values).fetchall()
            color_labels = None if not color_name or color_numeric else top_labels(color_name)
            shape_labels = top_labels(shape_name) if shape_name else None

            def fold(value: Any, labels: list[str] | None) -> Any:
                if labels is None or value is None:
                    return safe(value)
                return str(value) if str(value) in labels else "Other"

            point_rows = []
            for row in points:
                record: dict[str, Any] = {"x": safe(row[0]), "y": safe(row[1])}
                for offset, (_name, alias) in enumerate(extras, start=2):
                    raw = row[offset]
                    if alias == "color":
                        record["color"] = safe(raw) if color_numeric else fold(raw, color_labels)
                    elif alias == "shape":
                        record["shape"] = fold(raw, shape_labels)
                    else:
                        record["size"] = safe(raw)
                point_rows.append(record)
            for labels in (color_labels, shape_labels):
                if labels is not None and len(labels) == SERIES_LIMIT:
                    labels.append("Other")
            response: dict[str, Any] = {
                "chart": "scatter",
                "points": point_rows,
                "total_points": safe(total_points),
                "elapsed_ms": round((time.perf_counter() - started) * 1000, 3),
            }
            if color_name:
                response["color_kind"] = "numeric" if color_numeric else "categorical"
                if color_numeric:
                    response["color_domain"] = domain(color_name)
                else:
                    response["color_labels"] = color_labels
            if size_name:
                response["size_domain"] = domain(size_name)
            if shape_name:
                response["shape_labels"] = shape_labels
            return response
```

The domains and label sets come from queries over `{source} {where}`, not from `points`, so they do not move when the random sample changes.

- [ ] **Step 4: Add the frontend types**

```ts
export interface ScatterPoint {
  x: NumericValue;
  y: NumericValue;
  color?: string | number | boolean | null;
  size?: NumericValue;
  shape?: string | number | boolean | null;
}

export interface ScatterVisualizeResponse {
  chart: 'scatter';
  points: ScatterPoint[];
  total_points: AggregateCount;
  color_kind?: 'categorical' | 'numeric';
  color_domain?: [NumericValue, NumericValue];
  color_labels?: string[];
  shape_labels?: string[];
  size_domain?: [NumericValue, NumericValue];
  elapsed_ms: number;
}
```

- [ ] **Step 5: Run the tests and verify they pass**

```bash
uv run pytest tests/test_backend.py -k scatter -v
```

Expected: PASS, including `test_visualize_scatter_carries_color_categories_and_rejects_unknown_color`.

- [ ] **Step 6: Commit**

```bash
git add backend/app.py tests/test_backend.py frontend/src/lib/types.ts
git commit -m "$(printf 'feat(charts): add scatter size and shape channels with stable scales\n\nCo-Authored-By: Claude Opus 5 <noreply@anthropic.com>')"
```

---

## Task 6: Scatter scale rendering and legends

**Objective:** Draw size, shape, and continuous colour on the scatter canvas, with a legend per active channel.

**Files:**
- Modify: `frontend/src/components/molecules/ScatterPlot.svelte`
- Modify: `frontend/src/components/molecules/ChartView.svelte`

**Interfaces:**
- Consumes: the scatter response fields (Task 5), `ChartLegend` (Task 4).
- Produces: `ScatterPlot` props gain `colorKind`, `colorDomain`, `colorLabels`, `shapeLabels`, `sizeDomain`.

- [ ] **Step 1: Add the props**

```ts
  type Props = {
    points: ScatterPoint[];
    xTitle: string;
    yTitle: string;
    compact: (value: number | string | null | undefined) => string;
    theme?: string;
    colorKind?: 'categorical' | 'numeric';
    colorDomain?: [NumericValue, NumericValue];
    colorLabels?: string[];
    shapeLabels?: string[];
    sizeDomain?: [NumericValue, NumericValue];
    onSelectRegion: (region: { xMin: NumericValue; xMax: NumericValue; yMin: NumericValue; yMax: NumericValue }) => void;
  };
```

Replace the local `colorLabels` derivation with one that prefers the server's list and falls back to the old behaviour:

```ts
  let categoryIndex = $derived.by(() => {
    const labels = colorLabels ?? [...new Set(points.filter((point) => point.color != null).map((point) => String(point.color)))].sort();
    return new Map(labels.map((label, index) => [label, index]));
  });
  let shapeIndex = $derived(new Map((shapeLabels ?? []).map((label, index) => [label, index])));
```

- [ ] **Step 2: Add the colour and size helpers**

Above the `$effect`, add:

```ts
  function hexToRgb(hex: string): [number, number, number] {
    const value = hex.trim().replace('#', '');
    const full = value.length === 3 ? value.split('').map((char) => char + char).join('') : value;
    const number = Number.parseInt(full, 16);
    return Number.isFinite(number) ? [(number >> 16) & 255, (number >> 8) & 255, number & 255] : [0, 0, 0];
  }

  function mix(from: string, to: string, t: number): string {
    const a = hexToRgb(from);
    const b = hexToRgb(to);
    const clamped = Math.max(0, Math.min(1, t));
    const channel = (index: number) => Math.round(a[index] + (b[index] - a[index]) * clamped);
    return `rgb(${channel(0)}, ${channel(1)}, ${channel(2)})`;
  }

  const SHAPES = ['circle', 'square', 'triangle', 'diamond', 'cross', 'triangle-down'] as const;

  function drawShape(ctx: CanvasRenderingContext2D, shape: string, x: number, y: number, r: number) {
    ctx.beginPath();
    if (shape === 'square') ctx.rect(x - r, y - r, r * 2, r * 2);
    else if (shape === 'triangle') { ctx.moveTo(x, y - r); ctx.lineTo(x + r, y + r); ctx.lineTo(x - r, y + r); ctx.closePath(); }
    else if (shape === 'triangle-down') { ctx.moveTo(x, y + r); ctx.lineTo(x + r, y - r); ctx.lineTo(x - r, y - r); ctx.closePath(); }
    else if (shape === 'diamond') { ctx.moveTo(x, y - r); ctx.lineTo(x + r, y); ctx.lineTo(x, y + r); ctx.lineTo(x - r, y); ctx.closePath(); }
    else if (shape === 'cross') { ctx.moveTo(x - r, y - r); ctx.lineTo(x + r, y + r); ctx.moveTo(x + r, y - r); ctx.lineTo(x - r, y + r); }
    else ctx.arc(x, y, r, 0, Math.PI * 2);
  }
```

- [ ] **Step 3: Use them in the draw loop**

Inside the `$effect`, after `strong` is read, add the size and ramp setup:

```ts
    const sizeLow = sizeDomain ? Number(sizeDomain[0]) : 0;
    const sizeHigh = sizeDomain ? Number(sizeDomain[1]) : 1;
    const sizeSpan = sizeHigh - sizeLow || 1;
    const colorLow = colorDomain ? Number(colorDomain[0]) : 0;
    const colorHigh = colorDomain ? Number(colorDomain[1]) : 1;
    const colorSpan = colorHigh - colorLow || 1;
    const R_MIN = 1.6;
    const R_MAX = 7;
```

and replace the per-point fill and draw so radius encodes area and shape/colour follow their channels:

```ts
      const radius = sizeDomain && point.size != null
        ? R_MIN + (R_MAX - R_MIN) * Math.sqrt(Math.max(0, Math.min(1, (Number(point.size) - sizeLow) / sizeSpan)))
        : 2.2;
      const shapeKey = point.shape == null ? 'circle' : SHAPES[shapeIndex.get(String(point.shape)) ?? 0];
      if (colorKind === 'numeric' && point.color != null) {
        ctx.fillStyle = mix(mark, strong, (Number(point.color) - colorLow) / colorSpan);
      } else {
        const colorIndex = point.color == null ? -1 : categoryIndex.get(String(point.color)) ?? -1;
        ctx.fillStyle = colorIndex >= 0 && pointFills.length ? pointFills[colorIndex % pointFills.length] : mark;
      }
      ctx.strokeStyle = strong;
      drawShape(ctx, shapeKey, x, y, radius);
      if (shapeKey === 'cross') ctx.stroke();
      else { ctx.fill(); ctx.stroke(); }
```

Radius uses `Math.sqrt` so the drawn **area** is proportional to the value. Scaling the radius directly would exaggerate large values by the square.

Add `colorKind`, `colorDomain`, `sizeDomain`, `shapeLabels`, and `colorLabels` to the `$effect`'s read set by referencing them in the body, which the code above already does.

- [ ] **Step 4: Render the legends**

In `ChartView.svelte`, replace the scatter branch:

```svelte
  {:else if data?.chart === 'scatter'}
    <div class="stack">
      <ScatterPlot points={data.points} {xTitle} {yTitle} {compact} theme={chartTheme}
        colorKind={data.color_kind} colorDomain={data.color_domain} colorLabels={data.color_labels}
        shapeLabels={data.shape_labels} sizeDomain={data.size_domain}
        onSelectRegion={(region) => onMark({ kind: 'region', ...region })} />
      {#if data.color_kind === 'numeric' && data.color_domain}
        <ChartLegend kind="gradient" title={spec.encodings.color ?? 'Color'}
          from="var(--chart-mark)" to="var(--chart-mark-strong)"
          min={compact(data.color_domain[0])} max={compact(data.color_domain[1])} />
      {:else if data.color_labels}
        <ChartLegend kind="series" title={spec.encodings.color ?? 'Color'}
          items={data.color_labels.map((label, index) => ({ label, color: `var(--chart-series-${(index % 6) + 1})` }))} />
      {/if}
      {#if data.shape_labels}
        <ChartLegend kind="series" title={spec.encodings.pattern ?? 'Shape'}
          items={data.shape_labels.map((label) => ({ label, color: 'var(--chart-mark-strong)' }))} />
      {/if}
      {#if data.size_domain}
        <ChartLegend kind="size" title={spec.encodings.size ?? 'Size'}
          min={compact(data.size_domain[0])} max={compact(data.size_domain[1])} />
      {/if}
    </div>
```

- [ ] **Step 5: Name the new channels in the dashboard chart title**

`chartTitle` in `frontend/src/lib/dashboard.ts` lists only `category`, `x`, `value`, `y`, and `group`, so two scatters differing only by their size or colour column get identical tile titles. Add `chartTitle` to that file's `import { ... } from '../src/lib/dashboard.ts'` line — it is not imported today — then add a failing test to `frontend/tests/dashboard.test.js`:

```js
test('a chart title names the group, size, and color columns', () => {
  const spec = { chart: 'scatter', encodings: { x: 'price', y: 'amount', size: 'weight', color: 'region' } };
  assert.equal(chartTitle(spec), 'Scatter · price × amount × weight × region');
});
```

Run `cd frontend && npm test` and verify it fails, then widen the field list:

```ts
export function chartTitle(spec: ChartSpec): string {
  const fields = [
    spec.encodings.category, spec.encodings.x, spec.encodings.value, spec.encodings.y,
    spec.encodings.group, spec.encodings.size, spec.encodings.color, spec.encodings.pattern
  ].filter(Boolean);
  return `${spec.chart[0].toUpperCase()}${spec.chart.slice(1)} · ${fields.join(' × ') || 'Chart'}`;
}
```

Run the suite again and verify it passes. `dashboard.test.js` has no chart-title assertions today, so nothing else in it can regress; a spec with no group, size, colour, or shape still produces the same string as before.

- [ ] **Step 6: Typecheck and verify in the browser**

```bash
cd frontend && npx svelte-check --threshold error
```

Select two numeric columns plus a third numeric and a categorical; set the third numeric to Size and the categorical to Shape; confirm point radii vary, shapes differ per category, and a legend appears per channel. Then set a numeric column to Color and confirm the gradient legend reports the column's min and max.

The scatter plot draws to a canvas, so a channel change repaints rather than transitioning — the spec's "cross-fade colour, tween radius" is not reachable without keeping the previous frame and interpolating by hand each animation tick. Skip it; the repaint is instantaneous and already matches the reduced-motion end state. Mark the decision above the `$effect`:

```ts
  // ponytail: canvas repaints channel changes instantly; add a tweened repaint loop only if the jump reads as jarring
```

- [ ] **Step 7: Commit**

```bash
git add frontend/src frontend/tests
git commit -m "$(printf 'feat(charts): render scatter size, shape, and continuous color scales\n\nCo-Authored-By: Claude Opus 5 <noreply@anthropic.com>')"
```

---

## Task 7: Line chart backend

**Objective:** Implement the line branch with automatic time-bucketing, an explicit grain override, and optional series.

**Files:**
- Modify: `backend/app.py` (new branch before the final `raise`)
- Modify: `frontend/src/lib/types.ts`, `frontend/src/lib/visualize.ts`
- Modify: `frontend/tests/visualize.test.js`
- Test: `tests/test_backend.py`

**Interfaces:**
- Consumes: `TimeGrain` (Task 1), `SERIES_LIMIT`/`LINE_POINT_LIMIT` (Task 3).
- Produces: `LinePoint`, `LineSeries`, `LineVisualizeResponse`, and `autoGrain(spanSeconds: number): TimeGrain` in `visualize.ts`.

- [ ] **Step 1: Write the failing tests**

```python
def test_visualize_line_buckets_by_an_automatic_grain(client):
    body = b"day,amount\n" + b"".join(f"2026-01-{i:02d},{i}\n".encode() for i in range(1, 29))
    node = upload(client, "daily.csv", body)
    base = f"/api/nodes/{node['id']}/datasets/{dataset(client, node, 'daily')['id']}"
    payload = client.post(base + "/visualize", json={
        "spec": {"chart": "line", "encodings": {"x": "day", "y": "amount"}, "metric": "sum"},
    }).json()
    assert payload["chart"] == "line"
    assert payload["grain"] == "day"
    assert len(payload["series"]) == 1
    assert len(payload["series"][0]["points"]) == 28


def test_visualize_line_honours_an_explicit_grain(client):
    body = b"day,amount\n" + b"".join(f"2026-01-{i:02d},{i}\n".encode() for i in range(1, 29))
    node = upload(client, "explicit.csv", body)
    base = f"/api/nodes/{node['id']}/datasets/{dataset(client, node, 'explicit')['id']}"
    payload = client.post(base + "/visualize", json={
        "spec": {"chart": "line", "encodings": {"x": "day", "y": "amount"}, "metric": "sum", "grain": "month"},
    }).json()
    assert payload["grain"] == "month"
    assert len(payload["series"][0]["points"]) == 1
    assert payload["series"][0]["points"][0]["y"] == sum(range(1, 29))


def test_visualize_line_splits_into_series_by_group(client):
    body = b"day,region,amount\n" + b"".join(
        f"2026-01-{i:02d},{'east' if i % 2 else 'west'},{i}\n".encode() for i in range(1, 11)
    )
    node = upload(client, "lineseries.csv", body)
    base = f"/api/nodes/{node['id']}/datasets/{dataset(client, node, 'lineseries')['id']}"
    payload = client.post(base + "/visualize", json={
        "spec": {"chart": "line", "encodings": {"x": "day", "y": "amount", "group": "region"}, "metric": "sum"},
    }).json()
    assert sorted(series["label"] for series in payload["series"]) == ["east", "west"]


def test_visualize_line_counts_rows_without_a_y_encoding(client):
    node = upload(client, "counted.csv", b"day\n2026-01-01\n2026-01-01\n2026-01-02\n")
    base = f"/api/nodes/{node['id']}/datasets/{dataset(client, node, 'counted')['id']}"
    payload = client.post(base + "/visualize", json={"spec": {"chart": "line", "encodings": {"x": "day"}}}).json()
    assert [point["y"] for point in payload["series"][0]["points"]] == [2, 1]


def test_visualize_line_rejects_a_non_date_x(client):
    node = upload(client, "notdate.csv", b"x,y\n1,2\n")
    base = f"/api/nodes/{node['id']}/datasets/{dataset(client, node, 'notdate')['id']}"
    assert client.post(base + "/visualize", json={
        "spec": {"chart": "line", "encodings": {"x": "x", "y": "y"}},
    }).status_code == 422
```

- [ ] **Step 2: Run the tests and verify they fail**

```bash
uv run pytest tests/test_backend.py -k line -v
```

Expected: FAIL with 422 `Chart type line is not available yet`.

- [ ] **Step 3: Implement the branch**

Add the grain table beside the limits in `backend/app.py`:

```python
GRAIN_SECONDS = {
    "hour": 3600, "day": 86400, "week": 604800,
    "month": 2629746, "quarter": 7889238, "year": 31556952,
}
```

Then insert this branch immediately before the final `raise HTTPException(422, f"Chart type {spec.chart} is not available yet")`:

```python
        if spec.chart == "line":
            x_name = spec.encodings.x
            if not x_name or x_name not in columns:
                raise HTTPException(422, "Line requires an x encoding")
            if profile_kind(columns[x_name]) != "date":
                raise HTTPException(422, "Line requires a date or timestamp x column")
            x_field = quote(x_name)
            measure = spec.encodings.y
            metric = spec.metric or ("avg" if measure else "count")
            extra_filter = ""
            if measure:
                if measure not in columns:
                    raise HTTPException(422, "Line measure column was not found")
                if metric not in {"count", "distinct"} and profile_kind(columns[measure]) != "numeric":
                    raise HTTPException(422, "Line measure requires a numeric column")
                value_sql = METRIC_SQL.get(metric, METRIC_SQL["avg"]).format(column=quote(measure))
                extra_filter = f" AND {quote(measure)} IS NOT NULL"
                if metric not in {"count", "distinct"}:
                    extra_filter += f" AND isfinite({quote(measure)}::DOUBLE)"
            else:
                value_sql = "count(*)"
            where = f"WHERE {x_field} IS NOT NULL{extra_filter}"
            grain = spec.grain
            if grain is None:
                low, high = con.execute(f"SELECT min({x_field}), max({x_field}) FROM {source} {where}", values).fetchone()
                span = (high - low).total_seconds() if low is not None and high is not None else 0
                grain = "year"
                for candidate in ("hour", "day", "week", "month", "quarter", "year"):
                    if span / GRAIN_SECONDS[candidate] <= LINE_POINT_LIMIT:
                        grain = candidate
                        break
            bucket = f"date_trunc('{grain}', {x_field})"

            def series_points(clause: str, bound: list[Any]) -> list[dict[str, Any]]:
                rows = con.execute(f"""
                    SELECT bucket, value FROM (
                        SELECT {bucket} AS bucket, {value_sql} AS value
                        FROM {source} {where}{clause}
                        GROUP BY 1 ORDER BY 1 DESC
                        LIMIT {LINE_POINT_LIMIT}
                    ) ORDER BY bucket
                """, bound).fetchall()
                return [{"x": safe(bucket_value), "y": safe(value)} for bucket_value, value in rows]

            group_name = spec.encodings.group
            if group_name and group_name not in columns:
                raise HTTPException(422, "Line group column was not found")
            if group_name:
                group_field = quote(group_name)
                ranked = con.execute(f"""
                    SELECT {group_field} AS series, count(*) AS n
                    FROM {source} {where} AND {group_field} IS NOT NULL
                    GROUP BY 1 ORDER BY n DESC, series
                    LIMIT {SERIES_LIMIT}
                """, values).fetchall()
                top = [row[0] for row in ranked]
                series = []
                for label in top:
                    points = series_points(f" AND {group_field} = ?", [*values, label])
                    if points:
                        series.append({"label": str(label), "points": points})
                marks = ", ".join("?" for _ in top)
                if top:
                    other = series_points(f" AND {group_field} IS NOT NULL AND {group_field} NOT IN ({marks})", [*values, *top])
                    if other:
                        series.append({"label": "Other", "points": other})
            else:
                series = [{"label": "", "points": series_points("", values)}]
            return {
                "chart": "line",
                "grain": grain,
                "series": series,
                "elapsed_ms": round((time.perf_counter() - started) * 1000, 3),
            }
```

`grain` is interpolated into the SQL rather than bound, which is safe because Pydantic's `Literal` has already restricted it to the six known values.

- [ ] **Step 4: Add the frontend types and the shared grain helper**

In `frontend/src/lib/types.ts`:

```ts
export interface LinePoint { x: string; y: NumericValue }
export interface LineSeries { label: string; points: LinePoint[] }

export interface LineVisualizeResponse {
  chart: 'line';
  grain: TimeGrain;
  series: LineSeries[];
  elapsed_ms: number;
}

export type VisualizeResponse =
  | BarVisualizeResponse
  | HistogramVisualizeResponse
  | BoxVisualizeResponse
  | ScatterVisualizeResponse
  | LineVisualizeResponse;
```

In `frontend/src/lib/visualize.ts`, add the matching helper and drop the numeric-x line suggestion by changing the two-numeric branch of `suggestCharts` from `return [{ chart: 'scatter', encodings }, { chart: 'line', encodings }];` to `return [{ chart: 'scatter', encodings }];`:

```ts
const GRAIN_SECONDS: Record<TimeGrain, number> = {
  hour: 3600, day: 86400, week: 604800,
  month: 2629746, quarter: 7889238, year: 31556952
};

export const LINE_POINT_LIMIT = 400;

export function autoGrain(spanSeconds: number): TimeGrain {
  for (const grain of ['hour', 'day', 'week', 'month', 'quarter', 'year'] as TimeGrain[]) {
    if (spanSeconds / GRAIN_SECONDS[grain] <= LINE_POINT_LIMIT) return grain;
  }
  return 'year';
}
```

Add `pie` and keep `line` in `chartMeta` — `pie` lands in Task 9; for now add only what `line` needs, which is already present.

- [ ] **Step 5: Add the frontend test**

```js
test('autoGrain picks the finest grain that fits the point budget', () => {
  assert.equal(autoGrain(60 * 60 * 24), 'hour');
  assert.equal(autoGrain(60 * 60 * 24 * 60), 'day');
  assert.equal(autoGrain(60 * 60 * 24 * 365 * 5), 'day');
  assert.equal(autoGrain(60 * 60 * 24 * 365 * 500), 'month');
  assert.equal(autoGrain(60 * 60 * 24 * 365 * 100000), 'year');
});

test('two numeric columns no longer suggest a line chart', () => {
  assert.deepEqual(charts([price, amount]), ['scatter']);
});
```

Add `autoGrain` to the import list at the top of the test file.

- [ ] **Step 6: Run both suites and verify they pass**

```bash
uv run pytest tests/test_backend.py -k line -v && cd frontend && npm test
```

Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add backend/app.py tests/test_backend.py frontend/src/lib frontend/tests
git commit -m "$(printf 'feat(charts): add a time-bucketed line chart endpoint\n\nCo-Authored-By: Claude Opus 5 <noreply@anthropic.com>')"
```

---

## Task 8: Line chart component and grain controls

**Objective:** Draw the line chart and give the pane its time-grain buttons.

**Files:**
- Create: `frontend/src/components/molecules/LineChart.svelte`
- Modify: `frontend/src/components/molecules/ChartView.svelte`, `frontend/src/components/molecules/ChartOptionsPane.svelte`
- Modify: `frontend/src/components/organisms/VisualizeStage.svelte`, `frontend/src/components/organisms/DashboardStage.svelte`, `frontend/src/App.svelte`

**Interfaces:**
- Consumes: `LineVisualizeResponse` (Task 7), `ChartLegend` (Task 4), `ChipToggleGroup` and `timeGrains` (Tasks 1–2).
- Produces: `App.svelte` exposes `selectVisualizeGrain(grain: TimeGrain)`; `ChartOptionsPane` gains `onSelectGrain`.

- [ ] **Step 1: Create `LineChart.svelte`**

```svelte
<script lang="ts">
  import ChartFrame from './ChartFrame.svelte';
  import { formatPlotTick, formatTick, niceTicks, toPlotNumber } from '../../lib/visualize';
  import type { ChartHover, LineSeries } from '../../lib/types';

  type Props = {
    series: LineSeries[];
    xTitle: string;
    yTitle: string;
    compact: (value: number | string | null | undefined) => string;
    onSelect: (x: string, label: string) => void;
  };
  let { series, xTitle, yTitle, compact, onSelect }: Props = $props();

  let hover = $state<ChartHover | null>(null);
  let sample = $derived(series[0]?.points[0]?.x);
  let xs = $derived(series.flatMap((line) => line.points.map((point) => toPlotNumber(point.x))).filter(Number.isFinite));
  let ys = $derived(series.flatMap((line) => line.points.map((point) => Number(point.y))).filter(Number.isFinite));
  let domain = $derived.by(() => {
    const xTicksRaw = niceTicks(Math.min(...xs, 0), Math.max(...xs, 1), 6);
    const yTicksRaw = niceTicks(Math.min(0, ...ys), Math.max(1, ...ys), 5);
    return {
      xMin: xTicksRaw[0] ?? 0,
      xMax: xTicksRaw[xTicksRaw.length - 1] ?? 1,
      yMin: yTicksRaw[0] ?? 0,
      yMax: yTicksRaw[yTicksRaw.length - 1] ?? 1,
      xTicksRaw,
      yTicksRaw
    };
  });
  let xSpan = $derived(domain.xMax - domain.xMin || 1);
  let ySpan = $derived(domain.yMax - domain.yMin || 1);
  let xTicks = $derived(domain.xTicksRaw.map((value) => ({ label: formatPlotTick(value, sample), t: (value - domain.xMin) / xSpan })));
  let yTicks = $derived(domain.yTicksRaw.map((value) => ({ label: formatTick(value), t: (value - domain.yMin) / ySpan })));

  function path(line: LineSeries, plot: { x: number; y: number; width: number; height: number }): string {
    return line.points
      .map((point, index) => {
        const px = plot.x + ((toPlotNumber(point.x) - domain.xMin) / xSpan) * plot.width;
        const py = plot.y + (1 - (Number(point.y) - domain.yMin) / ySpan) * plot.height;
        return `${index ? 'L' : 'M'}${px.toFixed(2)} ${py.toFixed(2)}`;
      })
      .join(' ');
  }

  function describe(line: LineSeries, index: number): ChartHover {
    const point = line.points[index];
    return {
      title: formatPlotTick(toPlotNumber(point.x), sample),
      lines: [line.label ? `${line.label} · ${yTitle}: ${compact(point.y)}` : `${yTitle}: ${compact(point.y)}`],
      hint: line.label === 'Other' ? 'Folded from smaller series' : 'Click to filter this point'
    };
  }
</script>

{#if ys.length}
  <ChartFrame {xTicks} {yTicks} {xTitle} {yTitle} {hover} label="Line chart" onDismiss={() => hover = null}>
    {#snippet children(plot)}
      {#each series as line, lineIndex (line.label)}
        <path class="line" d={path(line, plot)} style={`--chart-line-stroke:var(--chart-series-${(lineIndex % 6) + 1})`} />
        {#each line.points as point, index (String(point.x))}
          {@const px = plot.x + ((toPlotNumber(point.x) - domain.xMin) / xSpan) * plot.width}
          {@const py = plot.y + (1 - (Number(point.y) - domain.yMin) / ySpan) * plot.height}
          <circle
            cx={px} cy={py} r="6"
            class="hit"
            tabindex={line.label === 'Other' ? -1 : 0}
            role={line.label === 'Other' ? 'presentation' : 'button'}
            aria-label={`${formatPlotTick(toPlotNumber(point.x), sample)}${line.label ? ` · ${line.label}` : ''}: ${compact(point.y)}. Filter to this point.`}
            onclick={() => { if (line.label !== 'Other') onSelect(String(point.x), line.label); }}
            onkeydown={(event) => { if (line.label !== 'Other' && (event.key === 'Enter' || event.key === ' ')) { event.preventDefault(); onSelect(String(point.x), line.label); } }}
            onpointerenter={() => hover = describe(line, index)}
            onfocus={() => hover = describe(line, index)}
          />
        {/each}
      {/each}
    {/snippet}
  </ChartFrame>
{:else}
  <p class="empty">No points to chart.</p>
{/if}

<style>
  .line { fill: none; stroke: var(--chart-line-stroke, var(--chart-mark-strong)); stroke-width: 1.75px; stroke-linejoin: round; stroke-linecap: round; }
  .hit { fill: transparent; cursor: pointer; }
  .hit:hover, .hit:focus-visible { fill: color-mix(in srgb, var(--chart-mark-strong) 22%, transparent); outline: none; }
  .empty { margin: auto; font-size: 12.5px; color: var(--muted); }
  @media (prefers-reduced-motion: no-preference) {
    .line { transition: d 260ms cubic-bezier(0.22, 1, 0.36, 1); }
  }
</style>
```

The `transition: d` morphs the path when the grain changes, in browsers that support interpolating path data; elsewhere it simply snaps, which is the reduced-motion end state anyway.

- [ ] **Step 2: Render it in `ChartView.svelte`**

Import `LineChart`, and add the branch after the scatter one:

```svelte
  {:else if data?.chart === 'line'}
    <div class="stack">
      <LineChart series={data.series} {xTitle} {yTitle} {compact}
        onSelect={(x, label) => onMark({ kind: 'category', value: x, series: label || undefined })} />
      {#if data.series.length > 1}
        <ChartLegend kind="series" title={spec.encodings.group ?? 'Series'}
          items={data.series.map((line, index) => ({ label: line.label, color: `var(--chart-series-${(index % 6) + 1})` }))} />
      {/if}
    </div>
```

Update the `xTitle`/`yTitle` derivations so line reads its own encodings:

```ts
  let xTitle = $derived(
    spec.chart === 'box' ? (spec.encodings.group ?? '')
      : spec.chart === 'scatter' || spec.chart === 'line' ? (spec.encodings.x ?? '')
      : spec.encodings.category ?? spec.encodings.value ?? spec.encodings.x ?? ''
  );
  let yTitle = $derived(
    spec.chart === 'line' ? (spec.encodings.y ? metricTitle(spec.metric, spec.encodings.y) : 'Rows')
      : spec.chart === 'box' || spec.chart === 'scatter' ? (spec.encodings.y ?? spec.encodings.value ?? '')
      : spec.chart === 'bar' && spec.encodings.value ? metricTitle(spec.metric, spec.encodings.value)
      : 'Rows'
  );
```

A line point's mark filters on the bucket start, not the whole bucket. This is a known ceiling: clicking a month point filters `x = '2026-01-01'` rather than the month's range. Mark it in `ChartView.svelte`:

```svelte
<!-- ponytail: a line mark filters the bucket start; widen to a bucket range filter if users ask for it -->
```

- [ ] **Step 3: Add the grain buttons**

In `ChartOptionsPane.svelte`, below the layout block:

```svelte
      {#if spec?.chart === 'line'}
        <div class="pane-block">
          <Eyebrow>Grain</Eyebrow>
          <ChipToggleGroup label="Time grain" options={timeGrains} selected={activeGrain}
            onSelect={(value) => onSelectGrain(value as TimeGrain)} />
        </div>
      {/if}
```

Add `timeGrains` to the pane's value import and `TimeGrain` to its type import. `activeGrain` is a new prop holding the grain the **response** reported, not the one requested, so the buttons show what is on screen: `activeGrain: TimeGrain | null`. Add it and `onSelectGrain: (grain: TimeGrain) => void` to the pane props and thread both through `VisualizeStage.svelte` and `DashboardStage.svelte`, exactly as `onSelectLayout` was threaded in Task 4 — `Props` type, destructuring, and call site in each.

In `App.svelte`:

```ts
  let visualizeGrain = $state<TimeGrain | null>(null);

  function selectVisualizeGrain(grain: TimeGrain) {
    visualizeGrain = grain;
    void loadVisualize();
  }
```

Reset it in `resetVisualizeDraft`, include `grain: visualizeGrain ?? undefined` in `visualizeSpec` when `pick.chart === 'line'`, and pass `activeGrain={visualizeData?.chart === 'line' ? visualizeData.grain : null}`.

- [ ] **Step 4: Enable the chart type**

In `App.svelte`, change the set on line 244:

```ts
  const implementedCharts = new Set<ChartType>(['bar', 'histogram', 'box', 'scatter', 'line']);
```

In `DashboardStage.svelte`, drop the filter:

```ts
  let editSuggestions = $derived(suggestCharts(editColumns));
```

In `VisualizeStage.svelte`, add the line case to `status` and update the empty-state copy:

```ts
    if (data.chart === 'line') {
      const points = data.series.reduce((sum, line) => sum + line.points.length, 0);
      return `${data.series.length} ${data.series.length === 1 ? 'series' : 'series'} · ${count(points)} points by ${data.grain} · ${data.elapsed_ms.toFixed(1)} ms`;
    }
```

Change both "Bar, histogram, box, and scatter are ready for this column mix." strings — in `VisualizeStage.svelte` and `ChartView.svelte` — to "Bar, line, pie, histogram, box, and scatter are ready for this column mix."

- [ ] **Step 5: Typecheck and verify in the browser**

```bash
cd frontend && npx svelte-check --threshold error
```

Select a date column and a numeric column, confirm the line renders, the grain buttons appear, and switching from day to month redraws with fewer points and updates the status line's reported grain.

- [ ] **Step 6: Commit**

```bash
git add frontend/src
git commit -m "$(printf 'feat(charts): add the line chart with time grain controls\n\nCo-Authored-By: Claude Opus 5 <noreply@anthropic.com>')"
```

---

## Task 9: Pie chart

**Objective:** Add the pie chart as a tighter-limit variant of the bar query, plus its component and icon.

**Files:**
- Modify: `backend/app.py` (bar branch), `frontend/src/lib/visualize.ts`, `frontend/src/lib/icons.ts`, `frontend/src/components/atoms/Icon.svelte`, `frontend/src/components/molecules/ChartView.svelte`, `frontend/src/components/organisms/VisualizeStage.svelte`, `frontend/src/App.svelte`
- Create: `frontend/src/components/molecules/PieChart.svelte`
- Test: `tests/test_backend.py`, `frontend/tests/visualize.test.js`

**Interfaces:**
- Consumes: the bar branch (Task 3), `ChartMark` with `series` (Task 4).
- Produces: `chart: 'pie'` responses shaped like bar, and `chartMeta.pie`.

- [ ] **Step 1: Write the failing tests**

```python
def test_visualize_pie_uses_the_tighter_slice_limit(client):
    body = b"tag,amount\n" + b"".join(f"t{i},{i}\n".encode() for i in range(20))
    node = upload(client, "slices.csv", body)
    base = f"/api/nodes/{node['id']}/datasets/{dataset(client, node, 'slices')['id']}"
    payload = client.post(base + "/visualize", json={
        "spec": {"chart": "pie", "encodings": {"category": "tag", "value": "amount"}, "metric": "sum"},
    }).json()
    assert payload["chart"] == "pie"
    assert len(payload["rows"]) == 8
    assert payload["other_count"] > 0


def test_visualize_pie_rejects_negative_values(client):
    node = upload(client, "negative.csv", b"tag,amount\na,5\nb,-3\n")
    base = f"/api/nodes/{node['id']}/datasets/{dataset(client, node, 'negative')['id']}"
    response = client.post(base + "/visualize", json={
        "spec": {"chart": "pie", "encodings": {"category": "tag", "value": "amount"}, "metric": "sum"},
    })
    assert response.status_code == 422
    assert "negative" in response.json()["detail"].lower()
```

```js
test('a pie chart is suggested for a single categorical column', () => {
  assert.equal(charts([region]).includes('pie'), true);
});
```

- [ ] **Step 2: Run the tests and verify they fail**

```bash
uv run pytest tests/test_backend.py -k pie -v
```

Expected: FAIL — `pie` is rejected by the spec model or falls through to the final 422.

- [ ] **Step 3: Implement the backend variant**

Change the bar branch's opening condition and its limit, and add the guard before the return:

```python
        if spec.chart in {"bar", "pie"}:
            column = spec.encodings.category
            if not column or column not in columns:
                raise HTTPException(422, f"{spec.chart.capitalize()} requires a category encoding")
            limit = PIE_LIMIT if spec.chart == "pie" else BAR_LIMIT
```

Replace `LIMIT {BAR_LIMIT}` in the rows query with `LIMIT {limit}`. Replace every other occurrence of the literal `"bar"` in that branch's messages and its returned `"chart"` value with `spec.chart`, and add before the return:

```python
            if spec.chart == "pie":
                if any(value is not None and float(value) < 0 for _label, value, _n in rows):
                    raise HTTPException(422, "A pie chart cannot show negative values; use a bar chart instead")
                series_labels = None
                series_values = {}
```

Pie ignores any group encoding, which the two lines above enforce.

- [ ] **Step 4: Add the icon and the suggestion**

In `frontend/src/lib/icons.ts`, add `| 'pie'` to `IconName`. In `frontend/src/components/atoms/Icon.svelte`, add to the path map:

```ts
    pie: 'M8 2.75a5.25 5.25 0 1 0 5.25 5.25H8Z M8 2.75V8h5.25A5.25 5.25 0 0 0 8 2.75Z',
```

In `frontend/src/lib/visualize.ts`, add the meta entry:

```ts
  pie: { label: 'Pie', tip: 'Shares of a whole', icon: 'pie' },
```

and offer pie beside bar wherever a single categorical column is suggested. Change `barCount` usage in the one-item branch:

```ts
    if (item.kind === 'categorical') return [barCount(item), { chart: 'pie', encodings: { category: item.name }, metric: 'count' }];
```

and in the `cats.length === 1 && nums.length === 1` branch, append `{ chart: 'pie', encodings: { category: cats[0].name, value: nums[0].name }, metric: 'sum' }` to the returned list.

- [ ] **Step 5: Create `PieChart.svelte`**

```svelte
<script lang="ts">
  import type { AggregateCount, BarVisualizeRow, ChartHover } from '../../lib/types';

  type Props = {
    rows: BarVisualizeRow[];
    title: string;
    count: (value: AggregateCount) => string;
    compact: (value: number | string | null | undefined) => string;
    aggregated: boolean;
    onSelect: (label: string | boolean | number) => void;
  };
  let { rows, title, count, compact, aggregated, onSelect }: Props = $props();

  let hover = $state<ChartHover | null>(null);
  let total = $derived(rows.reduce((sum, row) => sum + Number(row.value), 0));
  let slices = $derived.by(() => {
    let angle = -Math.PI / 2;
    return rows.map((row, index) => {
      const share = total ? Number(row.value) / total : 0;
      const start = angle;
      angle += share * Math.PI * 2;
      return { row, index, share, start, end: angle };
    });
  });

  function arc(start: number, end: number): string {
    const r = 74;
    const x0 = 100 + r * Math.cos(start);
    const y0 = 100 + r * Math.sin(start);
    const x1 = 100 + r * Math.cos(end);
    const y1 = 100 + r * Math.sin(end);
    const large = end - start > Math.PI ? 1 : 0;
    return `M100 100 L${x0.toFixed(2)} ${y0.toFixed(2)} A${r} ${r} 0 ${large} 1 ${x1.toFixed(2)} ${y1.toFixed(2)} Z`;
  }

  function describe(row: BarVisualizeRow, share: number): ChartHover {
    return {
      title: String(row.label),
      lines: [aggregated ? `${title}: ${compact(row.value)}` : `${count(row.value)} rows`, `${(share * 100).toFixed(1)}% of this chart`],
      hint: 'Click to filter this value'
    };
  }
</script>

{#if rows.length}
  <div class="pie-wrap">
    <svg viewBox="0 0 200 200" role="img" aria-label={`Pie chart of ${title}`}>
      {#each slices as slice (String(slice.row.label))}
        <path
          d={arc(slice.start, slice.end)}
          class="slice"
          style={`--chart-slice-fill:var(--chart-series-${(slice.index % 6) + 1})`}
          tabindex="0"
          role="button"
          aria-label={`${slice.row.label}: ${(slice.share * 100).toFixed(1)}%. Filter to this value.`}
          onclick={() => onSelect(slice.row.label)}
          onkeydown={(event) => { if (event.key === 'Enter' || event.key === ' ') { event.preventDefault(); onSelect(slice.row.label); } }}
          onpointerenter={() => hover = describe(slice.row, slice.share)}
          onpointerleave={() => hover = null}
          onfocus={() => hover = describe(slice.row, slice.share)}
          onblur={() => hover = null}
        />
      {/each}
    </svg>
    {#if hover}
      <div class="hover" role="status">
        <strong>{hover.title}</strong>
        {#each hover.lines as line (line)}<span>{line}</span>{/each}
      </div>
    {/if}
  </div>
{:else}
  <p class="empty">No values to chart.</p>
{/if}

<style>
  .pie-wrap { position: relative; flex: 1; min-width: 0; min-height: 0; display: flex; align-items: center; justify-content: center; padding: 8px; }
  svg { width: min(100%, 320px); height: auto; }
  .slice { fill: var(--chart-slice-fill, var(--chart-mark)); stroke: var(--surface); stroke-width: 1.5px; cursor: pointer; transform-origin: 100px 100px; }
  .slice:hover, .slice:focus-visible { outline: none; stroke: var(--chart-mark-strong); }
  .hover { position: absolute; left: 8px; top: 8px; display: flex; flex-direction: column; gap: 2px; padding: 6px 8px; border: 1px solid var(--line); border-radius: var(--radius-md); background: var(--surface); font-size: 11px; color: var(--muted); }
  .hover strong { color: var(--ink); }
  .empty { margin: auto; font-size: 12.5px; color: var(--muted); }
  @media (prefers-reduced-motion: no-preference) {
    .slice { transition: transform 160ms cubic-bezier(0.22, 1, 0.36, 1); }
    .slice:hover, .slice:focus-visible { transform: scale(1.02); }
  }
</style>
```

- [ ] **Step 6: Wire it up**

In `ChartView.svelte`, import `PieChart` and add the branch:

```svelte
  {:else if data?.chart === 'pie'}
    <PieChart rows={data.rows} title={yTitle} {count} {compact} {aggregated}
      onSelect={(value) => onMark({ kind: 'category', value })} />
```

Add `'pie'` to `implementedCharts` in `App.svelte`. Add the pie case to the `status` derivation in `VisualizeStage.svelte`, above the bar case:

```ts
    if (data.chart === 'pie') {
      const extra = Number(data.other_count) > 0 ? ` · ${count(data.other_count)} in Other` : '';
      return `${data.rows.length} ${data.rows.length === 1 ? 'slice' : 'slices'}${extra} · ${data.elapsed_ms.toFixed(1)} ms`;
    }
```

In `types.ts`, add the response type and put it in the union. Task 7 may or may not have landed first, so add `PieVisualizeResponse` to whatever the union currently holds rather than retyping it wholesale:

```ts
export interface PieVisualizeResponse extends Omit<BarVisualizeResponse, 'chart'> { chart: 'pie' }
```

- [ ] **Step 7: Run both suites and verify they pass**

```bash
uv run pytest tests/test_backend.py -k pie -v && cd frontend && npm test && npx svelte-check --threshold error
```

Expected: PASS, no type errors.

- [ ] **Step 8: Commit**

```bash
git add backend/app.py tests/test_backend.py frontend/src frontend/tests
git commit -m "$(printf 'feat(charts): add a pie chart over the bar aggregation\n\nCo-Authored-By: Claude Opus 5 <noreply@anthropic.com>')"
```

---

## Task 10: Grouped histogram

**Objective:** Make `encodings.group` real for histograms, with every group sharing the ungrouped bin edges.

**Files:**
- Modify: `backend/app.py` (histogram branch), `frontend/src/lib/types.ts`, `frontend/src/components/molecules/HistogramPlot.svelte`, `frontend/src/components/molecules/ChartView.svelte`
- Test: `tests/test_backend.py`

**Interfaces:**
- Consumes: `SERIES_LIMIT` (Task 3), `ChartLegend` (Task 4).
- Produces: `HistogramVisualizeResponse.series?: { label: string; bins: HistogramBin[] }[]`.

- [ ] **Step 1: Write the failing tests**

```python
def test_visualize_histogram_groups_share_bin_edges(client):
    body = b"region,amount\n" + b"".join(
        f"{'east' if i % 2 else 'west'},{i}\n".encode() for i in range(1, 101)
    )
    node = upload(client, "grouphist.csv", body)
    base = f"/api/nodes/{node['id']}/datasets/{dataset(client, node, 'grouphist')['id']}"
    payload = client.post(base + "/visualize", json={
        "spec": {"chart": "histogram", "encodings": {"value": "amount", "group": "region"}},
    }).json()
    assert sorted(series["label"] for series in payload["series"]) == ["east", "west"]
    edges = [(item["lower"], item["upper"]) for item in payload["bins"]]
    for series in payload["series"]:
        assert [(item["lower"], item["upper"]) for item in series["bins"]] == edges
    assert sum(item["count"] for item in payload["bins"]) == 100


def test_visualize_histogram_without_group_omits_series(client):
    node = upload(client, "plainhist.csv", b"amount\n1\n2\n3\n4\n")
    base = f"/api/nodes/{node['id']}/datasets/{dataset(client, node, 'plainhist')['id']}"
    payload = client.post(base + "/visualize", json={"spec": {"chart": "histogram", "encodings": {"value": "amount"}}}).json()
    assert "series" not in payload


def test_visualize_histogram_skips_series_for_a_date_column(client):
    body = b"region,day\n" + b"".join(
        f"{'east' if i % 2 else 'west'},2026-01-{i:02d}\n".encode() for i in range(1, 29)
    )
    node = upload(client, "datehist.csv", body)
    base = f"/api/nodes/{node['id']}/datasets/{dataset(client, node, 'datehist')['id']}"
    payload = client.post(base + "/visualize", json={
        "spec": {"chart": "histogram", "encodings": {"value": "day", "group": "region"}},
    }).json()
    assert payload["bins"]
    assert "series" not in payload
```

- [ ] **Step 2: Run the tests and verify they fail**

```bash
uv run pytest tests/test_backend.py -k histogram -v
```

Expected: FAIL — no `series` key.

- [ ] **Step 3: Implement the grouped branch**

Replace the `if spec.chart == "histogram":` block's return:

```python
            profile = profile_response(con, source, values, metadata_columns, column)
            bins = profile["histogram"]
            response = {
                "chart": "histogram",
                "bins": bins,
                "elapsed_ms": round((time.perf_counter() - started) * 1000, 3),
            }
            group_name = spec.encodings.group
            if group_name and group_name not in columns:
                raise HTTPException(422, "Histogram group column was not found")
            # ponytail: grouped counts need equal-width bins; date and huge-integer columns use
            # ntile bins instead, so they fall back to the single distribution.
            uniform = (
                profile_kind(columns[column]) == "numeric"
                and len(bins) >= 2
                and max(abs((item["upper"] - item["lower"]) - (bins[0]["upper"] - bins[0]["lower"])) for item in bins)
                <= abs(bins[0]["upper"] - bins[0]["lower"]) * 1e-6
            )
            if group_name and uniform:
                field = quote(column)
                group_field = quote(group_name)
                lower = bins[0]["lower"]
                width = bins[0]["upper"] - bins[0]["lower"]
                finite = f"{field} IS NOT NULL AND isfinite({field}::DOUBLE) AND {group_field} IS NOT NULL"
                ranked = con.execute(f"""
                    SELECT {group_field} AS series, count(*) AS n
                    FROM {source} WHERE {finite}
                    GROUP BY 1 ORDER BY n DESC, series
                    LIMIT {SERIES_LIMIT}
                """, values).fetchall()
                top = [row[0] for row in ranked]
                if top:
                    counted = con.execute(f"""
                        SELECT {group_field} AS series,
                               least(?, floor(({field} - ?) / ?)::INTEGER) AS idx,
                               count(*) AS n
                        FROM {source} WHERE {finite}
                        GROUP BY 1, 2
                    """, [len(bins) - 1, lower, width, *values]).fetchall()
                    kept = {str(item) for item in top}
                    buckets: dict[str, dict[int, int]] = {}
                    for series_label, index, n in counted:
                        key = str(series_label) if str(series_label) in kept else "Other"
                        slot = buckets.setdefault(key, {})
                        slot[int(index)] = slot.get(int(index), 0) + n
                    labels = [str(item) for item in top]
                    if "Other" in buckets:
                        labels.append("Other")
                    response["series"] = [{
                        "label": label,
                        "bins": [{
                            "lower": item["lower"],
                            "upper": item["upper"],
                            "count": safe(buckets.get(label, {}).get(index, 0)),
                        } for index, item in enumerate(bins)],
                    } for label in labels]
            return response
```

DuckDB binds `?` in statement order, and here the three bucket parameters sit in the `SELECT` ahead of `{source}` in the `FROM` — hence `[len(bins) - 1, lower, width, *values]`, the same order the existing numeric-profile histogram query already uses. The `Other` fold happens in Python rather than in SQL precisely to keep every remaining placeholder out of the `SELECT`.

- [ ] **Step 4: Add the frontend type and rendering**

```ts
export interface HistogramVisualizeResponse {
  chart: 'histogram';
  bins: HistogramBin[];
  series?: { label: string; bins: HistogramBin[] }[];
  elapsed_ms: number;
}
```

In `HistogramPlot.svelte`, add `series?: { label: string; bins: HistogramBin[] }[]` to the props and, when it is present, draw one translucent rect set per series over the shared x domain instead of the single set, keeping the existing `max` derivation across every series' counts:

```ts
  let allBins = $derived(series ? series.flatMap((item) => item.bins) : bins);
  let max = $derived(Math.max(1, ...allBins.map((bin) => Number(bin.count))));
```

Then draw one rect set per series over the shared x domain, replacing the single `{#each bins ...}` loop body with:

```svelte
      {#each (series ?? [{ label: '', bins }]) as line, lineIndex (line.label)}
        {#each line.bins as bin, index (`${line.label}:${index}`)}
          {@const left = plot.x + ((toPlotNumber(bin.lower) - domain.start) / domain.span) * plot.width}
          {@const right = plot.x + ((toPlotNumber(bin.upper) - domain.start) / domain.span) * plot.width}
          {@const height = (Number(bin.count) / yScale.max) * plot.height}
          <rect
            x={left} y={plot.y + plot.height - height}
            width={Math.max(1, right - left)} height={Math.max(0, height)}
            class="bin"
            class:overlaid={!!series}
            style={`--chart-bin-fill:var(--chart-series-${(lineIndex % 6) + 1}-fill)`}
            tabindex={series ? -1 : 0}
            role={series ? 'presentation' : 'button'}
            aria-label={`${binLabel(bin)}: ${count(bin.count)} rows.`}
            onclick={() => { if (!series) onSelect(bin, index === line.bins.length - 1); }}
            onpointerenter={() => hover = { title: binLabel(bin), lines: [`${line.label ? `${line.label} · ` : ''}${count(bin.count)} rows`] }}
            onfocus={() => hover = { title: binLabel(bin), lines: [`${count(bin.count)} rows`] }}
          />
        {/each}
      {/each}
```

with these styles added:

```css
  .bin { fill: var(--chart-bin-fill, var(--chart-mark)); }
  .bin.overlaid { fill-opacity: .55; stroke: none; }
```

Clicking is disabled while overlaid: a bin belongs to several series at once, so there is no single value for `filtersFromMark` to filter to.

In `ChartView.svelte`, wrap the histogram branch in the same `.stack` used for bar and show a `ChartLegend` when `data.series` is present:

```svelte
  {:else if data?.chart === 'histogram'}
    <div class="stack">
      <HistogramPlot bins={data.bins} series={data.series} {xTitle} {count} {binLabel}
        onSelect={(bin, last) => onMark({ kind: 'bin', lower: bin.lower, upper: bin.upper, last })} />
      {#if data.series}
        <ChartLegend kind="series" title={spec.encodings.group ?? 'Series'}
          items={data.series.map((line, index) => ({ label: line.label, color: `var(--chart-series-${(index % 6) + 1})` }))} />
      {/if}
    </div>
```

- [ ] **Step 5: Run both suites and verify they pass**

```bash
uv run pytest tests/test_backend.py -k histogram -v && cd frontend && npm test && npx svelte-check --threshold error
```

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add backend/app.py tests/test_backend.py frontend/src
git commit -m "$(printf 'feat(charts): overlay grouped histograms on shared bin edges\n\nCo-Authored-By: Claude Opus 5 <noreply@anthropic.com>')"
```

---

## Final verification

- [ ] **Run the whole suite**

```bash
uv run pytest tests/test_backend.py -q && cd frontend && npm test && npx svelte-check --threshold error
```

- [ ] **Walk the feature in the browser**

Confirm each of: a grouped bar switching between all three layouts; a scatter carrying size, shape, and continuous colour at once with four legends; a line chart whose grain buttons change the point density and whose status line reports the grain in use; a pie chart whose slices filter; and an overlaid grouped histogram.

- [ ] **Update `docs/plans/2026-09-21-visualization-engine-design.md`** with a one-line status note recording that the plan shipped, and commit.
