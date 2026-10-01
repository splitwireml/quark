<script lang="ts">
  import ChartFrame, { type PlotRect } from './ChartFrame.svelte';
  import { formatTick, fromPlotNumber, niceTicks, toPlotNumber } from '../../lib/visualize';
  import type { ChartHover, NumericValue, ScatterPoint } from '../../lib/types';

  type Props = {
    points: ScatterPoint[];
    xTitle: string;
    yTitle: string;
    compact: (value: number | string | null | undefined) => string;
    theme?: string;
    colorKind?: 'categorical' | 'numeric';
    colorDomain?: [NumericValue, NumericValue];
    colorLabels?: string[];
    shapeLabels?: string[];
    sizeDomain?: [NumericValue, NumericValue];
    onSelectRegion: (region: { xMin: NumericValue; xMax: NumericValue; yMin: NumericValue; yMax: NumericValue }) => void;
  };
  let {
    points, xTitle, yTitle, compact, theme = 'primary',
    colorKind, colorDomain, colorLabels, shapeLabels, sizeDomain,
    onSelectRegion
  }: Props = $props();

  type Brush = { x0: number; y0: number; x1: number; y1: number; ready: boolean };

  let hover = $state<ChartHover | null>(null);
  let brush = $state<Brush | null>(null);
  let view = $state<{ xMin: number; xMax: number; yMin: number; yMax: number } | null>(null);
  let plotRef = $state<PlotRect | null>(null);
  let canvasEl = $state<HTMLCanvasElement | null>(null);

  let domain = $derived.by(() => {
    const xs = points.map((point) => toPlotNumber(point.x)).filter(Number.isFinite);
    const ys = points.map((point) => toPlotNumber(point.y)).filter(Number.isFinite);
    const fallback = { xMin: 0, xMax: 1, yMin: 0, yMax: 1 };
    const xRange = view ?? (xs.length && ys.length
      ? { xMin: Math.min(...xs), xMax: Math.max(...xs), yMin: Math.min(...ys), yMax: Math.max(...ys) }
      : fallback);
    const xTicks = niceTicks(xRange.xMin, xRange.xMax, 6);
    const yTicks = niceTicks(xRange.yMin, xRange.yMax, 6);
    return {
      xMin: xTicks[0] ?? xRange.xMin,
      xMax: xTicks[xTicks.length - 1] ?? xRange.xMax,
      yMin: yTicks[0] ?? xRange.yMin,
      yMax: yTicks[yTicks.length - 1] ?? xRange.yMax,
      xTicks,
      yTicks
    };
  });
  let xSpan = $derived(domain.xMax - domain.xMin || 1);
  let ySpan = $derived(domain.yMax - domain.yMin || 1);
  let xTicks = $derived(domain.xTicks.map((value) => ({ label: formatTick(value), t: (value - domain.xMin) / xSpan })));
  let yTicks = $derived(domain.yTicks.map((value) => ({ label: formatTick(value), t: (value - domain.yMin) / ySpan })));
  let categoryIndex = $derived.by(() => {
    const labels = colorLabels
      ?? [...new Set(points.filter((point) => point.color != null).map((point) => String(point.color)))].sort();
    return new Map(labels.map((label, index) => [label, index]));
  });
  let shapeIndex = $derived(new Map((shapeLabels ?? []).map((label, index) => [label, index])));

  function hexToRgb(hex: string): [number, number, number] {
    const value = hex.trim().replace('#', '');
    const full = value.length === 3 ? value.split('').map((char) => char + char).join('') : value;
    const number = Number.parseInt(full, 16);
    return Number.isFinite(number) ? [(number >> 16) & 255, (number >> 8) & 255, number & 255] : [0, 0, 0];
  }

  function mix(from: string, to: string, t: number): string {
    const a = hexToRgb(from);
    const b = hexToRgb(to);
    const clamped = Math.max(0, Math.min(1, t));
    const channel = (index: number) => Math.round(a[index] + (b[index] - a[index]) * clamped);
    return `rgb(${channel(0)}, ${channel(1)}, ${channel(2)})`;
  }

  const SHAPES = ['circle', 'square', 'triangle', 'diamond', 'cross', 'triangle-down'] as const;

  function drawShape(ctx: CanvasRenderingContext2D, shape: string, x: number, y: number, r: number) {
    ctx.beginPath();
    if (shape === 'square') ctx.rect(x - r, y - r, r * 2, r * 2);
    else if (shape === 'triangle') { ctx.moveTo(x, y - r); ctx.lineTo(x + r, y + r); ctx.lineTo(x - r, y + r); ctx.closePath(); }
    else if (shape === 'triangle-down') { ctx.moveTo(x, y + r); ctx.lineTo(x + r, y - r); ctx.lineTo(x - r, y - r); ctx.closePath(); }
    else if (shape === 'diamond') { ctx.moveTo(x, y - r); ctx.lineTo(x + r, y); ctx.lineTo(x, y + r); ctx.lineTo(x - r, y); ctx.closePath(); }
    else if (shape === 'cross') { ctx.moveTo(x - r, y - r); ctx.lineTo(x + r, y + r); ctx.moveTo(x + r, y - r); ctx.lineTo(x - r, y + r); }
    else ctx.arc(x, y, r, 0, Math.PI * 2);
  }
  let box = $derived(brush ? {
    x: Math.min(brush.x0, brush.x1),
    y: Math.min(brush.y0, brush.y1),
    width: Math.abs(brush.x1 - brush.x0),
    height: Math.abs(brush.y1 - brush.y0)
  } : null);

  function syncPlot(plot: PlotRect) {
    return () => {
      const prev = plotRef;
      if (!prev || prev.x !== plot.x || prev.y !== plot.y || prev.width !== plot.width || prev.height !== plot.height) {
        plotRef = plot;
      }
    };
  }

  $effect(() => {
    const canvas = canvasEl;
    const plot = plotRef;
    void theme;
    if (!canvas || !plot || plot.width < 1 || plot.height < 1) return;
    const dpr = window.devicePixelRatio || 1;
    canvas.width = Math.floor(plot.width * dpr);
    canvas.height = Math.floor(plot.height * dpr);
    const ctx = canvas.getContext('2d');
    if (!ctx) return;
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    ctx.clearRect(0, 0, plot.width, plot.height);
    const styles = getComputedStyle(canvas);
    const mark = styles.getPropertyValue('--chart-mark').trim() || '#C9DBFF';
    const strong = styles.getPropertyValue('--chart-mark-strong').trim() || '#1155F5';
    const mode = document.documentElement.dataset.chartMode;
    const categoryPalette = mode === 'multicolor' || mode === 'monotone';
    const pointFills = Array.from({ length: 6 }, (_, index) => styles.getPropertyValue(`--chart-group-${index + 1}`).trim() || mark);
    const flatPalette = !categoryPalette && !colorLabels;
    const sizeLow = sizeDomain ? Number(sizeDomain[0]) : 0;
    const sizeHigh = sizeDomain ? Number(sizeDomain[1]) : 1;
    const sizeSpan = sizeHigh - sizeLow || 1;
    const colorLow = colorDomain ? Number(colorDomain[0]) : 0;
    const colorHigh = colorDomain ? Number(colorDomain[1]) : 1;
    const colorSpan = colorHigh - colorLow || 1;
    const R_MIN = 1.6;
    const R_MAX = 7;
    void shapeIndex;
    ctx.lineWidth = 0.8;
    for (const point of points) {
      const x = ((toPlotNumber(point.x) - domain.xMin) / xSpan) * plot.width;
      const y = (1 - (toPlotNumber(point.y) - domain.yMin) / ySpan) * plot.height;
      if (!Number.isFinite(x) || !Number.isFinite(y)) continue;
      // sqrt so the drawn AREA tracks the value; scaling the radius would square it.
      const radius = sizeDomain && point.size != null
        ? R_MIN + (R_MAX - R_MIN) * Math.sqrt(Math.max(0, Math.min(1, (Number(point.size) - sizeLow) / sizeSpan)))
        : 2.6;
      if (colorKind === 'numeric' && point.color != null) {
        ctx.fillStyle = mix(mark, strong, (Number(point.color) - colorLow) / colorSpan);
      } else {
        const colorIndex = point.color == null ? -1 : categoryIndex.get(String(point.color)) ?? -1;
        ctx.fillStyle = colorIndex >= 0 && !flatPalette ? pointFills[colorIndex % pointFills.length] || mark : mark;
      }
      ctx.strokeStyle = strong;
      const shapeKey = point.shape == null ? 'circle' : SHAPES[shapeIndex.get(String(point.shape)) ?? 0];
      drawShape(ctx, shapeKey, x, y, radius);
      if (shapeKey === 'cross') ctx.stroke();
      else { ctx.fill(); ctx.stroke(); }
    }
  });

  function describe(point: ScatterPoint): ChartHover {
    return {
      title: `${xTitle} × ${yTitle}`,
      lines: [
        point.color == null ? '' : `Color ${String(point.color)}`,
        point.size == null ? '' : `Size ${compact(point.size)}`,
        point.shape == null ? '' : `Shape ${String(point.shape)}`,
        `${xTitle} ${compact(point.x)}`,
        `${yTitle} ${compact(point.y)}`
      ].filter(Boolean),
      hint: 'Drag to select a region'
    };
  }

  function localPoint(event: PointerEvent): { x: number; y: number } | null {
    const svg = (event.currentTarget as Element).closest('svg') as SVGSVGElement | null;
    const ctm = svg?.getScreenCTM();
    if (!svg || !ctm) return null;
    const point = svg.createSVGPoint();
    point.x = event.clientX;
    point.y = event.clientY;
    const local = point.matrixTransform(ctm.inverse());
    return { x: local.x, y: local.y };
  }

  function clamp(point: { x: number; y: number }, plot: PlotRect) {
    return {
      x: Math.min(plot.x + plot.width, Math.max(plot.x, point.x)),
      y: Math.min(plot.y + plot.height, Math.max(plot.y, point.y))
    };
  }

  function hoverNearest(local: { x: number; y: number }, plot: PlotRect) {
    let nearest: ScatterPoint | null = null;
    let best = 10;
    for (const candidate of points) {
      const cx = plot.x + ((toPlotNumber(candidate.x) - domain.xMin) / xSpan) * plot.width;
      const cy = plot.y + (1 - (toPlotNumber(candidate.y) - domain.yMin) / ySpan) * plot.height;
      const distance = Math.hypot(local.x - cx, local.y - cy);
      if (distance < best) {
        best = distance;
        nearest = candidate;
      }
    }
    hover = nearest ? describe(nearest) : null;
  }

  function onPointerDown(event: PointerEvent, plot: PlotRect) {
    if (event.button !== 0) return;
    const local = localPoint(event);
    if (!local) return;
    const start = clamp(local, plot);
    brush = { x0: start.x, y0: start.y, x1: start.x, y1: start.y, ready: false };
    hover = null;
    (event.currentTarget as SVGElement).setPointerCapture(event.pointerId);
  }

  function onPointerMove(event: PointerEvent, plot: PlotRect) {
    const local = localPoint(event);
    if (!local) return;
    if (brush && !brush.ready) {
      const next = clamp(local, plot);
      brush = { ...brush, x1: next.x, y1: next.y };
      return;
    }
    if (!brush?.ready) hoverNearest(local, plot);
  }

  function onPointerUp(event: PointerEvent) {
    if (!brush || brush.ready) return;
    const distance = Math.hypot(brush.x1 - brush.x0, brush.y1 - brush.y0);
    brush = distance < 6 ? null : { ...brush, ready: true };
    (event.currentTarget as SVGElement).releasePointerCapture(event.pointerId);
  }

  function regionFromBrush(plot: PlotRect) {
    if (!brush) return null;
    const xMinPx = Math.min(brush.x0, brush.x1);
    const xMaxPx = Math.max(brush.x0, brush.x1);
    const yMinPx = Math.min(brush.y0, brush.y1);
    const yMaxPx = Math.max(brush.y0, brush.y1);
    const xMin = domain.xMin + ((xMinPx - plot.x) / plot.width) * xSpan;
    const xMax = domain.xMin + ((xMaxPx - plot.x) / plot.width) * xSpan;
    const yMax = domain.yMin + (1 - (yMinPx - plot.y) / plot.height) * ySpan;
    const yMin = domain.yMin + (1 - (yMaxPx - plot.y) / plot.height) * ySpan;
    const xSample = points[0]?.x ?? 0;
    const ySample = points[0]?.y ?? 0;
    return {
      xMin: fromPlotNumber(Math.min(xMin, xMax), xSample),
      xMax: fromPlotNumber(Math.max(xMin, xMax), xSample),
      yMin: fromPlotNumber(Math.min(yMin, yMax), ySample),
      yMax: fromPlotNumber(Math.max(yMin, yMax), ySample)
    };
  }

  function numericRegion(plot: PlotRect) {
    if (!brush) return null;
    const xMinPx = Math.min(brush.x0, brush.x1);
    const xMaxPx = Math.max(brush.x0, brush.x1);
    const yMinPx = Math.min(brush.y0, brush.y1);
    const yMaxPx = Math.max(brush.y0, brush.y1);
    return {
      xMin: domain.xMin + ((xMinPx - plot.x) / plot.width) * xSpan,
      xMax: domain.xMin + ((xMaxPx - plot.x) / plot.width) * xSpan,
      yMin: domain.yMin + (1 - (yMaxPx - plot.y) / plot.height) * ySpan,
      yMax: domain.yMin + (1 - (yMinPx - plot.y) / plot.height) * ySpan
    };
  }

  function filterRegion() {
    if (!plotRef) return;
    const region = regionFromBrush(plotRef);
    if (!region) return;
    brush = null;
    onSelectRegion(region);
  }

  function zoomRegion() {
    if (!plotRef) return;
    const region = numericRegion(plotRef);
    if (!region) return;
    view = region;
    brush = null;
  }

  function resetView() {
    view = null;
    brush = null;
    hover = null;
  }

  function keyboardSelect(event: KeyboardEvent, plot: PlotRect) {
    if (event.key === 'Escape') { brush = null; return; }
    if (event.key === 'Enter' || event.key === ' ') {
      event.preventDefault();
      if (brush?.ready) filterRegion();
      else brush = {
        x0: plot.x + plot.width * 0.25, y0: plot.y + plot.height * 0.25,
        x1: plot.x + plot.width * 0.75, y1: plot.y + plot.height * 0.75, ready: true
      };
      return;
    }
    if (!['ArrowLeft', 'ArrowRight', 'ArrowUp', 'ArrowDown'].includes(event.key)) return;
    event.preventDefault();
    const current = brush?.ready ? brush : {
      x0: plot.x + plot.width * 0.25, y0: plot.y + plot.height * 0.25,
      x1: plot.x + plot.width * 0.75, y1: plot.y + plot.height * 0.75, ready: true
    };
    const dx = event.key === 'ArrowLeft' ? -plot.width * 0.05 : event.key === 'ArrowRight' ? plot.width * 0.05 : 0;
    const dy = event.key === 'ArrowUp' ? -plot.height * 0.05 : event.key === 'ArrowDown' ? plot.height * 0.05 : 0;
    if (event.shiftKey) {
      const end = clamp({ x: current.x1 + dx, y: current.y1 + dy }, plot);
      brush = { ...current, x1: end.x, y1: end.y };
    } else {
      const width = current.x1 - current.x0;
      const height = current.y1 - current.y0;
      const start = clamp({ x: current.x0 + dx, y: current.y0 + dy }, plot);
      const x0 = Math.min(plot.x + plot.width - Math.abs(width), Math.max(plot.x, start.x));
      const y0 = Math.min(plot.y + plot.height - Math.abs(height), Math.max(plot.y, start.y));
      brush = { ...current, x0, y0, x1: x0 + width, y1: y0 + height };
    }
  }
