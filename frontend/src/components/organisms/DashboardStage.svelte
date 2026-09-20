<script lang="ts">
  import { tick, untrack } from 'svelte';
  import Button from '../atoms/Button.svelte';
  import TextInput from '../atoms/TextInput.svelte';
  import ChartView from '../molecules/ChartView.svelte';
  import { clampPlacement, DASHBOARD_WIDTH, DEFAULT_TILE } from '../../lib/dashboard';
  import type { AggregateCount, ChartMark, DashboardChart, DashboardPlacement, DashboardTab, HistogramBin, VisualizeResponse } from '../../lib/types';

  type ChartState = { data: VisualizeResponse | null; loading: boolean; error: string };
  type Rect = Pick<DashboardPlacement, 'x' | 'y' | 'width' | 'height'>;
  type Props = {
    tabs: DashboardTab[];
    activeTabId: string;
    charts: DashboardChart[];
    chartStates: Record<string, ChartState>;
    count: (value: AggregateCount) => string;
    compact: (value: number | string | null | undefined) => string;
    chartTheme?: string;
    binLabel: (bin: HistogramBin) => string;
    onSelectTab: (id: string) => void;
    onCreateTab: (name: string) => void;
    onPlace: (chartId: string, rect: Rect) => void;
    onMove: (placement: DashboardPlacement) => void;
    onRemove: (placementId: string) => void;
    onMark: (chartId: string, mark: ChartMark) => void;
    onRetryChart: (chartId: string) => void;
    onScroll: (top: number) => void;
  };

  let { tabs, activeTabId, charts, chartStates, count, compact, chartTheme = 'primary', binLabel, onSelectTab, onCreateTab, onPlace, onMove, onRemove, onMark, onRetryChart, onScroll }: Props = $props();
  let layoutMode = $state(false);
  let newName = $state('');
  let scroller = $state<HTMLDivElement | null>(null);
  let viewportHeight = $state(0);
  let board = $state<HTMLDivElement | null>(null);
  let boardWidth = $state(DASHBOARD_WIDTH);
  let draw = $state<{ x0: number; y0: number; x1: number; y1: number } | null>(null);
  let pending = $state<Rect | null>(null);
  let gesture = $state<{ kind: 'move' | 'resize'; startX: number; startY: number; original: DashboardPlacement } | null>(null);
  let draft = $state<DashboardPlacement | null>(null);
  let extraHeight = $state(760);
  let capture: { element: HTMLElement; pointerId: number } | null = null;
  let lastPointer: { clientX: number; clientY: number } | null = null;
  let frame = 0;
  let activeTab = $derived(tabs.find((tab) => tab.id === activeTabId));
  let canvasHeight = $derived(Math.max(extraHeight, (activeTab?.scrollTop ?? 0) + viewportHeight + 760,
    ...(activeTab?.placements.map((placement) => placement.y + placement.height + 240) ?? [0]),
    draft ? draft.y + draft.height + 240 : 0, pending ? pending.y + pending.height + 240 : 0,
    draw ? Math.max(draw.y0, draw.y1) + 240 : 0));
  let drawRect = $derived(draw ? fitPlacement({ id: '', chartId: '', x: Math.min(draw.x0, draw.x1), y: Math.min(draw.y0, draw.y1), width: Math.abs(draw.x1 - draw.x0), height: Math.abs(draw.y1 - draw.y0) }) : null);

  function fitPlacement(placement: DashboardPlacement): DashboardPlacement {
    const width = Math.min(Math.max(280, boardWidth), placement.width);
    return { ...clampPlacement({ ...placement, width }), width, x: Math.max(0, Math.min(boardWidth - width, placement.x)) };
  }

  function restoreScroll(node: HTMLDivElement) {
    const top = untrack(() => activeTab?.scrollTop ?? 0);
    node.scrollTop = top;
    return cancelInteractions;
  }

  function point(event: { clientX: number; clientY: number }) {
    const rect = board?.getBoundingClientRect();
    return rect ? { x: Math.max(0, Math.min(boardWidth, event.clientX - rect.left)), y: Math.max(0, event.clientY - rect.top) } : null;
  }

  function capturePointer(event: PointerEvent) {
    event.preventDefault();
    const element = event.currentTarget as HTMLElement;
    element.focus({ preventScroll: true });
    element.setPointerCapture(event.pointerId);
    capture = { element, pointerId: event.pointerId };
    lastPointer = { clientX: event.clientX, clientY: event.clientY };
    frame = requestAnimationFrame(autoScroll);
  }

  function releasePointer() {
    cancelAnimationFrame(frame);
    const previous = capture;
    capture = null;
    lastPointer = null;
    if (previous?.element.hasPointerCapture(previous.pointerId)) previous.element.releasePointerCapture(previous.pointerId);
  }

  function cancelInteractions() {
    draw = null;
    gesture = null;
    draft = null;
    pending = null;
    releasePointer();
  }

  function lostCapture(event: PointerEvent) {
    if (capture?.pointerId === event.pointerId) cancelInteractions();
  }

  function autoScroll() {
    if (!capture || !scroller || !lastPointer) return;
    const bounds = scroller.getBoundingClientRect();
    const dy = lastPointer.clientY > bounds.bottom - 40 ? 16 : lastPointer.clientY < bounds.top + 40 ? -16 : 0;
    const dx = lastPointer.clientX > bounds.right - 40 ? 16 : lastPointer.clientX < bounds.left + 40 ? -16 : 0;
    if (dy > 0) extraHeight = Math.max(extraHeight, scroller.scrollTop + scroller.clientHeight + 240);
    scroller.scrollBy(dx, dy);
    updatePointer(lastPointer);
    frame = requestAnimationFrame(autoScroll);
  }

  function startDraw(event: PointerEvent) {
    if (!layoutMode || event.button !== 0 || capture) return;
    const start = point(event);
    if (!start) return;
    pending = null;
    draw = { x0: start.x, y0: start.y, x1: start.x, y1: start.y };
    capturePointer(event);
  }

  function updatePointer(event: { clientX: number; clientY: number }) {
    const next = point(event);
    if (!next) return;
    if (draw) draw = { ...draw, x1: next.x, y1: next.y };
    if (gesture) {
      const dx = next.x - gesture.startX;
      const dy = next.y - gesture.startY;
      draft = fitPlacement(gesture.kind === 'move'
        ? { ...gesture.original, x: gesture.original.x + dx, y: gesture.original.y + dy }
        : { ...gesture.original, width: Math.min(boardWidth - gesture.original.x, gesture.original.width + dx), height: gesture.original.height + dy });
    }
  }

  function movePointer(event: PointerEvent) {
    if (capture?.pointerId !== event.pointerId) return;
    lastPointer = { clientX: event.clientX, clientY: event.clientY };
    updatePointer(lastPointer);
  }

  function finishPointer(event: PointerEvent) {
    if (capture?.pointerId !== event.pointerId) return;
    updatePointer(event);
    const rect = draw && (Math.abs(draw.x1 - draw.x0) >= 8 || Math.abs(draw.y1 - draw.y0) >= 8) ? drawRect : null;
    if (gesture && draft) onMove(draft);
    draw = null;
    gesture = null;
    draft = null;
    releasePointer();
    if (rect) pending = rect;
  }

  function doubleCreate(event: MouseEvent) {
    if (!layoutMode) return;
    const at = point(event);
    if (!at) return;
    pending = fitPlacement({ id: '', chartId: '', x: at.x - DEFAULT_TILE.width / 2, y: at.y - 24, ...DEFAULT_TILE });
  }

  function addRectangle() {
    pending = { x: 24, y: (scroller?.scrollTop ?? 0) + 24, ...DEFAULT_TILE };
  }

  function focusChooser(node: HTMLElement) {
    node.querySelector('button')?.focus({ preventScroll: true });
  }

  function closeChooser() {
    pending = null;
    board?.querySelector<HTMLButtonElement>('.canvas-surface')?.focus({ preventScroll: true });
  }

  function chooseChart(chartId: string) {
    if (!pending) return;
    onPlace(chartId, pending);
    closeChooser();
  }

  function startGesture(event: PointerEvent, placement: DashboardPlacement, kind: 'move' | 'resize') {
    if (!layoutMode || event.button !== 0 || capture) return;
    const start = point(event);
    if (!start) return;
    pending = null;
    gesture = { kind, startX: start.x, startY: start.y, original: placement };
    draft = placement;
    capturePointer(event);
  }

  function keyboardLayout(event: KeyboardEvent, placement: DashboardPlacement, resize = false) {
    if (!layoutMode || !['ArrowLeft', 'ArrowRight', 'ArrowUp', 'ArrowDown'].includes(event.key)) return;
    event.preventDefault();
    const dx = event.key === 'ArrowLeft' ? -8 : event.key === 'ArrowRight' ? 8 : 0;
    const dy = event.key === 'ArrowUp' ? -8 : event.key === 'ArrowDown' ? 8 : 0;
    onMove(fitPlacement(resize || event.shiftKey
      ? { ...placement, width: Math.min(boardWidth - placement.x, placement.width + dx), height: placement.height + dy }
      : { ...placement, x: placement.x + dx, y: placement.y + dy }));
  }

  async function selectTab(id: string) {
    cancelInteractions();
    extraHeight = 760;
    onSelectTab(id);
    await tick();
    document.getElementById(`dashboard-tab-${id}`)?.focus();
  }

  function tabKey(event: KeyboardEvent, index: number) {
    const next = event.key === 'ArrowRight' ? (index + 1) % tabs.length
      : event.key === 'ArrowLeft' ? (index + tabs.length - 1) % tabs.length
      : event.key === 'Home' ? 0 : event.key === 'End' ? tabs.length - 1 : -1;
    if (next < 0) return;
    event.preventDefault();
    void selectTab(tabs[next].id);
  }

  function escape(event: KeyboardEvent) {
    if (event.key !== 'Escape' || (!pending && !capture)) return;
    event.preventDefault();
    cancelInteractions();
    closeChooser();
  }
