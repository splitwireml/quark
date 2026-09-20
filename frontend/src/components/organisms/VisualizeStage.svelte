<script lang="ts">
  import Chip from '../atoms/Chip.svelte';
  import Eyebrow from '../atoms/Eyebrow.svelte';
  import TextInput from '../atoms/TextInput.svelte';
  import Button from '../atoms/Button.svelte';
  import ChartTypeToggle from '../molecules/ChartTypeToggle.svelte';
  import ChartView from '../molecules/ChartView.svelte';
  import { groupColumns, visualizeMetrics } from '../../lib/visualize';
  import type {
    AggregateCount,
    AggregateMetric,
    ChartMark,
    ChartSpec,
    ChartSuggestion,
    ChartType,
    ColumnInfo,
    HistogramBin,
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
    dashboardNames?: string[];
    onAddToDashboard?: (name: string) => void;
    onMark: (mark: ChartMark) => void;
  };

  let {
    columnSearch, setColumnSearch, columns, selected,
    onToggleColumn, suggestions, spec, onSelectChart, onSelectMetric,
    data, loading, error, count, compact, chartTheme = 'primary', binLabel,
    dashboardNames = [], onAddToDashboard, onMark
  }: Props = $props();

  let dashboardOpen = $state(false);
  let newDashboardName = $state('');

  function addDashboard(name: string) {
    const trimmed = name.trim();
    if (!trimmed) return;
    onAddToDashboard?.(trimmed);
    newDashboardName = '';
    dashboardOpen = false;
  }

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
</script>

<section class="stage" aria-label="Chart">
  <aside class="pane" aria-label="Chart options">
    {#if spec && onAddToDashboard}
      <div class="pane-block add-dashboard">
        <Button
          type="button"
          aria-expanded={dashboardOpen}
          aria-controls="visualize-dashboard-choices"
          aria-haspopup="true"
          onclick={() => dashboardOpen = !dashboardOpen}
        >Add to dashboard</Button>
        {#if dashboardOpen}
          <div id="visualize-dashboard-choices" class="dashboard-choices" role="group" aria-label="Choose a dashboard">
            {#each dashboardNames as name (name)}
              <button type="button" onclick={() => addDashboard(name)}>{name}</button>
            {/each}
            <form onsubmit={(event) => { event.preventDefault(); addDashboard(newDashboardName); }}>
              <TextInput value={newDashboardName} oninput={(event: Event) => newDashboardName = (event.currentTarget as HTMLInputElement).value} aria-label="New dashboard name" placeholder="New dashboard" maxlength="40" />
              <button type="submit" disabled={!newDashboardName.trim()}>Create</button>
            </form>
          </div>
        {/if}
      </div>
    {/if}
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
    {:else if data}
      <ChartView {spec} {data} {count} {compact} {chartTheme} {binLabel} {onMark} />
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
  .add-dashboard { position: relative; }
  .dashboard-choices { display: flex; flex-direction: column; gap: 4px; padding-top: 4px; }
  .dashboard-choices > button, .dashboard-choices form { min-height: 28px; border: 1px solid var(--control-border); border-radius: var(--radius-md); background: var(--surface-2); color: var(--ink); font-size: 11.5px; }
  .dashboard-choices > button { padding: 0 8px; text-align: left; }
  .dashboard-choices > button:hover, .dashboard-choices > button:focus-visible { border-color: var(--action); background: var(--action-tint); color: var(--action-dark); }
  .dashboard-choices form { display: flex; overflow: hidden; }
  .dashboard-choices form :global(.field) { min-width: 0; flex: 1; height: 28px; border: 0; border-radius: 0; }
  .dashboard-choices form button { padding: 0 8px; border: 0; border-left: 1px solid var(--control-border); background: var(--surface-2); color: var(--action-dark); font-size: 11px; }
  .dashboard-choices form button:disabled { opacity: 0.45; }
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
  .metric-chip.on { border-color: var(--ink-fill); background: var(--ink-fill); color: #fff; }
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