</script>

<svelte:window onkeydown={(event) => { if (event.key === 'Escape') brush = null; }} />
{#if points.length}
  <div class="wrap">
    <ChartFrame {xTicks} {yTicks} {xTitle} {yTitle} {hover} label="Scatter plot" onDismiss={() => { if (!brush) hover = null; }}>
      {#snippet children(plot)}
        {@const current = plot}
        <foreignObject {@attach syncPlot(current)} x={current.x} y={current.y} width={current.width} height={current.height}>
          <button
            type="button"
            class="hit"
            aria-label="Scatter plot. Drag to select a region. Keyboard: Enter creates or filters a region; arrows move it; Shift plus arrows resize it. Double-click resets zoom."
            onpointerdown={(event) => onPointerDown(event, current)}
            onpointermove={(event) => onPointerMove(event, current)}
            onpointerup={onPointerUp}
            onkeydown={(event) => keyboardSelect(event, current)}
            ondblclick={resetView}
          ></button>
        </foreignObject>
        {#if box && box.width + box.height > 0}
          <rect class="brush" x={box.x} y={box.y} width={box.width} height={box.height} />
        {/if}
      {/snippet}
    </ChartFrame>
    {#if plotRef}
      <canvas
        bind:this={canvasEl}
        class="dots"
        style:left={`${plotRef.x}px`}
        style:top={`${plotRef.y}px`}
        style:width={`${plotRef.width}px`}
        style:height={`${plotRef.height}px`}
      ></canvas>
    {/if}
    {#if brush?.ready && box}
      <div class="choice" style:left={`${box.x + box.width + 8}px`} style:top={`${box.y + box.height - 4}px`}>
        <button type="button" onclick={filterRegion}>Filter</button>
        <button type="button" onclick={zoomRegion}>Zoom</button>
      </div>
    {/if}
  </div>
{:else}
  <p class="empty">No values to chart.</p>
{/if}

<style>
  .wrap { position: relative; flex: 1; min-height: 0; display: flex; }
  .wrap :global(.frame) { z-index: 1; }
  .hit { width: 100%; height: 100%; padding: 0; border: 0; background: transparent; cursor: crosshair; }
  .hit:focus-visible { outline: 2px solid var(--chart-mark-strong); outline-offset: -2px; }
  .dots { position: absolute; pointer-events: none; z-index: 0; }
  .brush { fill: none; stroke: var(--chart-mark-strong); stroke-dasharray: 4 3; pointer-events: none; }
  .choice {
    position: absolute;
    z-index: 5;
    display: flex;
    gap: 4px;
    transform: translateY(-100%);
  }
  .choice button {
    height: 26px;
    padding: 0 9px;
    border-radius: var(--radius-sm);
    border: 1px solid var(--control-border);
    background: var(--surface);
    font-size: 11.5px;
    font-weight: 500;
    color: var(--ink);
    box-shadow: var(--shadow-panel);
  }
  .choice button:hover, .choice button:focus-visible { border-color: var(--chart-mark-strong); color: var(--chart-mark-ink); background: var(--chart-mark-fill); }
  .empty { margin: auto; font-size: 12.5px; color: var(--muted); }
</style>
