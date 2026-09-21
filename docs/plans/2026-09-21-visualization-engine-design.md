# Visualization Engine: Multi-Group Controls and New Chart Types — Design

**Status:** Approved design. Implementation plan to follow.

**Goal:** Give the visualization engine user-controlled encoding roles, series-aware bar charts, real scatter scales for size, color, and shape, a date-axis line chart with time-grain controls, and a pie chart.

---

## Background: how the engine works today

The engine is suggestion-driven. The user toggles columns in `ChartOptionsPane`; `suggestCharts` in `frontend/src/lib/visualize.ts` classifies them (`categorical`, `discrete`, `continuous`, `date`) and returns a list of `ChartSuggestion` values, each carrying a chart type and a complete set of encodings. `visualizeSpec` in `App.svelte` picks the suggestion matching the user's chosen chart type and sends the resulting `ChartSpec` to `POST /api/nodes/{id}/datasets/{dataset}/visualize`. The backend's `visualize_response` in `backend/app.py` branches on `spec.chart` and returns one of four response shapes.

Encodings are never authored. They are derived entirely from which columns are selected and in what order: with two numeric columns, `nums[0]` becomes x and `nums[1]` becomes y, and the user has no way to swap them or to say that a third column is the size rather than the color.

Four gaps stand between that engine and this work:

- The bar branch ignores `encodings.group` completely. No series appear in the SQL or in the response.
- The scatter branch reads `encodings.color` as an opaque passthrough value. `size` and `pattern` exist in `ChartEncodings` but nothing consumes them, and there is no legend or scale for any channel.
- `line` is suggested by `suggestCharts` but unimplemented: the backend falls through to `raise HTTPException(422, ...)`, `implementedCharts` in `App.svelte` excludes it, and `DashboardStage` filters it out explicitly.
- `pie` does not exist in any layer.

## Decisions taken

| Question | Decision |
|---|---|
| Encoding authoring model | Role chips on selected columns. Inference stays the default; a chip overrides one role. |
| Grouped vs stacked bar | A `layout` field on `ChartSpec`, with `grouped`, `stacked`, and `stacked100`. Not new chart types. |
| Scatter channels | All three: size, color, shape. Color is a continuous scale for numeric columns, defaulting to the column's min→max domain. |
| Line data shaping | Auto time-bucketing, plus explicit grain buttons in the pane. |
| Line x-axis | Date only. The numeric-x line suggestion is dropped. |
| Pie | Reuses the bar query with a tighter slice limit. New component only. |
| Group role reach | Bar, line, and histogram. Box already honours `group` server-side. |

---

## 1. Spec model

`ChartEncodings` already carries every role this work needs — `x`, `y`, `category`, `value`, `group`, `size`, `color`, `pattern`. Nothing is added to it. What changes is that roles become editable rather than purely inferred.

```ts
// frontend/src/lib/types.ts
export type ChartType = 'bar' | 'histogram' | 'box' | 'scatter' | 'line' | 'pie';
export type EncodingRole = keyof ChartEncodings;
export type BarLayout = 'grouped' | 'stacked' | 'stacked100';
export type TimeGrain = 'hour' | 'day' | 'week' | 'month' | 'quarter' | 'year';

export interface ChartSpec {
  chart: ChartType;
  encodings: ChartEncodings;
  metric?: AggregateMetric;
  density?: boolean;
  layout?: BarLayout;   // bar only; absent means 'grouped'
  grain?: TimeGrain;    // line only; absent means auto
}
```

The backend's `ChartSpec` model in `backend/app.py` mirrors this: `chart` gains `"pie"`, and `layout` and `grain` are added as optional `Literal` fields. `model_config = ConfigDict(extra="forbid")` stays, so an unknown field is still a 422.

### Role overrides

`App.svelte` gains one piece of state beside `visualizeColumns`:

```ts
let visualizeRoles = $state<Record<string, EncodingRole>>({});
```

`visualizeSpec` takes the chosen suggestion's encodings and applies the overrides on top of them. A new pure helper in `visualize.ts` does the work and is what the tests target:

