<script lang="ts">
  import Chip from '../atoms/Chip.svelte';
  import Eyebrow from '../atoms/Eyebrow.svelte';
  import TextInput from '../atoms/TextInput.svelte';
  import ChartTypeToggle from '../molecules/ChartTypeToggle.svelte';
  import BarChart from '../molecules/BarChart.svelte';
  import HistogramPlot from '../molecules/HistogramPlot.svelte';
  import BoxPlot from '../molecules/BoxPlot.svelte';
  import ScatterPlot from '../molecules/ScatterPlot.svelte';
  import { groupColumns, metricTitle, visualizeMetrics } from '../../lib/visualize';
  import type {
    AggregateCount,
    AggregateMetric,
    BoxGroup,
    ChartSpec,
    ChartSuggestion,
    ChartType,
    ColumnInfo,
    HistogramBin,
    NumericValue,
    VisualizeResponse
  } from '../../lib/types';

  type Props = {
    columnSearch: string;
    setColumnSearch: (value: string) => void;
    columns: ColumnInfo[];
    selected: ColumnInfo[];
    onToggleColumn: (name: string) => void;
    suggestions: ChartSuggestion[];
    spec: ChartSpec | null;
    onSelectChart: (chart: ChartType) => void;
    onSelectMetric: (metric: AggregateMetric) => void;
    data: VisualizeResponse | null;
    loading: boolean;
    error: string;
    count: (value: AggregateCount) => string;
    compact: (value: number | string | null | undefined) => string;
    chartTheme?: string;
    binLabel: (bin: HistogramBin) => string;
    onSelectBar: (label: string | boolean | number) => void;
    onSelectBin: (bin: HistogramBin, last: boolean) => void;
    onSelectBox: (group: BoxGroup) => void;
    onSelectRegion: (region: { xMin: NumericValue; xMax: NumericValue; yMin: NumericValue; yMax: NumericValue }) => void;
  };

  let {
    columnSearch, setColumnSearch, columns, selected,
    onToggleColumn, suggestions, spec, onSelectChart, onSelectMetric,
    data, loading, error, count, compact, chartTheme = 'primary', binLabel,
    onSelectBar, onSelectBin, onSelectBox, onSelectRegion
  }: Props = $props();

  let selectedNames = $derived(new Set(selected.map((column) => column.name)));
  let groups = $derived.by(() => {
    const query = columnSearch.trim().toLowerCase();
    const visible = query ? columns.filter((column) => column.name.toLowerCase().includes(query)) : columns;
    return groupColumns(visible);
  });
  let status = $derived.by(() => {
    if (!data) return '';
    if (data.chart === 'bar') {
      const extra = Number(data.other_count) > 0 ? ` · ${count(data.other_count)} in Other` : '';
      return `${data.rows.length} ${data.rows.length === 1 ? 'category' : 'categories'}${extra} · ${data.elapsed_ms.toFixed(1)} ms`;
    }
    if (data.chart === 'box') {
      return `${data.groups.length} ${data.groups.length === 1 ? 'distribution' : 'groups'} · ${data.elapsed_ms.toFixed(1)} ms`;
    }
    if (data.chart === 'scatter') {
      const sampled = Number(data.total_points) > data.points.length ? ` of ${count(data.total_points)}` : '';
      return `${count(data.points.length)} points${sampled} · ${data.elapsed_ms.toFixed(1)} ms`;
    }
    return `${data.bins.length} ${data.bins.length === 1 ? 'bin' : 'bins'} · ${data.elapsed_ms.toFixed(1)} ms`;
  });
  let showAggregate = $derived(spec?.chart === 'bar' && !!spec.encodings.value);
  let xTitle = $derived(
    spec?.chart === 'box' ? (spec.encodings.group ?? '')
      : spec?.chart === 'scatter' ? (spec.encodings.x ?? '')
      : spec?.encodings.category ?? spec?.encodings.value ?? spec?.encodings.x ?? ''
  );
  let yTitle = $derived(
    spec?.chart === 'box' || spec?.chart === 'scatter' ? (spec.encodings.y ?? spec.encodings.value ?? '')
      : spec?.chart === 'bar' && spec.encodings.value ? metricTitle(spec.metric, spec.encodings.value)
      : 'Rows'
  );
</script>

