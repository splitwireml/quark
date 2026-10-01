<script lang="ts">
  import ChartFrame from './ChartFrame.svelte';
  import { formatPlotTick, formatTick, niceTicks, toPlotNumber } from '../../lib/visualize';
  import type { ChartHover, LineSeries, TimeGrain } from '../../lib/types';

  type Props = {
    series: LineSeries[];
    grain: TimeGrain;
    xTitle: string;
    yTitle: string;
    compact: (value: number | string | null | undefined) => string;
    onSelect: (x: string, label: string) => void;
  };
  let { series, grain, xTitle, yTitle, compact, onSelect }: Props = $props();

  let hover = $state<ChartHover | null>(null);
  let sample = $derived(grain === 'hour' ? series[0]?.points[0]?.x : (series[0]?.points[0]?.x ?? '').slice(0, 10));
  let xs = $derived(series.flatMap((line) => line.points.map((point) => toPlotNumber(point.x))).filter(Number.isFinite));
  let ys = $derived(series.flatMap((line) => line.points.map((point) => Number(point.y))).filter(Number.isFinite));
  let domain = $derived.by(() => {
    const xRaw = niceTicks(Math.min(...xs, Infinity), Math.max(...xs, -Infinity), 6);
    const yRaw = niceTicks(Math.min(0, ...ys), Math.max(1, ...ys), 5);
    return {
      xMin: xRaw[0] ?? 0,
      xMax: xRaw[xRaw.length - 1] ?? 1,
      yMin: yRaw[0] ?? 0,
      yMax: yRaw[yRaw.length - 1] ?? 1,
      xRaw,
      yRaw
    };
  });
  let xSpan = $derived(domain.xMax - domain.xMin || 1);
  let ySpan = $derived(domain.yMax - domain.yMin || 1);
  let xTicks = $derived(domain.xRaw.map((value) => ({ label: formatPlotTick(value, sample), t: (value - domain.xMin) / xSpan })));
  let yTicks = $derived(domain.yRaw.map((value) => ({ label: formatTick(value), t: (value - domain.yMin) / ySpan })));

  function at(point: { x: string; y: unknown }, plot: { x: number; y: number; width: number; height: number }) {
    return {
      px: plot.x + ((toPlotNumber(point.x) - domain.xMin) / xSpan) * plot.width,
      py: plot.y + (1 - (Number(point.y) - domain.yMin) / ySpan) * plot.height
    };
  }

  function path(line: LineSeries, plot: { x: number; y: number; width: number; height: number }): string {
    return line.points
      .map((point, index) => {
        const { px, py } = at(point, plot);
        return `${index ? 'L' : 'M'}${px.toFixed(2)} ${py.toFixed(2)}`;
      })
      .join(' ');
  }

  function describe(line: LineSeries, index: number): ChartHover {
    const point = line.points[index];
    return {
      title: formatPlotTick(toPlotNumber(point.x), sample),
      lines: [line.label ? `${line.label} · ${yTitle}: ${compact(point.y)}` : `${yTitle}: ${compact(point.y)}`],
      hint: line.label === 'Other' ? 'Folded from smaller series' : 'Click to filter this point'
    };
  }
</script>

{#if ys.length}
  <ChartFrame {xTicks} {yTicks} {xTitle} {yTitle} {hover} label="Line chart" onDismiss={() => hover = null}>
    {#snippet children(plot)}
      {#each series as line, lineIndex (line.label)}
        <path class="line" d={path(line, plot)} style={`--chart-line-stroke:var(--chart-group-${(lineIndex % 6) + 1})`} />
        {#each line.points as point, index (String(point.x))}
          {@const spot = at(point, plot)}
          {@const clickable = line.label !== 'Other'}
          <circle
            cx={spot.px} cy={spot.py} r="3"
            class="dot"
            style={`--chart-line-stroke:var(--chart-group-${(lineIndex % 6) + 1})`}
          />
          <circle
            cx={spot.px} cy={spot.py} r="8"
            class="hit"
            tabindex={clickable ? 0 : -1}
            role="button"
            aria-disabled={!clickable}
            aria-label={`${formatPlotTick(toPlotNumber(point.x), sample)}${line.label ? ` · ${line.label}` : ''}: ${compact(point.y)}. Filter to this point.`}
            onclick={() => { if (clickable) onSelect(String(point.x), line.label); }}
            onkeydown={(event) => { if (clickable && (event.key === 'Enter' || event.key === ' ')) { event.preventDefault(); onSelect(String(point.x), line.label); } }}
            onpointerenter={() => hover = describe(line, index)}
            onfocus={() => hover = describe(line, index)}
          />
        {/each}
      {/each}
    {/snippet}
  </ChartFrame>
{:else}
  <p class="empty">No points to chart.</p>
{/if}

<style>
  .line { fill: none; stroke: var(--chart-line-stroke, var(--chart-mark-strong)); stroke-width: 1.75px; stroke-linejoin: round; stroke-linecap: round; }
  .dot { fill: var(--chart-line-stroke, var(--chart-mark-strong)); pointer-events: none; }
  .hit { fill: transparent; cursor: pointer; }
  .hit:hover, .hit:focus-visible { fill: color-mix(in srgb, var(--chart-mark-strong) 22%, transparent); outline: none; }
  .empty { margin: auto; font-size: 12.5px; color: var(--muted); }
  @media (prefers-reduced-motion: no-preference) {
    /* Morphs the series in place when the grain changes, where `d` is interpolable. */
    .line { transition: d 260ms cubic-bezier(0.22, 1, 0.36, 1); }
    .dot { transition: cx 260ms cubic-bezier(0.22, 1, 0.36, 1), cy 260ms cubic-bezier(0.22, 1, 0.36, 1); }
  }
</style>
