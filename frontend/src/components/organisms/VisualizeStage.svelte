<script lang="ts">
  import ChartOptionsPane from '../molecules/ChartOptionsPane.svelte';
  import ChartView from '../molecules/ChartView.svelte';
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
</script>

<section class="stage" aria-label="Chart">
  <ChartOptionsPane
    {columnSearch} {setColumnSearch} {columns} {selected} {onToggleColumn}
    {suggestions} {spec} {onSelectChart} {onSelectMetric}
    {dashboardNames} {onAddToDashboard}
  />

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
</style>