<section class="stage" aria-label="Chart">
  <aside class="pane" aria-label="Chart options">
    <div class="pane-block">
      <Eyebrow>Chart</Eyebrow>
      <ChartTypeToggle {suggestions} selected={spec?.chart ?? null} onSelect={onSelectChart} />
    </div>
    {#if showAggregate}
      <div class="pane-block">
        <Eyebrow>Aggregate</Eyebrow>
        <div class="metrics" role="group" aria-label="Aggregation">
          {#each visualizeMetrics as metric (metric.value)}
            <button
              type="button"
              class="metric-chip"
              class:on={spec?.metric === metric.value}
              aria-pressed={spec?.metric === metric.value}
              data-tip={metric.tip}
              data-tip-position="top"
              onclick={() => onSelectMetric(metric.value)}
            >{metric.label}</button>
          {/each}
        </div>
      </div>
    {/if}
    {#if selected.length}
      <div class="pane-block">
        <Eyebrow>Selected</Eyebrow>
        <div class="chips">
          {#each selected as column (column.name)}
            <Chip tone="accent" onRemove={() => onToggleColumn(column.name)} removeLabel={`Remove ${column.name}`}>
              {column.name}
            </Chip>
          {/each}
        </div>
      </div>
    {/if}
    <div class="search">
      <label for="visualize-column-search" class="sr-only">Find a column to add</label>
      <TextInput
        id="visualize-column-search"
        type="search"
        glyph="⌕"
        value={columnSearch}
        oninput={(event: Event) => setColumnSearch((event.currentTarget as HTMLInputElement).value)}
        placeholder="Find a column"
      />
    </div>
    {#each groups as group (group.label)}
      <div class="pane-block">
        <Eyebrow>{group.label}</Eyebrow>
        <div class="fields" role="list" aria-label={group.label}>
          {#each group.columns as column (column.name)}
            {@const on = selectedNames.has(column.name)}
            <button
              type="button"
              class="field"
              class:on
              aria-pressed={on}
              onclick={() => onToggleColumn(column.name)}
            >
              <span title={column.name}>{column.name}</span>
              <small>{column.type}</small>
            </button>
          {/each}
        </div>
      </div>
    {:else}
      <p class="muted">{columns.length ? 'No matching columns' : 'No chartable columns'}</p>
    {/each}
  </aside>

  <div class="canvas">
    {#if loading}
      <div class="state"><span class="spinner"></span>Computing chart…</div>
    {:else if error}
      <div class="state error"><strong>Chart unavailable</strong><span>{error}</span></div>
    {:else if !spec}
      <div class="state">
        <strong>Choose a column</strong>
        <span>Numbers, categories, and dates stay grouped in the pane.</span>
      </div>
    {:else if data?.chart === 'bar'}
      <BarChart rows={data.rows} {xTitle} {yTitle} {count} {compact} aggregated={showAggregate} onSelect={onSelectBar} />
    {:else if data?.chart === 'histogram'}
      <HistogramPlot bins={data.bins} {xTitle} {count} {binLabel} onSelect={onSelectBin} />
    {:else if data?.chart === 'box'}
      <BoxPlot groups={data.groups} {xTitle} {yTitle} {compact} onSelect={onSelectBox} />
    {:else if data?.chart === 'scatter'}
      <ScatterPlot points={data.points} {xTitle} {yTitle} {compact} theme={chartTheme} {onSelectRegion} />
    {:else}
      <div class="state">
        <strong>This chart is not available yet</strong>
        <span>Bar, histogram, box, and scatter are ready for this column mix.</span>
      </div>
    {/if}
  </div>

  {#if status}
    <p class="status">{status}</p>
  {/if}
</section>

<style>
  .stage {
    position: relative;
    flex: 1;
    min-width: 0;
    min-height: 0;
    display: flex;
    background: var(--surface);
  }
  .pane {
    position: absolute;
    top: 12px;
    right: 12px;
    z-index: 3;
    display: flex;
    flex-direction: column;
    gap: 12px;
    width: 244px;
    max-height: calc(100% - 24px);
    overflow: auto;
    padding: 12px;
    border: 1px solid var(--line-strong);
    border-radius: var(--radius-2xl);
    background: var(--surface);
    box-shadow: var(--shadow-panel);
    animation: pane-in 180ms cubic-bezier(0.16, 1, 0.3, 1);
  }
  .pane-block { display: flex; flex-direction: column; gap: 6px; min-width: 0; }
  .search :global(.field) { width: 100%; }
  .chips { display: flex; flex-wrap: wrap; gap: 6px; }
  .metrics { display: flex; flex-wrap: wrap; gap: 5px; }
  .metric-chip {
    height: 26px;
    padding: 0 9px;
    border-radius: var(--radius-sm);
    border: 1px solid var(--control-border);
    background: var(--surface);
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--muted);
  }
  .metric-chip.on { border-color: var(--ink-fill); background: var(--ink-fill); color: var(--on-fill); }
  .metric-chip:hover:not(.on) { border-color: var(--faint); color: var(--ink); }
  .fields { display: flex; flex-direction: column; gap: 2px; }
  .field {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    min-height: 28px;
    padding: 4px 8px;
    border: 0;
    border-radius: var(--radius-md);
    background: transparent;
    color: var(--ink);
    text-align: left;
  }
  .field:hover, .field:focus-visible { background: var(--surface-hover); }
  .field.on { background: var(--action-tint); color: var(--action-dark); }
  .field span { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font: 11.5px var(--font-mono); }
  .field small { flex: none; font: 10px var(--font-mono); color: var(--faint); }
  .field.on small { color: var(--action-dark); }
  .canvas { flex: 1; min-width: 0; min-height: 0; display: flex; padding: 8px 268px 28px 8px; }
  .state {
    margin: auto;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 6px;
    padding: 24px;
    text-align: center;
    color: var(--muted);
    font-size: 12.5px;
  }
  .state strong { color: var(--ink); font-size: 13.5px; font-weight: 600; }
  .state.error { color: var(--error); }
  .state.error strong { color: var(--error); }
  .status {
    position: absolute;
    left: 16px;
    bottom: 8px;
    margin: 0;
    font: 11px var(--font-mono);
    color: var(--faint);
  }
  .muted { margin: 0; font-size: 11px; color: var(--faint); }
  @keyframes pane-in { from { opacity: 0; transform: translate(6px, -4px); } to { opacity: 1; transform: none; } }
  @media (prefers-reduced-motion: reduce) { .pane { animation: none; } }
</style>
