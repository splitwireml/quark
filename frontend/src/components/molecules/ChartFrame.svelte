<script lang="ts">
  import type { Snippet } from 'svelte';
  import type { ChartHover } from '../../lib/types';

  export type PlotRect = { x: number; y: number; width: number; height: number };
  export type AxisTick = { label: string; t: number };

  type Props = {
    xTicks: AxisTick[];
    yTicks: AxisTick[];
    xTitle?: string;
    yTitle?: string;
    xAngle?: number;
    minPlotWidth?: number;
    hover: ChartHover | null;
    label: string;
    onDismiss?: () => void;
    children: Snippet<[PlotRect]>;
  };

  let {
    xTicks, yTicks, xTitle = '', yTitle = '', xAngle = 0, minPlotWidth = 0,
    hover, label, onDismiss, children
  }: Props = $props();

  let width = $state(640);
  let height = $state(320);
  let pointer = $state({ x: 24, y: 24 });
  let pad = $derived({
    top: 16,
    right: 16,
    bottom: xAngle ? 84 : 44,
    left: 56
  });
  let innerWidth = $derived(Math.max(width, pad.left + minPlotWidth + pad.right));
  let scrolling = $derived(innerWidth > width + 1);
  let plot = $derived({
    x: pad.left,
    y: pad.top,
    width: Math.max(1, innerWidth - pad.left - pad.right),
    height: Math.max(1, height - pad.top - pad.bottom)
  });
  let pane = $derived.by(() => {
    const left = pointer.x + 14 > width - 250 ? pointer.x - 226 : pointer.x + 14;
    const top = pointer.y + 14 > height - 120 ? pointer.y - 108 : pointer.y + 14;
    return { left: Math.max(8, left), top: Math.max(8, top) };
  });

  function observeSize(node: HTMLElement) {
    const ro = new ResizeObserver((entries) => {
      const rect = entries[0]?.contentRect;
      if (!rect) return;
      width = Math.max(1, rect.width);
      height = Math.max(1, rect.height);
    });
    ro.observe(node);
    return () => ro.disconnect();
  }

  function trackPointer(event: PointerEvent) {
    const rect = (event.currentTarget as HTMLElement).getBoundingClientRect();
    pointer = { x: event.clientX - rect.left, y: event.clientY - rect.top };
  }

  function xAt(t: number) { return plot.x + t * plot.width; }
  function yAt(t: number) { return plot.y + (1 - t) * plot.height; }
</script>

<div class="frame" {@attach observeSize} onpointermove={trackPointer} onpointerleave={onDismiss} role="presentation">
  <div class="scroller" class:scrolling>
    <svg width={innerWidth} {height} role="img" aria-label={label}>
      {#each yTicks as tick (tick.t)}
        <line class="grid" x1={plot.x} x2={plot.x + plot.width} y1={yAt(tick.t)} y2={yAt(tick.t)} />
      {/each}
      {#if !scrolling}
        {#each yTicks as tick (tick.t)}
          <text class="tick" text-anchor="end" x={plot.x - 8} y={yAt(tick.t) + 3}>{tick.label}</text>
        {/each}
        <line class="axis" x1={plot.x} y1={plot.y} x2={plot.x} y2={plot.y + plot.height} />
        {#if yTitle}
          <text class="title" text-anchor="middle" transform="translate(14, {plot.y + plot.height / 2}) rotate(-90)">{yTitle}</text>
        {/if}
      {/if}
      {#each xTicks as tick (tick.t + tick.label)}
        {#if xAngle}
          <text
            class="tick"
            text-anchor="end"
            transform="translate({xAt(tick.t)}, {plot.y + plot.height + 10}) rotate({xAngle})"
          >{tick.label}</text>
        {:else}
          <text class="tick" text-anchor="middle" x={xAt(tick.t)} y={plot.y + plot.height + 16}>{tick.label}</text>
        {/if}
      {/each}
      <line class="axis" x1={plot.x} y1={plot.y + plot.height} x2={plot.x + plot.width} y2={plot.y + plot.height} />
      {@render children(plot)}
    </svg>
  </div>
  {#if scrolling}
    <div class="y-lock" style:width={`${pad.left}px`}>
      <svg width={pad.left} {height} aria-hidden="true">
        {#each yTicks as tick (tick.t)}
          <text class="tick" text-anchor="end" x={pad.left - 8} y={yAt(tick.t) + 3}>{tick.label}</text>
        {/each}
        <line class="axis" x1={pad.left - 0.5} y1={plot.y} x2={pad.left - 0.5} y2={plot.y + plot.height} />
        {#if yTitle}
          <text class="title" text-anchor="middle" transform="translate(14, {plot.y + plot.height / 2}) rotate(-90)">{yTitle}</text>
        {/if}
      </svg>
    </div>
  {/if}
  {#if xTitle}
    <p class="x-title">{xTitle}</p>
  {/if}
  {#if hover}
    <aside class="hover" style:left={`${pane.left}px`} style:top={`${pane.top}px`} aria-live="polite">
      <strong>{hover.title}</strong>
      {#each hover.lines as line (line)}
        <p>{line}</p>
      {/each}
      {#if hover.hint}<small>{hover.hint}</small>{/if}
    </aside>
  {/if}
</div>

<style>
  .frame { position: relative; flex: 1; min-height: 0; width: 100%; }
  .scroller { width: 100%; height: 100%; overflow-x: hidden; overflow-y: hidden; }
  .scroller.scrolling { overflow-x: auto; }
  svg { display: block; }
  .grid { stroke: var(--line); }
  .axis { stroke: var(--line-strong); }
  .tick { font: 10.5px var(--font-mono); fill: var(--muted); }
  .title { font: 10.5px var(--font-ui); fill: var(--faint); }
  .y-lock {
    position: absolute;
    left: 0;
    top: 0;
    bottom: 0;
    background: var(--surface);
    pointer-events: none;
  }
  .x-title {
    position: absolute;
    left: 56px;
    right: 16px;
    bottom: 4px;
    margin: 0;
    text-align: center;
    font: 10.5px var(--font-ui);
    color: var(--faint);
    pointer-events: none;
  }
  .hover {
    position: absolute;
    z-index: 4;
    width: max-content;
    max-width: 240px;
    padding: 8px 10px;
    border: 1px solid var(--line-strong);
    border-radius: var(--radius-xl);
    background: var(--surface);
    box-shadow: var(--shadow-panel);
    pointer-events: none;
  }
  .hover strong { display: block; font-size: 12.5px; font-weight: 600; color: var(--ink); }
  .hover p { margin: 4px 0 0; font: 11.5px var(--font-mono); color: var(--muted); }
  .hover small { display: block; margin-top: 6px; font-size: 10.5px; color: var(--faint); }
</style>
