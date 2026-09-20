<script lang="ts">
  import ChartFrame from './ChartFrame.svelte';
  import { formatTick, niceTicks } from '../../lib/visualize';
  import type { AggregateCount, BarVisualizeRow, ChartHover } from '../../lib/types';

  type Props = {
    rows: BarVisualizeRow[];
    xTitle: string;
    yTitle: string;
    count: (value: AggregateCount) => string;
    compact: (value: number | string | null | undefined) => string;
    aggregated: boolean;
    onSelect: (label: string | boolean | number) => void;
  };
  let { rows, xTitle, yTitle, count, compact, aggregated, onSelect }: Props = $props();

  let hover = $state<ChartHover | null>(null);
  let values = $derived(rows.map((row) => Number(row.value)));
  let total = $derived(values.reduce((sum, value) => sum + value, 0));
  let yScale = $derived.by(() => {
    const lo = Math.min(0, ...values);
    const hi = Math.max(0, ...values, 1);
    const ticks = niceTicks(lo, hi, 5);
    return { ticks, min: ticks[0] ?? lo, max: ticks[ticks.length - 1] ?? hi };
  });
  let ySpan = $derived(yScale.max - yScale.min || 1);
  let yTicks = $derived(yScale.ticks.map((value) => ({ label: formatTick(value), t: (value - yScale.min) / ySpan })));
  let xTicks = $derived(rows.map((row, index) => ({
    label: String(row.label),
    t: (index + 0.5) / rows.length
  })));
  const minBand = 36;

  function describe(row: BarVisualizeRow): ChartHover {
    if (aggregated) {
      return {
        title: String(row.label),
        lines: [compact(row.value), row.n != null ? `${count(row.n)} rows` : ''].filter(Boolean),
        hint: 'Click to filter this value'
      };
    }
    const share = total ? `${((Number(row.value) * 100) / total).toFixed(1)}%` : '0%';
    return {
      title: String(row.label),
      lines: [`${count(row.value)} rows`, `${share} of this chart`],
      hint: 'Click to filter this value'
    };
  }

  function yAt(plotY: number, plotHeight: number, value: number) {
    return plotY + (1 - (value - yScale.min) / ySpan) * plotHeight;
  }
</script>

{#if rows.length}
  <ChartFrame {xTicks} {yTicks} {xTitle} {yTitle} {hover} xAngle={-40} minPlotWidth={rows.length * minBand} label="Bar chart" onDismiss={() => hover = null}>
    {#snippet children(plot)}
      {#each rows as row, index (String(row.label))}
        {@const band = plot.width / rows.length}
        {@const top = yAt(plot.y, plot.height, Number(row.value))}
        {@const base = yAt(plot.y, plot.height, 0)}
        {@const y = Math.min(top, base)}
        {@const barHeight = Math.max(2, Math.abs(base - top))}
        {@const x = plot.x + index * band + band * 0.12}
        {@const w = Math.max(1, band * 0.76)}
        <rect
          {x} {y} width={w} height={barHeight} rx="1"
          class="bar"
          tabindex="0"
          role="button"
          aria-label={`${row.label}: ${aggregated ? compact(row.value) : `${count(row.value)} rows`}. Filter to this value.`}
          onclick={() => onSelect(row.label)}
          onkeydown={(event) => { if (event.key === 'Enter' || event.key === ' ') { event.preventDefault(); onSelect(row.label); } }}
          onpointerenter={() => hover = describe(row)}
          onfocus={() => hover = describe(row)}
        />
      {/each}
    {/snippet}
  </ChartFrame>
{:else}
  <p class="empty">No values to chart.</p>
{/if}

<style>
  .bar { fill: var(--chart-mark); cursor: pointer; }
  .bar:hover, .bar:focus-visible { fill: var(--chart-mark-strong); outline: none; }
  .empty { margin: auto; font-size: 12.5px; color: var(--muted); }
  @media (prefers-reduced-motion: no-preference) {
    .bar { transform-box: fill-box; transform-origin: center bottom; animation: grow 160ms cubic-bezier(0.16, 1, 0.3, 1) both; }
    @keyframes grow { from { transform: scaleY(0.08); } to { transform: none; } }
  }
</style>
