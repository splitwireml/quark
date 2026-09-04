<script lang="ts">
  import DistributionRow from '../molecules/DistributionRow.svelte';
  import HistogramChart from '../molecules/HistogramChart.svelte';
  import type { AggregateCount, ColumnStats, HistogramBin } from '../../lib/types';

  type Props = {
    loading: boolean;
    error: string;
    stats: ColumnStats | null;
    count: (value: AggregateCount) => string;
    compact: (value: number | string | null | undefined) => string;
    maxBin: number;
    binReadout: string;
    binLabel: (bin: HistogramBin) => string;
    onFocusBin: (bin: HistogramBin) => void;
    distributionMode: 'count' | 'percent';
    setDistributionMode: (mode: 'count' | 'percent') => void;
    cumulativeDistribution: boolean;
    toggleCumulative: () => void;
    distributionText: (value: AggregateCount, values: { count: AggregateCount }[], index: number, total: AggregateCount) => string;
  };

  let {
    loading, error, stats, count, compact, maxBin, binReadout, binLabel, onFocusBin,
    distributionMode, setDistributionMode, cumulativeDistribution, toggleCumulative, distributionText
  }: Props = $props();
</script>

{#if loading}
  <div class="state"><span class="spinner"></span>Computing statistics…</div>
{:else if error}
  <div class="state error"><strong>Statistics unavailable</strong><span>{error}</span></div>
{:else if stats}
  <dl class="summary"><div><dt>Completeness</dt><dd>{count(stats.non_null_count)} non-null · {count(stats.null_count)} null</dd></div></dl>
  {#if stats.kind === 'numeric'}
    <dl class="summary">
      <div><dt>Range</dt><dd>{compact(stats.min)} — {compact(stats.max)}</dd></div>
      <div><dt>Center</dt><dd>mean {compact(stats.mean)} · median {compact(stats.median)}</dd></div>
      <div><dt>Spread</dt><dd>σ {compact(stats.stddev)} · P25 {compact(stats.p25)} · P75 {compact(stats.p75)}</dd></div>
    </dl>
    <section class="histogram">
      <header><h3>Distribution</h3><span>{stats.histogram.length} bins</span></header>
      {#if stats.histogram.length}
        <HistogramChart bins={stats.histogram} {maxBin} readout={binReadout} {binLabel} {count} onfocusBin={onFocusBin} />
      {:else}
        <p class="muted">No values to chart.</p>
      {/if}
    </section>
  {:else if stats.kind === 'categorical'}
    <dl class="summary"><div><dt>Distinct values</dt><dd>{count(stats.distinct_count)}</dd></div></dl>
    <section class="values">
      <header>
        <h3>Top values</h3>
        <div class="controls" role="group" aria-label="Distribution display">
          <button aria-pressed={distributionMode === 'count'} onclick={() => setDistributionMode('count')}>Count</button>
          <button aria-pressed={distributionMode === 'percent'} onclick={() => setDistributionMode('percent')}>Percent</button>
          <button aria-pressed={cumulativeDistribution} onclick={toggleCumulative}>Cumulative</button>
        </div>
      </header>
      {#each stats.top_values as value, index (String(value.value))}
        <DistributionRow label={String(value.value)} value={distributionText(value.count, stats.top_values, index, stats.non_null_count)} />
      {:else}
        <p class="muted">No values to show.</p>
      {/each}
    </section>
  {:else}
    <dl class="summary">
      <div><dt>Range</dt><dd>{compact(stats.min)} — {compact(stats.max)}</dd></div>
      <div><dt>Distinct values</dt><dd>{count(stats.distinct_count)}</dd></div>
    </dl>
    <section class="values">
      <header>
        <h3>By year</h3>
        <div class="controls" role="group" aria-label="Distribution display">
          <button aria-pressed={distributionMode === 'count'} onclick={() => setDistributionMode('count')}>Count</button>
          <button aria-pressed={distributionMode === 'percent'} onclick={() => setDistributionMode('percent')}>Percent</button>
          <button aria-pressed={cumulativeDistribution} onclick={toggleCumulative}>Cumulative</button>
        </div>
      </header>
      {#each stats.year_counts as year, index (year.year)}
        <DistributionRow label={year.year} value={distributionText(year.count, stats.year_counts, index, stats.non_null_count)} />
      {:else}
        <p class="muted">No yearly values to show.</p>
      {/each}
    </section>
  {/if}
{:else}
  <div class="state">No statistics available.</div>
{/if}

<style>
  .state { display: flex; align-items: center; gap: 8px; font-size: 12.5px; color: var(--muted); padding: 8px 0; }
  .state.error { flex-direction: column; align-items: flex-start; color: var(--error); }
  .summary { margin: 0 0 14px; display: flex; flex-direction: column; gap: 6px; }
  .summary div { display: flex; justify-content: space-between; gap: 10px; font-size: 12px; }
  dt { color: var(--faint); }
  dd { margin: 0; color: var(--ink-2); font-family: var(--font-mono); font-size: 11.5px; text-align: right; }
  .histogram, .values { margin-top: 4px; }
  header { display: flex; align-items: center; justify-content: space-between; margin-bottom: 8px; }
  h3 { margin: 0; font-size: 12.5px; font-weight: 600; color: var(--ink); }
  header span { font-size: 11px; color: var(--faint); }
  .controls { display: flex; gap: 4px; }
  .controls button { height: 22px; padding: 0 7px; border-radius: var(--radius-sm); border: 1px solid var(--control-border); background: var(--surface); font-size: 10.5px; color: var(--muted); }
  .controls button[aria-pressed="true"] { border-color: var(--action); background: var(--action-tint); color: var(--action-dark); }
  .muted { font-size: 12px; color: var(--muted); }
</style>
