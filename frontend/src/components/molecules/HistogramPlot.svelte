<script lang="ts">
  import ChartFrame from './ChartFrame.svelte';
  import { formatPlotTick, formatTick, niceTicks, toPlotNumber } from '../../lib/visualize';
  import type { AggregateCount, ChartHover, HistogramBin } from '../../lib/types';

  type Props = {
    bins: HistogramBin[];
    xTitle: string;
    count: (value: AggregateCount) => string;
    binLabel: (bin: HistogramBin) => string;
    onSelect: (bin: HistogramBin, last: boolean) => void;
  };
  let { bins, xTitle, count, binLabel, onSelect }: Props = $props();

  let hover = $state<ChartHover | null>(null);
  let max = $derived(Math.max(1, ...bins.map((bin) => Number(bin.count))));
  let yScale = $derived.by(() => {
    const ticks = niceTicks(0, max, 5);
    return { ticks, max: ticks[ticks.length - 1] || max };
  });
  let domain = $derived.by(() => {
    const start = toPlotNumber(bins[0]?.lower ?? 0);
    const end = toPlotNumber(bins[bins.length - 1]?.upper ?? 1);
    const ticks = niceTicks(start, end, 6);
    const niceStart = ticks[0] ?? start;
    const niceEnd = ticks[ticks.length - 1] ?? end;
    return { start: niceStart, span: (niceEnd - niceStart) || 1, ticks };
  });
  let yTicks = $derived(yScale.ticks.map((value) => ({ label: formatTick(value), t: value / yScale.max })));
  let xTicks = $derived(domain.ticks.map((value) => ({
    label: formatPlotTick(value, bins[0]?.lower),
    t: (value - domain.start) / domain.span
  })));

  function describe(bin: HistogramBin): ChartHover {
    return {
      title: binLabel(bin),
      lines: [`${count(bin.count)} rows`],
      hint: 'Click to filter this range'
    };
  }
</script>

{#if bins.length}
  <ChartFrame {xTicks} {yTicks} {xTitle} yTitle="Rows" {hover} label="Histogram" onDismiss={() => hover = null}>
    {#snippet children(plot)}
      {#each bins as bin, index (String(bin.lower))}
        {@const start = (toPlotNumber(bin.lower) - domain.start) / domain.span}
        {@const end = (toPlotNumber(bin.upper) - domain.start) / domain.span}
        {@const x = plot.x + start * plot.width + 1}
        {@const w = Math.max(1, (end - start) * plot.width - 2)}
        {@const barHeight = Math.max(2, (Number(bin.count) / yScale.max) * plot.height)}
        {@const y = plot.y + plot.height - barHeight}
        {@const last = index === bins.length - 1}
        <rect
          {x} {y} width={w} height={barHeight} rx="1"
          class="bar"
          style={`--chart-bar-fill:var(--chart-series-${(index % 6) + 1}-fill)`}
          tabindex="0"
          role="button"
          aria-label={`${binLabel(bin)}: ${count(bin.count)} rows. Filter to this range.`}
          onclick={() => onSelect(bin, last)}
          onkeydown={(event) => { if (event.key === 'Enter' || event.key === ' ') { event.preventDefault(); onSelect(bin, last); } }}
          onpointerenter={() => hover = describe(bin)}
          onfocus={() => hover = describe(bin)}
        />
      {/each}
    {/snippet}
  </ChartFrame>
{:else}
  <p class="empty">No values to chart.</p>
{/if}

<style>
  .bar { fill: var(--chart-bar-fill, var(--chart-mark)); stroke: var(--chart-mark-strong); stroke-width: .75px; cursor: pointer; }
  .bar:hover, .bar:focus-visible { fill: var(--chart-mark-strong); outline: none; }
  .empty { margin: auto; font-size: 12.5px; color: var(--muted); }
  @media (prefers-reduced-motion: no-preference) {
    .bar { transform-box: fill-box; transform-origin: center bottom; animation: grow 160ms cubic-bezier(0.16, 1, 0.3, 1) both; }
    @keyframes grow { from { transform: scaleY(0.08); } to { transform: none; } }
  }
</style>