```ts
export function applyRoles(
  encodings: ChartEncodings,
  chart: ChartType,
  overrides: Record<string, EncodingRole>,
  selected: ColumnInfo[]
): ChartEncodings;
```

Applying an override means clearing the column from whatever role inference gave it, then setting it on the requested role — a role holds at most one column, so assigning a column to an occupied role displaces the previous occupant back to unassigned.

`chartRoles(chart: ChartType): EncodingRole[]` states which roles each chart accepts, and the chip's role menu offers only those:

| Chart | Roles |
|---|---|
| `bar` | `category`, `value`, `group` |
| `pie` | `category`, `value` |
| `histogram` | `value`, `group` |
| `box` | `value`, `group` |
| `scatter` | `x`, `y`, `size`, `color`, `pattern` |
| `line` | `x`, `y`, `group` |

Overrides are self-healing, exactly as `visualizeChart` already is in `toggleVisualizeColumn`. An override is dropped when its column leaves the selection, or when the active chart type does not accept that role. Switching chart types therefore never produces an invalid spec; it silently falls back to inference for the roles that no longer apply.

Inference remains the default path. A user who never touches a chip sees today's behaviour unchanged.

---

## 2. Backend response shapes

### Bar with series

`BarVisualizeResponse` gains `series`, and rows gain `values` aligned positionally to it:

```ts
export interface BarVisualizeRow {
  label: string | boolean | number;
  value: AggregateCount;
  n?: AggregateCount;
  values?: AggregateCount[];   // present when series is present; aligned to series
}

export interface BarVisualizeResponse {
  chart: 'bar';
  rows: BarVisualizeRow[];
  series: string[] | null;     // null when no group encoding
  other_count: AggregateCount;
  elapsed_ms: number;
}
```

Ungrouped responses keep `series: null` and omit `values`, so every existing consumer — `BarChart.svelte`, the `status` derivation in `VisualizeStage.svelte`, the dashboard tiles — keeps working untouched.

The grouped query ranks categories and series independently, then aggregates the cross product:

1. Top `BAR_LIMIT` (30) categories by total measure, as today.
2. Top `SERIES_LIMIT` (6) group values by total measure. Six matches the palette's `series` array length in `chartThemes.ts`, so every series has a distinct colour.
3. Remaining group values fold into a single `Other` series rather than being dropped, so stacked totals stay honest against the ungrouped chart.
4. `GROUP BY category, series`, with missing combinations filled as `0`.

`grouped`, `stacked`, and `stacked100` all issue the identical query. Layout is purely a rendering choice, which is why it is a spec field rather than three chart types.

### Line

A new branch and a new response type:

```ts
export interface LinePoint { x: string; y: NumericValue }
export interface LineSeries { label: string; points: LinePoint[] }

export interface LineVisualizeResponse {
  chart: 'line';
  grain: TimeGrain;
  series: LineSeries[];   // one series with label '' when no group encoding
  elapsed_ms: number;
}
```

The branch requires a date `x` and rejects anything else with a 422 naming the column. `y` is optional: with `y` present the metric applies to it (`avg` by default); without `y` the measure is `count(*)`.

Bucketing uses `date_trunc(grain, x)`. Auto grain runs one `min`/`max` query on x and picks the smallest grain whose bucket count fits `LINE_POINT_LIMIT` (400), walking `hour → day → week → month → quarter → year`. If even `year` overflows the limit — decades of data — `year` is used and the series is truncated to the most recent `LINE_POINT_LIMIT` buckets.

`spec.grain` overrides the automatic choice. The response reports the grain actually used in every case, so the pane's grain buttons show what is on screen rather than what was asked for.

The group encoding produces one series per value, ranked and capped by `SERIES_LIMIT` with an `Other` fold, matching bar.

### Scatter scales

