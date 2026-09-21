<script lang="ts">
  import ChartFrame from './ChartFrame.svelte';
  import { formatTick, niceTicks } from '../../lib/visualize';
  import type { AggregateCount, BarLayout, BarVisualizeRow, ChartHover } from '../../lib/types';

  type Props = {
    rows: BarVisualizeRow[];
    xTitle: string;
    yTitle: string;
    count: (value: AggregateCount) => string;
    compact: (value: number | string | null | undefined) => string;
    aggregated: boolean;
    series?: string[] | null;
    layout?: BarLayout;
    onSelect: (label: string | boolean | number, series?: string | boolean | number) => void;
  };
  let { rows, xTitle, yTitle, count, compact, aggregated, series = null, layout = 'grouped', onSelect }: Props = $props();

  let hover = $state<ChartHover | null>(null);
  let stacked = $derived(!!series && layout !== 'grouped');
  let normalized = $derived(!!series && layout === 'stacked100');
  let cells = $derived(rows.map((row) => (series && row.values ? row.values : [row.value]).map(Number)));
  let rowTotals = $derived(cells.map((row) => row.reduce((sum, value) => sum + value, 0)));
  let total = $derived(cells.flat().reduce((sum, value) => sum + value, 0));
  let values = $derived(
    normalized ? [0, 1]
      : stacked ? rowTotals
      : cells.flat()
  );
  let yScale = $derived.by(() => {
    const lo = Math.min(0, ...values);
    const hi = Math.max(0, ...values, 1);
    const ticks = niceTicks(lo, hi, 5);
    return { ticks, min: ticks[0] ?? lo, max: ticks[ticks.length - 1] ?? hi };
  });
  let ySpan = $derived(yScale.max - yScale.min || 1);
  let yTicks = $derived(yScale.ticks.map((value) => ({
    label: normalized ? `${Math.round(value * 100)}%` : formatTick(value),
    t: (value - yScale.min) / ySpan
  })));
  let xTicks = $derived(rows.map((row, index) => ({
    label: String(row.label),
    t: (index + 0.5) / rows.length
  })));
  const minBand = 36;

  function describe(row: BarVisualizeRow, index: number, seriesIndex: number): ChartHover {
    const label = series?.[seriesIndex];
    const raw = cells[index][seriesIndex];
    const title = label ? `${row.label} · ${label}` : String(row.label);
    if (aggregated || series) {
      return {
        title,
        lines: [`${yTitle}: ${compact(raw)}`],
        hint: label === 'Other' ? 'Folded from smaller series' : 'Click to filter this value'
      };
    }
    const share = total ? `${((raw * 100) / total).toFixed(1)}%` : '0%';
    return {
      title,
      lines: [`${count(raw)} rows`, `${share} of this chart`],
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
        {@const cellValues = cells[index]}
        {@const scale = normalized ? (rowTotals[index] || 1) : 1}
        {@const slots = cellValues.length}
        {@const bandX = plot.x + index * band + band * 0.12}
        {@const bandWidth = Math.max(1, band * 0.76)}
        {#each cellValues as raw, seriesIndex}
          {@const value = raw / scale}
          {@const below = stacked ? cellValues.slice(0, seriesIndex).reduce((sum, item) => sum + item / scale, 0) : 0}
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
            role="button"
            aria-disabled={!clickable}
            aria-label={`${row.label}${seriesLabel ? ` · ${seriesLabel}` : ''}: ${aggregated || series ? `${yTitle}: ${compact(raw)}` : `${count(raw)} rows`}. Filter to this value.`}
            onclick={() => { if (clickable) onSelect(row.label, seriesLabel); }}
            onkeydown={(event) => { if (clickable && (event.key === 'Enter' || event.key === ' ')) { event.preventDefault(); onSelect(row.label, seriesLabel); } }}
            onpointerenter={() => hover = describe(row, index, seriesIndex)}
            onfocus={() => hover = describe(row, index, seriesIndex)}
          />
        {/each}
      {/each}
    {/snippet}
  </ChartFrame>
{:else}
  <p class="empty">No values to chart.</p>
{/if}

<style>
  .bar { fill: var(--chart-bar-fill, var(--chart-mark)); stroke: var(--chart-mark-strong); stroke-width: .75px; cursor: pointer; }
  .bar:hover, .bar:focus-visible { fill: var(--chart-mark-strong); outline: none; }
  .bar.flat { cursor: default; }
  .bar.flat:hover { fill: var(--chart-bar-fill, var(--chart-mark)); }
  .empty { margin: auto; font-size: 12.5px; color: var(--muted); }
  @media (prefers-reduced-motion: no-preference) {
    .bar { transform-box: fill-box; transform-origin: center bottom; animation: grow 160ms cubic-bezier(0.16, 1, 0.3, 1) both; transition: x 220ms cubic-bezier(0.22, 1, 0.36, 1), y 220ms cubic-bezier(0.22, 1, 0.36, 1), width 220ms cubic-bezier(0.22, 1, 0.36, 1), height 220ms cubic-bezier(0.22, 1, 0.36, 1); }
    @keyframes grow { from { transform: scaleY(0.08); } to { transform: none; } }
  }
</style>
