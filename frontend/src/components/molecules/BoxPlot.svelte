<script lang="ts">
  import ChartFrame from './ChartFrame.svelte';
  import Checkbox from '../atoms/Checkbox.svelte';
  import { formatTick, niceTicks, toPlotNumber } from '../../lib/visualize';
  import type { BoxGroup, ChartHover, NumericValue } from '../../lib/types';

  type Props = {
    groups: BoxGroup[];
    xTitle: string;
    yTitle: string;
    compact: (value: number | string | null | undefined) => string;
    onSelect: (group: BoxGroup) => void;
  };
  let { groups, xTitle, yTitle, compact, onSelect }: Props = $props();

  let hover = $state<ChartHover | null>(null);
  let showOutliers = $state(false);
  let hasOutliers = $derived(groups.some((group) => group.outliers.length > 0));
  let domain = $derived.by(() => {
    const values = groups.flatMap((group) => [group.whisker_low, group.whisker_high, ...(showOutliers ? group.outliers : [])].map(toPlotNumber)).filter(Number.isFinite);
    const min = Math.min(...values);
    const max = Math.max(...values);
    const ticks = niceTicks(min, max, 6);
    return { min: ticks[0] ?? min, max: ticks[ticks.length - 1] ?? max, ticks };
  });
  let span = $derived(domain.max - domain.min || 1);
  let yTicks = $derived(domain.ticks.map((value) => ({
    label: formatTick(value),
    t: (value - domain.min) / span
  })));
  let xTicks = $derived(groups.map((group, index) => ({
    label: String(group.label === 'all' ? '' : group.label),
    t: (index + 0.5) / groups.length
  })));

  function y(plotY: number, plotHeight: number, value: NumericValue) {
    return plotY + (1 - (toPlotNumber(value) - domain.min) / span) * plotHeight;
  }

  function describe(group: BoxGroup): ChartHover {
    const outliers = group.outliers.length ? `${group.outliers.length} outliers` : 'No outliers';
    return {
      title: group.label === 'all' ? yTitle : String(group.label),
      lines: [
        `Median ${compact(group.median)}`,
        `Q1 ${compact(group.p25)} · Q3 ${compact(group.p75)}`,
        `Whiskers ${compact(group.whisker_low)} – ${compact(group.whisker_high)}`,
        outliers
      ],
      hint: 'Click to filter this range'
    };
  }
</script>

{#if groups.length}
  <div class="box-plot">
  {#if hasOutliers}
    <div class="controls">
      <Checkbox checked={showOutliers} label="Show outliers" onchange={(checked) => { showOutliers = checked; hover = null; }} />
      <span>{showOutliers ? 'Scale includes outliers' : 'Scale fits whiskers · outliers hidden'}</span>
    </div>
  {/if}
  <ChartFrame {xTicks} {yTicks} {xTitle} {yTitle} {hover} xAngle={groups.length > 1 ? -40 : 0} minPlotWidth={groups.length > 1 ? groups.length * 96 : 0} label="Box plot" onDismiss={() => hover = null}>
    {#snippet children(plot)}
      {#each groups as group, index (String(group.label))}
        {@const band = plot.width / groups.length}
        {@const cx = plot.x + (index + 0.5) * band}
        {@const boxW = Math.min(48, band * 0.45)}
        {@const top = y(plot.y, plot.height, group.p75)}
        {@const mid = y(plot.y, plot.height, group.median)}
        {@const bottom = y(plot.y, plot.height, group.p25)}
        {@const hi = y(plot.y, plot.height, group.whisker_high)}
        {@const lo = y(plot.y, plot.height, group.whisker_low)}
        <g
          class="box"
          style={`--chart-box-fill:var(--chart-series-${(index % 6) + 1}-fill)`}
          tabindex="0"
          role="button"
          aria-label={`${describe(group).title}: median ${compact(group.median)}. Filter to this range.`}
          onclick={() => onSelect(group)}
          onkeydown={(event) => { if (event.key === 'Enter' || event.key === ' ') { event.preventDefault(); onSelect(group); } }}
          onpointerenter={() => hover = describe(group)}
          onfocus={() => hover = describe(group)}
        >
          <line x1={cx} x2={cx} y1={hi} y2={top} />
          <line x1={cx} x2={cx} y1={bottom} y2={lo} />
          <line x1={cx - boxW / 3} x2={cx + boxW / 3} y1={hi} y2={hi} />
          <line x1={cx - boxW / 3} x2={cx + boxW / 3} y1={lo} y2={lo} />
          <rect x={cx - boxW / 2} y={Math.min(top, bottom)} width={boxW} height={Math.max(2, Math.abs(bottom - top))} rx="1" />
          <line class="median" x1={cx - boxW / 2} x2={cx + boxW / 2} y1={mid} y2={mid} />
          {#if showOutliers}
            {#each group.outliers as outlier, outlierIndex (`${group.label}-${outlierIndex}`)}
              <circle cx={cx} cy={y(plot.y, plot.height, outlier)} r="2.5" />
            {/each}
          {/if}
        </g>
      {/each}
    {/snippet}
  </ChartFrame>
  </div>
{:else}
  <p class="empty">No values to chart.</p>
{/if}

<style>
  .box-plot { flex: 1; min-width: 0; min-height: 0; display: flex; flex-direction: column; }
  .controls { display: flex; align-items: center; flex-wrap: wrap; gap: 8px 16px; min-height: 28px; padding: 0 16px 4px 56px; }
  .controls span { font-size: 11.5px; color: var(--muted); }
  .box { cursor: pointer; stroke: var(--chart-mark-strong); fill: var(--chart-box-fill, var(--chart-mark-fill)); }
  .box:hover, .box:focus-visible { fill: var(--chart-mark); outline: none; }
  .box :global(line) { stroke: var(--chart-mark-strong); }
  .median { stroke-width: 2; }
  .box :global(circle) { fill: var(--chart-mark-strong); stroke: none; }
  .empty { margin: auto; font-size: 12.5px; color: var(--muted); }
</style>