`ScatterPoint` gains the two new channels, and the response gains the scale metadata the component needs:

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
  color_domain?: [NumericValue, NumericValue];   // numeric color: min→max
  color_labels?: string[];                       // categorical color
  shape_labels?: string[];
  size_domain?: [NumericValue, NumericValue];
  elapsed_ms: number;
}
```

The domains and label sets are computed over the **whole filtered relation**, not over the `SCATTER_LIMIT` sample. This matters: the sample is `ORDER BY random()`, so scales derived from it would shift on every refetch and the same point would change colour between loads. One extra aggregate query per active channel buys a stable scale.

`color_kind` follows `profile_kind` of the colour column — `numeric` gets a continuous ramp over `color_domain`, anything else gets the categorical palette. Categorical `color_labels` and `shape_labels` are capped at 6 with an `Other` fold, again matching the palette width and the shape-marker count.

Size must be numeric; a non-numeric size column is a 422.

### Pie

Pie runs the bar branch with `PIE_LIMIT` (8) in place of `BAR_LIMIT` and returns `chart: 'pie'` with otherwise bar-shaped rows. No separate SQL and no separate aggregation logic.

One guard is specific to pie: if any slice value is negative, the request fails with a 422 explaining that a pie cannot represent negative values and suggesting a bar chart. A pie of mixed-sign values is not a rendering problem to solve in the component — the shape is meaningless.

Pie ignores `layout` and accepts no group encoding.

### Grouped histogram

`suggestCharts` already emits `{ group, value }` for histogram while the backend ignores `group` and returns one distribution. Making it real requires groups to share bin edges, or the overlaid distributions are not comparable.

Edges come from the existing ungrouped `profile_response` for the value column. A second query then counts rows per group per bucket using those edges. The response keeps `bins` for the ungrouped case and adds `series`:

```ts
export interface HistogramVisualizeResponse {
  chart: 'histogram';
  bins: HistogramBin[];                              // shared edges; counts are the total across shown groups
  series?: { label: string; bins: HistogramBin[] }[];
  elapsed_ms: number;
}
```

Groups are capped by `SERIES_LIMIT` with an `Other` fold. Rendering is overlaid translucent distributions rather than facets, which keeps the existing single-plot `ChartFrame` layout.

---

## 3. Frontend components

New components:

- `molecules/LineChart.svelte` — multi-series line over a date axis, using the existing `ChartFrame` and `formatPlotTick` for date ticks.
- `molecules/PieChart.svelte` — slices from bar-shaped rows, click-to-filter per slice.
- `molecules/ChartLegend.svelte` — one legend used by every channel that needs one: bar series, line series, histogram series, scatter categorical colour, scatter shape, scatter size, and scatter continuous colour. The continuous case renders a gradient bar with min and max labels; the others render swatch-and-label rows.

Extended components:

- `molecules/BarChart.svelte` — series bands for `grouped`, running accumulation for `stacked`, and share-of-row-total normalization for `stacked100`. Ungrouped rendering is unchanged.
- `molecules/ScatterPlot.svelte` — radius from `sqrt` of the normalized size value, so area rather than radius encodes magnitude and the perceived quantity is true; six canvas marker shapes (circle, square, triangle, diamond, cross, triangle-down); continuous colour interpolated between `--chart-mark` and `--chart-mark-strong`.

The continuous colour ramp deliberately reuses `--chart-mark` and `--chart-mark-strong`. Every palette in `chartThemes.ts` defines both as a light-to-dark pair of the same hue, in both light and dark variants, which is exactly a sequential scale. Interpolating the `series` array instead would give a rainbow ramp in the `multicolor` palettes, which reads as unordered and defeats the purpose.

### Atomic-design debt this creates, and pays

`ChartOptionsPane` holds the metric chip row as inline markup with local `.metric-chip` styles. This work would add a layout toggle and a grain button row in the same shape, making three near-identical copies. Rather than duplicate it twice more, the row is extracted once into `molecules/ChipToggleGroup.svelte` and the existing metrics block moves onto it. This is the targeted kind of cleanup — code the work already touches — not unrelated refactoring.

The role chip composes the existing `atoms/Chip.svelte` and `molecules/MenuPopover.svelte`. No new primitive.

---

## 4. Marks and filtering

Clicking a grouped bar, a stacked segment, a pie slice, or a line point should filter on both the category and the series. `ChartMark` gains one optional field:

```ts
export type ChartMark =
  | { kind: 'category'; value: string | boolean | number; series?: string | boolean | number }
  | { kind: 'bin'; lower: NumericValue; upper: NumericValue; last?: boolean }
  | { kind: 'region'; xMin: NumericValue; xMax: NumericValue; yMin: NumericValue; yMax: NumericValue };
