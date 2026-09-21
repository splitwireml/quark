<script lang="ts">
  import Button from '../atoms/Button.svelte';
  import BarChart from './BarChart.svelte';
  import HistogramPlot from './HistogramPlot.svelte';
  import BoxPlot from './BoxPlot.svelte';
  import ScatterPlot from './ScatterPlot.svelte';
  import ChartLegend from './ChartLegend.svelte';
  import LineChart from './LineChart.svelte';
  import { metricTitle } from '../../lib/visualize';
  import type { AggregateCount, BoxGroup, ChartMark, ChartSpec, HistogramBin, VisualizeResponse } from '../../lib/types';

  type Props = {
    spec: ChartSpec;
    data: VisualizeResponse | null;
    loading?: boolean;
    error?: string;
    count: (value: AggregateCount) => string;
    compact: (value: number | string | null | undefined) => string;
    chartTheme?: string;
    binLabel: (bin: HistogramBin) => string;
    onMark: (mark: ChartMark) => void;
    onRetry?: () => void;
  };

  let { spec, data, loading = false, error = '', count, compact, chartTheme = 'primary', binLabel, onMark, onRetry }: Props = $props();
  let xTitle = $derived(
    spec.chart === 'box' ? (spec.encodings.group ?? '')
      : spec.chart === 'scatter' || spec.chart === 'line' ? (spec.encodings.x ?? '')
      : spec.encodings.category ?? spec.encodings.value ?? spec.encodings.x ?? ''
  );
  let yTitle = $derived(
    spec.chart === 'line' ? (spec.encodings.y ? metricTitle(spec.metric ?? 'avg', spec.encodings.y) : 'Rows')
      : spec.chart === 'box' || spec.chart === 'scatter' ? (spec.encodings.y ?? spec.encodings.value ?? '')
      : spec.chart === 'bar' && spec.encodings.value ? metricTitle(spec.metric, spec.encodings.value)
      : 'Rows'
  );
  let aggregated = $derived(spec.chart === 'bar' && !!spec.encodings.value);

  function boxMark(group: BoxGroup): ChartMark {
    return spec.encodings.group && group.label !== 'all'
      ? { kind: 'category', value: group.label }
      : { kind: 'bin', lower: group.whisker_low, upper: group.whisker_high, last: true };
  }
</script>

<div class="chart-view" aria-busy={loading}>
  {#if loading && !data}
    <div class="state" role="status" aria-live="polite"><span class="spinner"></span>Computing chart…</div>
  {:else if error}
    <div class="state error" role="alert"><strong>Chart unavailable</strong><span>{error}</span>{#if onRetry}<Button onclick={onRetry}>Retry chart</Button>{/if}</div>
  {:else if data?.chart === 'bar'}
    <div class="stack">
      <BarChart rows={data.rows} series={data.series} layout={spec.layout ?? 'grouped'} {xTitle} {yTitle} {count} {compact} {aggregated}
        onSelect={(value, series) => onMark({ kind: 'category', value, series })} />
      {#if data.series}
        <ChartLegend kind="series" title={spec.encodings.group ?? 'Series'}
          items={data.series.map((label, index) => ({ label, color: `var(--chart-group-${(index % 6) + 1})` }))} />
      {/if}
    </div>
  {:else if data?.chart === 'histogram'}
    <HistogramPlot bins={data.bins} {xTitle} {count} {binLabel} onSelect={(bin, last) => onMark({ kind: 'bin', lower: bin.lower, upper: bin.upper, last })} />
  {:else if data?.chart === 'box'}
    <BoxPlot groups={data.groups} {xTitle} {yTitle} {compact} onSelect={(group) => onMark(boxMark(group))} />
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
          items={data.color_labels.map((label, index) => ({ label, color: `var(--chart-group-${(index % 6) + 1})` }))} />
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
  {:else if data?.chart === 'line'}
    <!-- ponytail: a line mark filters the bucket start; widen to a bucket range filter if users ask for it -->
    <div class="stack">
      <LineChart series={data.series} grain={data.grain} {xTitle} {yTitle} {compact}
        onSelect={(x, label) => onMark({ kind: 'category', value: x, series: label || undefined })} />
      {#if data.series.length > 1}
        <ChartLegend kind="series" title={spec.encodings.group ?? 'Series'}
          items={data.series.map((line, index) => ({ label: line.label, color: `var(--chart-group-${(index % 6) + 1})` }))} />
      {/if}
    </div>
  {:else}
    <div class="state"><strong>This chart is not available yet</strong><span>Bar, line, histogram, box, and scatter are ready for this column mix.</span></div>
  {/if}
  {#if loading && data}<span class="refreshing" role="status">Refreshing chart…</span>{/if}
</div>

<style>
  .chart-view { position: relative; flex: 1; min-width: 0; min-height: 0; width: 100%; display: flex; }
  .stack { flex: 1; min-width: 0; min-height: 0; display: flex; flex-direction: column; }
  .state { margin: auto; display: flex; flex-direction: column; align-items: center; gap: 6px; padding: 24px; text-align: center; color: var(--muted); font-size: 12.5px; }
  .state strong { color: var(--ink); }
  .state.error strong { color: var(--error); }
  .refreshing { position: absolute; right: 8px; bottom: 6px; padding: 3px 7px; border-radius: var(--radius-md); background: color-mix(in srgb, var(--surface) 90%, transparent); color: var(--muted); font-size: 10.5px; }
</style>