</script>

<svelte:window onpointermove={movePointer} onpointerup={finishPointer} onpointercancel={cancelInteractions} onblur={cancelInteractions} onkeydown={escape} />
<section class="dashboard" aria-label="Dashboard">
  <header class="dashboard-bar">
    <div class="tabs" role="tablist" aria-label="Dashboards">
      {#each tabs as tab, index (tab.id)}
        <button type="button" role="tab" id={`dashboard-tab-${tab.id}`} title={tab.name} aria-controls={`dashboard-panel-${tab.id}`} tabindex={tab.id === activeTabId ? 0 : -1} aria-selected={tab.id === activeTabId} class:active={tab.id === activeTabId} onclick={() => selectTab(tab.id)} onkeydown={(event) => tabKey(event, index)}>{tab.name}</button>
      {/each}
    </div>
    <form onsubmit={(event) => { event.preventDefault(); if (newName.trim()) { onCreateTab(newName.trim()); newName = ''; } }}>
      <TextInput value={newName} oninput={(event: Event) => newName = (event.currentTarget as HTMLInputElement).value} aria-label="New dashboard name" placeholder="New dashboard" maxlength="40" />
      <Button type="submit" disabled={!newName.trim()}>Add</Button>
    </form>
    {#if layoutMode && activeTab}<Button onclick={addRectangle}>Add rectangle</Button>{/if}
    <Button active={layoutMode} aria-pressed={layoutMode} disabled={!activeTab} onclick={() => { cancelInteractions(); layoutMode = !layoutMode; }}>{layoutMode ? 'Done arranging' : 'Arrange'}</Button>
  </header>
  {#if !activeTab}
    <div class="empty"><strong>Create a dashboard</strong><span>Name the first tab, then add a chart from Chart view.</span></div>
  {:else}
    {#if layoutMode}<p class="arrange-hint" id="dashboard-arrange-hint">Double-click for a rectangle, or drag to draw. Move with arrow keys; Shift + arrows to resize. Escape cancels.</p>{/if}
    {#key activeTabId}
    <div class="scroller" role="tabpanel" id={`dashboard-panel-${activeTabId}`} aria-labelledby={`dashboard-tab-${activeTabId}`} tabindex="0" bind:this={scroller} bind:clientHeight={viewportHeight} {@attach restoreScroll} onscroll={(event) => onScroll(event.currentTarget.scrollTop)}>
      <div
        class="board" class:layout={layoutMode} bind:this={board} bind:clientWidth={boardWidth}
        style:height={`${canvasHeight}px`}
      >
        <button type="button" class="canvas-surface" disabled={!layoutMode} aria-label="Add a chart rectangle to the empty canvas" aria-describedby={layoutMode ? 'dashboard-arrange-hint' : undefined} onpointerdown={startDraw} onlostpointercapture={lostCapture} ondblclick={doubleCreate} onclick={(event) => { if (event.detail === 0) addRectangle(); }}></button>
        {#if !activeTab.placements.length && !layoutMode}
          <div class="empty canvas-empty"><strong>This dashboard is empty</strong><span>Add the current chart from Chart view, or choose Arrange to place a saved chart.</span></div>
        {/if}
        {#each activeTab.placements as placement (placement.id)}
          {@const shown = fitPlacement(draft?.id === placement.id ? draft : placement)}
          {@const chart = charts.find((item) => item.id === placement.chartId)}
          {#if chart}
            <article class="tile" style:left={`${shown.x}px`} style:top={`${shown.y}px`} style:width={`${shown.width}px`} style:height={`${shown.height}px`}>
              <header>
                {#if layoutMode}
                  <button type="button" class="move" aria-label={`Move ${chart.title}`} aria-describedby="dashboard-arrange-hint" onpointerdown={(event) => startGesture(event, shown, 'move')} onlostpointercapture={lostCapture} onkeydown={(event) => keyboardLayout(event, shown)}>{chart.title}</button>
                  <Button variant="ghost" aria-label={`Remove ${chart.title}`} onclick={() => onRemove(placement.id)}>Remove</Button>
                {:else}<strong>{chart.title}</strong>{/if}
              </header>
              <div class="chart"><ChartView spec={chart.spec} data={chartStates[chart.id]?.data ?? null} loading={chartStates[chart.id]?.loading ?? false} error={chartStates[chart.id]?.error ?? ''} {count} {compact} {chartTheme} {binLabel} onMark={(mark) => onMark(chart.id, mark)} onRetry={() => onRetryChart(chart.id)} /></div>
              {#if layoutMode}<button type="button" class="resize" aria-label={`Resize ${chart.title}`} title="Resize with arrow keys or drag" onpointerdown={(event) => startGesture(event, shown, 'resize')} onlostpointercapture={lostCapture} onkeydown={(event) => keyboardLayout(event, shown, true)}><svg viewBox="0 0 16 16" width="14" height="14" aria-hidden="true"><path d="m5 13 8-8M10 13l3-3" fill="none" stroke="currentColor" stroke-width="1.5" /></svg></button>{/if}
            </article>
          {/if}
        {/each}
        {#if drawRect}<div class="draft" style:left={`${drawRect.x}px`} style:top={`${drawRect.y}px`} style:width={`${drawRect.width}px`} style:height={`${drawRect.height}px`}></div>{/if}
        {#if pending}
          <div class="draft" style:left={`${pending.x}px`} style:top={`${pending.y}px`} style:width={`${pending.width}px`} style:height={`${pending.height}px`}></div>
          <section class="chooser" aria-label="Choose a chart" style:left={`${Math.max(0, Math.min(pending.x, boardWidth - 260))}px`} style:top={`${pending.y}px`} {@attach focusChooser}>
            <strong>Choose a chart</strong>
            {#each charts as chart (chart.id)}<button type="button" onclick={() => chooseChart(chart.id)}>{chart.title}</button>{/each}
            {#if !charts.length}<span>Add a chart from Chart view first.</span>{/if}
            <button type="button" class="cancel" onclick={closeChooser}>Cancel</button>
          </section>
        {/if}
      </div>
    </div>
    {/key}
  {/if}
</section>

<style>
  .dashboard { flex: 1; min-width: 0; min-height: 0; display: flex; flex-direction: column; background: var(--canvas); }
  .dashboard-bar { flex: none; min-height: 42px; display: flex; flex-wrap: wrap; align-items: center; gap: 8px; padding: 6px 12px; border-bottom: 1px solid var(--line); background: var(--surface); }
  .tabs { flex: 1; min-width: 0; display: flex; gap: 2px; overflow-x: auto; }
  .tabs button { min-height: 30px; max-width: 240px; overflow: hidden; text-overflow: ellipsis; padding: 0 10px; border: 1px solid transparent; border-radius: var(--radius-md); background: transparent; color: var(--muted); font: 12.5px var(--font-ui); white-space: nowrap; }
  .tabs button:hover { color: var(--ink); background: var(--surface-hover); }
  .tabs button.active { border-color: var(--control-border); color: var(--action-dark); background: var(--action-tint); }
  form { display: flex; gap: 4px; }
  form :global(.field) { width: 132px; height: 30px; }
  .arrange-hint { flex: none; margin: 0; padding: 6px 12px; color: var(--muted); background: var(--surface-2); border-bottom: 1px solid var(--line); font-size: 12px; }
  .scroller { flex: 1; min-width: 0; min-height: 0; overflow: auto; overscroll-behavior: contain; overflow-anchor: none; scrollbar-color: var(--control-border) var(--canvas); }
  .board { position: relative; isolation: isolate; width: 100%; max-width: 1200px; min-height: 100%; margin: 0 auto; overflow: clip; background: var(--surface); }
  .canvas-surface { position: absolute; inset: 0; width: 100%; height: 100%; padding: 0; border: 0; border-radius: 0; background: transparent; cursor: default; }
  .canvas-surface:focus-visible { outline-offset: -3px; }
  .layout .canvas-surface { cursor: crosshair; touch-action: none; background-image: radial-gradient(var(--line-strong) 1px, transparent 1px); background-size: 24px 24px; }
  .tile { position: absolute; display: flex; flex-direction: column; min-width: 0; min-height: 0; overflow: hidden; border: 1px solid var(--line-strong); border-radius: var(--radius-card); background: var(--surface); }
  .tile > header { flex: none; height: 34px; display: flex; align-items: center; gap: 8px; padding: 0 10px; border-bottom: 1px solid var(--line); background: var(--surface-2); cursor: default; }
  .tile > header strong, .move { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font: 600 12.5px var(--font-ui); color: var(--ink); }
  .move { height: 100%; padding: 0; border: 0; background: transparent; text-align: left; cursor: move; touch-action: none; }
  .chart { flex: 1; min-width: 0; min-height: 0; overflow: hidden; display: flex; padding: 6px; }
  .resize { position: absolute; right: 0; bottom: 0; display: grid; place-items: center; width: 28px; height: 28px; border: 0; border-radius: var(--radius-md) 0 0 0; background: var(--surface-2); color: var(--action-dark); cursor: nwse-resize; touch-action: none; }
  .draft { position: absolute; border: 1px dashed var(--action); background: color-mix(in srgb, var(--action-tint) 65%, transparent); pointer-events: none; }
  .chooser { position: absolute; z-index: 8; width: 260px; max-height: 360px; overflow: auto; display: flex; flex-direction: column; gap: 4px; padding: 10px; border-radius: var(--radius-xl); background: var(--surface); box-shadow: var(--shadow-popover); }
  .chooser strong { margin-bottom: 4px; font-size: 12.5px; }
  .chooser button { flex: none; min-height: 30px; padding: 6px 8px; border: 0; border-radius: var(--radius-md); background: var(--surface-2); color: var(--ink); text-align: left; font: 12.5px var(--font-ui); overflow-wrap: anywhere; }
  .chooser button:hover, .chooser button:focus-visible { background: var(--action-tint); color: var(--action-dark); }
  .chooser .cancel { margin-top: 4px; color: var(--muted); background: transparent; }
  .chooser span { color: var(--muted); font-size: 12.5px; }
  button:focus-visible, .scroller:focus-visible { outline: 2px solid var(--action); outline-offset: -2px; }
  .empty { margin: auto; display: flex; flex-direction: column; align-items: center; gap: 6px; padding: 28px; color: var(--muted); font-size: 12.5px; text-align: center; }
  .empty strong { color: var(--ink); font-size: 14px; }
  .canvas-empty { position: absolute; inset: 0; pointer-events: none; }
  @media (prefers-reduced-motion: no-preference) { .chooser { animation: reveal 120ms ease-out; } @keyframes reveal { from { opacity: 0; transform: scale(.98); } to { opacity: 1; transform: scale(1); } } }
  @media (prefers-reduced-motion: reduce) { .dashboard :global(button) { transition: none; } }
</style>
