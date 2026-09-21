<script lang="ts">
  import type { AggregateCount, BarVisualizeRow, ChartHover } from '../../lib/types';

  type Props = {
    rows: BarVisualizeRow[];
    title: string;
    count: (value: AggregateCount) => string;
    compact: (value: number | string | null | undefined) => string;
    aggregated: boolean;
    onSelect: (label: string | boolean | number) => void;
  };
  let { rows, title, count, compact, aggregated, onSelect }: Props = $props();

  let hover = $state<ChartHover | null>(null);
  let total = $derived(rows.reduce((sum, row) => sum + Number(row.value), 0));
  let slices = $derived.by(() => {
    let angle = -Math.PI / 2;
    return rows.map((row, index) => {
      const share = total ? Number(row.value) / total : 0;
      const start = angle;
      angle += share * Math.PI * 2;
      return { row, index, share, start, end: angle };
    });
  });

  function arc(start: number, end: number): string {
    const r = 74;
    // A single slice covering the whole circle has start === end, which an arc cannot draw.
    if (end - start >= Math.PI * 2 - 1e-9) return `M100 26 A${r} ${r} 0 1 1 99.99 26 Z`;
    const x0 = 100 + r * Math.cos(start);
    const y0 = 100 + r * Math.sin(start);
    const x1 = 100 + r * Math.cos(end);
    const y1 = 100 + r * Math.sin(end);
    const large = end - start > Math.PI ? 1 : 0;
    return `M100 100 L${x0.toFixed(2)} ${y0.toFixed(2)} A${r} ${r} 0 ${large} 1 ${x1.toFixed(2)} ${y1.toFixed(2)} Z`;
  }

  function describe(row: BarVisualizeRow, share: number): ChartHover {
    return {
      title: String(row.label),
      lines: [
        aggregated ? `${title}: ${compact(row.value)}` : `${count(row.value)} rows`,
        `${(share * 100).toFixed(1)}% of this chart`
      ],
      hint: 'Click to filter this value'
    };
  }
</script>

{#if rows.length}
  <div class="pie-wrap">
    <svg viewBox="0 0 200 200" role="img" aria-label={`Pie chart of ${title}`}>
      {#each slices as slice (String(slice.row.label))}
        <path
          d={arc(slice.start, slice.end)}
          class="slice"
          style={`--chart-slice-fill:var(--chart-group-${(slice.index % 6) + 1})`}
          tabindex="0"
          role="button"
          aria-label={`${slice.row.label}: ${(slice.share * 100).toFixed(1)}%. Filter to this value.`}
          onclick={() => onSelect(slice.row.label)}
          onkeydown={(event) => { if (event.key === 'Enter' || event.key === ' ') { event.preventDefault(); onSelect(slice.row.label); } }}
          onpointerenter={() => hover = describe(slice.row, slice.share)}
          onpointerleave={() => hover = null}
          onfocus={() => hover = describe(slice.row, slice.share)}
          onblur={() => hover = null}
        />
      {/each}
    </svg>
    {#if hover}
      <div class="hover" role="status">
        <strong>{hover.title}</strong>
        {#each hover.lines as line (line)}<span>{line}</span>{/each}
      </div>
    {/if}
  </div>
{:else}
  <p class="empty">No values to chart.</p>
{/if}

<style>
  .pie-wrap { position: relative; flex: 1; min-width: 0; min-height: 0; display: flex; align-items: center; justify-content: center; padding: 8px; }
  svg { width: min(100%, 320px); height: auto; }
  .slice { fill: var(--chart-slice-fill, var(--chart-mark)); stroke: var(--surface); stroke-width: 1.5px; cursor: pointer; transform-origin: 100px 100px; }
  .slice:hover, .slice:focus-visible { outline: none; stroke: var(--chart-mark-strong); stroke-width: 2px; }
  .hover { position: absolute; left: 8px; top: 8px; display: flex; flex-direction: column; gap: 2px; padding: 6px 8px; border: 1px solid var(--line); border-radius: var(--radius-md); background: var(--surface); font-size: 11px; color: var(--muted); }
  .hover strong { color: var(--ink); font-size: 12px; }
  .empty { margin: auto; font-size: 12.5px; color: var(--muted); }
  @media (prefers-reduced-motion: no-preference) {
    .slice { transition: transform 160ms cubic-bezier(0.22, 1, 0.36, 1); }
    .slice:hover, .slice:focus-visible { transform: scale(1.03); }
  }
</style>