```

`filtersFromMark` emits a second condition on `encodings.group` when `series` is present. Every chart that can produce a series-aware click routes through this one function, so one change covers all of them. The `Other` series is not clickable — it has no single value to filter to — and renders without the pointer affordance.

Smaller call-site updates:

- `dashboard.ts` `chartTitle` includes the group, size, and colour columns in its field list.
- `App.svelte` `implementedCharts` gains `line` and `pie`.
- `DashboardStage.svelte` drops its `item.chart !== 'line'` filter.
- `chartMeta` gains a `pie` entry; `IconName` and `atoms/Icon.svelte` gain a `pie` path.
- `ChartView.svelte` and `VisualizeStage.svelte` gain `line` and `pie` branches, and the `status` line reports the line chart's grain and series count.
- The "Bar, histogram, box, and scatter are ready for this column mix" empty-state copy in both `ChartView.svelte` and `VisualizeStage.svelte` is updated.

---

## 5. Motion

Per the project's interaction rule, a control grows out of what the user just did rather than appearing beside it:

- The role chip expands in place into its role list. It does not open a menu next to itself; the chip becomes the picker and collapses back to a chip once a role is picked — the element the user clicked is the element that answers.
- Switching `grouped` ↔ `stacked` animates bars between their two positions rather than re-rendering. Same data, same marks, so the transition is a position tween.
- Changing grain morphs the line path rather than swapping it out.
- Scatter channel changes cross-fade point colour and tween radius.

Each of these has a `prefers-reduced-motion: reduce` path that applies the end state directly. One authored moment per interaction; no motion that delays reading the chart.

---

## 6. Testing

Backend, in `tests/test_backend.py`, following the existing style:

- Grouped bar returns aligned `series` and `values`, with the `Other` fold covering values beyond `SERIES_LIMIT`.
- `stacked` and `stacked100` return rows identical to `grouped` — layout does not touch the query.
- Line auto-grain picks the expected grain for a short span and for a long one.
- Explicit `spec.grain` overrides auto, and the response echoes the grain used.
- Grouped line returns one series per value, capped and folded.
- Line rejects a non-date x with a 422.
- Pie honours `PIE_LIMIT` and rejects negative measure values.
- Scatter size, colour, and shape domains are computed over the full relation, not the sample — asserted by making the sample smaller than the data and checking the domain still spans the full range.
- Scatter rejects a non-numeric size column.
- Grouped histogram series share bin edges with each other and with the ungrouped histogram.

Frontend, in `frontend/tests/visualize.test.js` under `node:test`:

- `applyRoles` overrides a role, displaces an occupied role, and leaves untouched roles to inference.
- Overrides are dropped when the column leaves the selection or the chart rejects the role.
- `chartRoles` returns only legal roles per chart type.
- `filtersFromMark` emits the group condition when `series` is present and omits it when absent.
- Auto-grain selection, as a pure function shared with the display logic.

---

## Implementation phasing

One foundation, then five independent pieces:

1. **Role chips and spec model** — types, `applyRoles`, `chartRoles`, `ChipToggleGroup`, the chip UI, and the App state. Everything else depends on this.
2. **Bar series** — grouped, stacked, 100% stacked, plus the series legend and the series-aware mark.
3. **Scatter scales** — size, colour, shape, and their legends.
4. **Line chart** — backend branch, auto grain, grain buttons, component.
5. **Pie chart** — backend limit and guard, component, icon.
6. **Grouped histogram** — shared edges and overlaid series.

Phases 2 through 6 can be built in any order once phase 1 lands.

## Out of scope

- Faceting or small multiples. The group role produces series within one plot, not a grid of plots.
- Dual y-axes.
- Numeric-x line charts. Explicitly dropped; two unordered numbers are a scatter.
- User-editable colour scale domains. Scatter colour defaults to the column's min→max and stays there.
- Continuous size or colour scales for bar and pie.
