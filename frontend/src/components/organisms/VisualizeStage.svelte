<script lang="ts">
  import ChartOptionsPane from '../molecules/ChartOptionsPane.svelte';
  import ChartView from '../molecules/ChartView.svelte';
  import type {
    AggregateCount,
    AggregateMetric,
    BarLayout,
    ChartMark,
    ChartSpec,
    ChartSuggestion,
    ChartType,
    ColumnInfo,
    EncodingRole,
    HistogramBin,
    TimeGrain,
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
    onSelectLayout: (layout: BarLayout) => void;
    onSelectGrain: (grain: TimeGrain) => void;
    activeGrain: TimeGrain | null;
    roles: EncodingRole[];
    roleOf: (name: string) => EncodingRole | null;
    onSetRole: (name: string, role: EncodingRole) => void;
    data: VisualizeResponse | null;
    loading: boolean;
    error: string;
    count: (value: AggregateCount) => string;
    compact: (value: number | string | null | undefined) => string;
    chartTheme?: string;
    binLabel: (bin: HistogramBin) => string;
    onAddToDashboard?: () => boolean;
    onMark: (mark: ChartMark) => void;
  };

  let {
    columnSearch, setColumnSearch, columns, selected,
    onToggleColumn, suggestions, spec, onSelectChart, onSelectMetric, onSelectLayout, onSelectGrain, activeGrain,
    roles, roleOf, onSetRole,
    data, loading, error, count, compact, chartTheme = 'primary', binLabel,
    onAddToDashboard, onMark
  }: Props = $props();

  let status = $derived.by(() => {
    if (!data) return '';
    if (data.chart === 'pie') {
      const extra = Number(data.other_count) > 0 ? ` · ${count(data.other_count)} in Other` : '';
      return `${data.rows.length} ${data.rows.length === 1 ? 'slice' : 'slices'}${extra} · ${data.elapsed_ms.toFixed(1)} ms`;
    }
    if (data.chart === 'bar') {
      const extra = Number(data.other_count) > 0 ? ` · ${count(data.other_count)} in Other` : '';
      return `${data.rows.length} ${data.rows.length === 1 ? 'category' : 'categories'}${extra} · ${data.elapsed_ms.toFixed(1)} ms`;
    }
    if (data.chart === 'box') {
      return `${data.groups.length} ${data.groups.length === 1 ? 'distribution' : 'groups'} · ${data.elapsed_ms.toFixed(1)} ms`;
    }
    if (data.chart === 'line') {
      const points = data.series.reduce((sum, line) => sum + line.points.length, 0);
      return `${data.series.length} ${data.series.length === 1 ? 'series' : 'series'} · ${count(points)} points by ${data.grain} · ${data.elapsed_ms.toFixed(1)} ms`;
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
    {suggestions} {spec} {onSelectChart} {onSelectMetric} {onSelectLayout} {onSelectGrain} {activeGrain}
    {roles} {roleOf} {onSetRole}
    {onAddToDashboard}
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
        <span>Bar, line, pie, histogram, box, and scatter are ready for this column mix.</span>
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
  .canvas { flex: 1; min-width: 0; min-height: 0; display: flex; padding: 8px 8px 28px; }
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
