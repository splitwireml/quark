<script lang="ts">
  import { tick, untrack } from 'svelte';
  import Icon from '../atoms/Icon.svelte';
  import IconButton from '../atoms/IconButton.svelte';
  import EditableName from '../atoms/EditableName.svelte';
  import ChartOptionsPane from '../molecules/ChartOptionsPane.svelte';
  import ChartView from '../molecules/ChartView.svelte';
  import { clampPlacement, DEFAULT_TILE, snapMove, snapResize, type SnapGuide } from '../../lib/dashboard';
  import { suggestCharts } from '../../lib/visualize';
  import type { AggregateCount, AggregateMetric, ChartMark, ChartSpec, ChartSuggestion, ChartType, ColumnInfo, DashboardChart, DashboardPlacement, DashboardTab, HistogramBin, VisualizeResponse } from '../../lib/types';

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
    columnSearch: string;
    setColumnSearch: (value: string) => void;
    columns: ColumnInfo[];
    selected: ColumnInfo[];
    onToggleColumn: (name: string) => void;
    suggestions: ChartSuggestion[];
    spec: ChartSpec | null;
    onSelectChart: (chart: ChartType) => void;
    onSelectMetric: (metric: AggregateMetric) => void;
    onSelectTab: (id: string) => void;
    onCreateTab: () => void;
    onRenameTab: (id: string, name: string) => void;
    onRenameChart: (id: string, title: string) => void;
    onEditChart: (placementId: string, spec: ChartSpec) => void;
    onPlaceCurrent: (rect: Rect) => void;
    onMove: (placement: DashboardPlacement) => void;
    onRemove: (placementId: string) => void;
    onMark: (chartId: string, mark: ChartMark) => void;
    onRetryChart: (chartId: string) => void;
    onScroll: (top: number) => void;
  };

  let {
    tabs, activeTabId, charts, chartStates, count, compact, chartTheme = 'primary', binLabel,
    columnSearch, setColumnSearch, columns, selected, onToggleColumn, suggestions, spec, onSelectChart, onSelectMetric,
    onSelectTab, onCreateTab, onRenameTab, onRenameChart, onEditChart, onPlaceCurrent, onMove, onRemove, onMark, onRetryChart, onScroll
  }: Props = $props();
  let activeTab = $derived(tabs.find((tab) => tab.id === activeTabId));
  let palette: ChartOptionsPane;
  let editingId = $state('');
  let editColumns = $state.raw<ColumnInfo[]>([]);
  let editType = $state<ChartType | null>(null);
  let editMetric = $state<AggregateMetric | null>(null);
  let editingPlacement = $derived(activeTab?.placements.find((item) => item.id === editingId));
  let editingChart = $derived(charts.find((item) => item.id === editingPlacement?.chartId));
  let editSuggestions = $derived(suggestCharts(editColumns).filter((item) => item.chart !== 'line'));
  let editSpec = $derived.by((): ChartSpec | null => {
    const pick = editSuggestions.find((item) => item.chart === editType) ?? editSuggestions[0];
    if (!pick || !editingChart) return null;
    return { ...editingChart.spec, ...pick,
      metric: pick.chart === 'bar' && pick.encodings.value ? editMetric ?? pick.metric : pick.metric };
  });
  let scroller = $state<HTMLDivElement | null>(null);
  let viewportHeight = $state(0);
  let board = $state<HTMLDivElement | null>(null);
  let boardWidth = $state(0);
  let guides = $state<SnapGuide[]>([]);
  let draw = $state<{ x0: number; y0: number; x1: number; y1: number } | null>(null);
  let pending = $state<Rect | null>(null);
  let gesture = $state<{ kind: 'move' | 'resize'; startX: number; startY: number; original: DashboardPlacement } | null>(null);
  let draft = $state<DashboardPlacement | null>(null);
  let extraHeight = $state(760);
  let capture: { element: HTMLElement; pointerId: number } | null = null;
  let lastPointer: { clientX: number; clientY: number } | null = null;
  let frame = 0;
  let canvasWidth = $derived(Math.max(0, ...(activeTab?.placements.map((placement) => placement.x + placement.width) ?? [0])));
  let canvasHeight = $derived(Math.max(extraHeight, (activeTab?.scrollTop ?? 0) + viewportHeight + 760,
    ...(activeTab?.placements.map((placement) => placement.y + placement.height + 240) ?? [0]),
    draft ? draft.y + draft.height + 240 : 0, pending ? pending.y + pending.height + 240 : 0,
    draw ? Math.max(draw.y0, draw.y1) + 240 : 0));
  let drawRect = $derived(draw ? fitPlacement({ id: '', chartId: '', x: Math.min(draw.x0, draw.x1), y: Math.min(draw.y0, draw.y1), width: Math.abs(draw.x1 - draw.x0), height: Math.abs(draw.y1 - draw.y0) }) : null);

  function fitPlacement(placement: DashboardPlacement): DashboardPlacement {
    return clampPlacement(placement, boardWidth || undefined);
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

  function cancelGestures() {
    draw = null;
    gesture = null;
    draft = null;
    guides = [];
    releasePointer();
  }

  function cancelInteractions() {
    cancelGestures();
    pending = null;
    editingId = '';
  }

  function fitRect(rect: Rect): Rect {
    const fitted = fitPlacement({
      id: 'pending',
      chartId: 'pending',
      x: rect.x,
      y: rect.y,
      width: rect.width,
      height: rect.height
    });
    return { x: fitted.x, y: fitted.y, width: fitted.width, height: fitted.height };
  }

  function nextSlot(): Rect {
    return fitRect({
      x: 20,
      y: Math.max(20, ...(activeTab?.placements.map((placement) => placement.y + placement.height + 20) ?? [0])),
      ...DEFAULT_TILE
    });
  }

  function placeAt(rect: Rect) {
    if (!spec) {
      pending = rect;
      return;
    }
    pending = null;
    onPlaceCurrent(rect);
  }

  function startEditing(placement: DashboardPlacement) {
    const chart = charts.find((item) => item.id === placement.chartId);
    if (!chart) return;
    cancelInteractions();
    editingId = placement.id;
    const e = chart.spec.encodings;
    const names = [...new Set([e.x, e.y, e.category, e.value, e.group, e.size, e.color, e.pattern].filter(Boolean))];
    editColumns = names.map((name) => columns.find((column) => column.name === name)).filter((column): column is ColumnInfo => !!column);
    editType = chart.spec.chart;
    editMetric = chart.spec.metric ?? null;
    setColumnSearch('');
    palette.reveal();
  }

  function shiftEdit(event: MouseEvent, placement: DashboardPlacement) {
    if (!event.shiftKey || event.button !== 0 || (event.target as Element).closest('input, textarea')) return;
    event.preventDefault();
    event.stopPropagation();
    if (event.type === 'click') startEditing(placement);
  }

  function saveEdit() {
    if (editSpec && editingPlacement) onEditChart(editingPlacement.id, editSpec);
  }

  function pickChart(chart: ChartType) {
    if (editingChart) {
      editType = chart;
      saveEdit();
      return;
    }
    onSelectChart(chart);
    placeAt(pending ?? nextSlot());
  }

  function pickMetric(metric: AggregateMetric) {
    if (editingChart) { editMetric = metric; saveEdit(); }
    else onSelectMetric(metric);
  }

  function toggleColumn(name: string) {
    if (editingChart) {
      const column = columns.find((item) => item.name === name);
      if (!column) return;
      editColumns = editColumns.some((item) => item.name === name)
        ? editColumns.filter((item) => item.name !== name) : [...editColumns, column];
      setColumnSearch('');
      saveEdit();
      return;
    }
    onToggleColumn(name);
    if (!pending) return;
    void tick().then(() => { if (pending && spec) placeAt(pending); });
  }

  function lostCapture(event: PointerEvent) {
    if (capture?.pointerId === event.pointerId) cancelGestures();
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
    if (event.button !== 0 || capture) return;
    const start = point(event);
    if (!start) return;
    pending = null;
    editingId = '';
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
      if (gesture.kind === 'move' && Math.hypot(dx, dy) < 3) {
        draft = gesture.original;
        guides = [];
        return;
      }
      const resized = fitPlacement(gesture.kind === 'move'
        ? { ...gesture.original, x: gesture.original.x + dx, y: gesture.original.y + dy }
        : { ...gesture.original, width: Math.min(boardWidth - gesture.original.x, gesture.original.width + dx), height: gesture.original.height + dy });
      const snapped = gesture.kind === 'resize'
        ? snapResize(resized, activeTab?.placements ?? [])
        : snapMove(resized, activeTab?.placements ?? [], boardWidth);
      draft = fitPlacement(snapped.placement);
      guides = snapped.guides;
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
    guides = [];
    releasePointer();
    if (rect) commitRect(rect);
  }

  function commitRect(rect: Rect) {
    if (spec) placeAt(rect);
    else pending = rect;
  }

  function doubleCreate(event: MouseEvent) {
    const at = point(event);
    if (!at) return;
    commitRect(fitRect({ x: at.x - DEFAULT_TILE.width / 2, y: at.y - 24, ...DEFAULT_TILE }));
  }

  function addRectangle() {
    commitRect(nextSlot());
  }

  function startGesture(event: PointerEvent, placement: DashboardPlacement, kind: 'move' | 'resize') {
    if (event.button !== 0 || capture || event.detail > 1) return;
    const start = point(event);
    if (!start) return;
    pending = null;
    gesture = { kind, startX: start.x, startY: start.y, original: placement };
    draft = placement;
    capturePointer(event);
  }

  function keyboardLayout(event: KeyboardEvent, placement: DashboardPlacement, resize = false) {
    if (!['ArrowLeft', 'ArrowRight', 'ArrowUp', 'ArrowDown'].includes(event.key)) return;
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
    if (event.key !== 'Escape') return;
    if (!pending && !capture) { editingId = ''; return; }
    event.preventDefault();
    cancelInteractions();
  }
</script>

<svelte:window onpointermove={movePointer} onpointerup={finishPointer} onpointercancel={cancelGestures} onblur={cancelGestures} onkeydown={escape} />
<section class="dashboard" aria-label="Dashboard">
  <p id="dashboard-edit-hint" class="sr-only">Shift-click a chart or focus its title and press Shift+Enter to edit it.</p>
  <header class="dashboard-bar">
    <div class="tabs" role="tablist" aria-label="Dashboards">
      {#each tabs as tab, index (tab.id)}
        <EditableName value={tab.name} label="Dashboard name" maxlength={40} onSave={(name) => onRenameTab(tab.id, name)} role="tab" id={`dashboard-tab-${tab.id}`} aria-controls={`dashboard-panel-${tab.id}`} tabindex={tab.id === activeTabId ? 0 : -1} aria-selected={tab.id === activeTabId} class={tab.id === activeTabId ? "active" : ""} onclick={() => selectTab(tab.id)} onkeydown={(event) => tabKey(event, index)} />
      {/each}
    </div>
    <div class="bar-actions">
      <button type="button" class="menu-trigger" data-tip="New dashboard" data-tip-position="top" aria-label="New dashboard" onclick={() => { cancelInteractions(); onCreateTab(); }}>
        <Icon name="plus" size={14} />
        <span class="menu-label"><span>New</span></span>
      </button>
    </div>
  </header>
  <div class="workspace">
    <ChartOptionsPane
      bind:this={palette}
      {columnSearch} {setColumnSearch} {columns} selected={editingChart ? editColumns : selected}
      onToggleColumn={toggleColumn}
      suggestions={editingChart ? editSuggestions : suggestions} spec={editingChart ? editSpec : spec}
      editingTitle={editingChart?.title} onFinishEditing={() => editingId = ''}
      onSelectChart={pickChart} onSelectMetric={pickMetric}
    />
    {#key activeTabId || 'empty'}
    <div class="scroller" role="tabpanel" id={`dashboard-panel-${activeTabId || 'empty'}`} aria-labelledby={activeTab ? `dashboard-tab-${activeTabId}` : undefined} tabindex="0" bind:this={scroller} bind:clientHeight={viewportHeight} {@attach restoreScroll} onscroll={(event) => onScroll(event.currentTarget.scrollTop)}>
      <div
        class="board layout" bind:this={board} bind:clientWidth={boardWidth}
        style:height={`${canvasHeight}px`} style:min-width={`${canvasWidth}px`}
      >
        <button type="button" class="canvas-surface" aria-label="Draw a chart on the canvas" onpointerdown={startDraw} onlostpointercapture={lostCapture} ondblclick={doubleCreate} onclick={(event) => { if (event.detail === 0) addRectangle(); }}></button>
        {#if !activeTab?.placements.length && !pending && !draw}
          <div class="empty canvas-empty"><strong>Place a chart</strong><span>Pick a chart type in the pane, or drag on the board.</span></div>
        {/if}
        {#each activeTab?.placements ?? [] as placement (placement.id)}
          {@const shown = fitPlacement(draft?.id === placement.id ? draft : placement)}
          {@const chart = charts.find((item) => item.id === placement.chartId)}
          {#if chart}
            <article class="tile" class:editing={editingPlacement?.id === placement.id}
              onpointerdowncapture={(event) => shiftEdit(event, placement)} onclickcapture={(event) => shiftEdit(event, placement)}
              style:left={`${shown.x}px`} style:top={`${shown.y}px`} style:width={`${shown.width}px`} style:height={`${shown.height}px`}>
              <header>
                <EditableName value={chart.title} label="Chart title" onSave={(title) => onRenameChart(chart.id, title)} onEdit={cancelGestures} class="move" aria-label={`Move ${chart.title}`} aria-describedby="dashboard-edit-hint" onpointerdown={(event) => startGesture(event, shown, 'move')} onlostpointercapture={lostCapture} onkeydown={(event) => {
                  if (event.shiftKey && event.key === 'Enter') { event.preventDefault(); startEditing(placement); }
                  else keyboardLayout(event, shown);
                }} />
                <IconButton type="button" glyph="×" label={`Remove ${chart.title}`} onclick={() => onRemove(placement.id)} />
              </header>
              <div class="chart"><ChartView spec={chart.spec} data={chartStates[chart.id]?.data ?? null} loading={chartStates[chart.id]?.loading ?? false} error={chartStates[chart.id]?.error ?? ''} {count} {compact} {chartTheme} {binLabel} onMark={(mark) => onMark(chart.id, mark)} onRetry={() => onRetryChart(chart.id)} /></div>
              <button type="button" class="resize" aria-label={`Resize ${chart.title}`} title="Resize with arrow keys or drag" onpointerdown={(event) => startGesture(event, shown, 'resize')} onlostpointercapture={lostCapture} onkeydown={(event) => keyboardLayout(event, shown, true)}><svg viewBox="0 0 16 16" width="14" height="14" aria-hidden="true"><path d="m5 13 8-8M10 13l3-3" fill="none" stroke="currentColor" stroke-width="1.5" /></svg></button>
            </article>
          {/if}
        {/each}
        {#each guides as guide (guide.axis)}
          <div class="snap-guide" aria-hidden="true"
            style:left={`${guide.axis === 'x' ? guide.position : guide.start}px`}
            style:top={`${guide.axis === 'y' ? guide.position : guide.start}px`}
            style:width={`${guide.axis === 'x' ? 1 : guide.end - guide.start}px`}
            style:height={`${guide.axis === 'y' ? 1 : guide.end - guide.start}px`}></div>
        {/each}
        {#if drawRect}<div class="draft" style:left={`${drawRect.x}px`} style:top={`${drawRect.y}px`} style:width={`${drawRect.width}px`} style:height={`${drawRect.height}px`}></div>{/if}
        {#if pending}
          <article class="tile pending-tile" style:left={`${pending.x}px`} style:top={`${pending.y}px`} style:width={`${pending.width}px`} style:height={`${pending.height}px`}>
            <header>
              <strong>New chart</strong>
              <IconButton type="button" glyph="×" label="Cancel new chart" onclick={() => pending = null} />
            </header>
            <div class="chart empty-slot">Pick a chart type</div>
          </article>
        {/if}
      </div>
    </div>
    {/key}
  </div>
</section>

<style>
  .dashboard { flex: 1; min-width: 0; min-height: 0; display: flex; flex-direction: column; background: var(--canvas); }
  .dashboard-bar { flex: none; min-height: 42px; display: flex; flex-wrap: wrap; align-items: center; gap: 8px; padding: 6px 12px; border-bottom: 1px solid var(--line); background: var(--surface); }
  .tabs { flex: 1; min-width: 0; display: flex; gap: 2px; overflow-x: auto; }
  .tabs :global(button) { min-height: 30px; max-width: 240px; overflow: hidden; text-overflow: ellipsis; padding: 0 10px; border: 1px solid transparent; border-radius: var(--radius-md); background: transparent; color: var(--muted); font: 12.5px var(--font-ui); white-space: nowrap; }
  .tabs :global(button:hover) { color: var(--ink); background: var(--surface-hover); }
  .tabs :global(button.active) { border-color: var(--control-border); color: var(--action-dark); background: var(--action-tint); }
  .bar-actions { display: flex; align-items: center; gap: 6px; flex: none; }
  .workspace { position: relative; flex: 1; min-width: 0; min-height: 0; display: flex; }
  .scroller { flex: 1; min-width: 0; min-height: 0; overflow: auto; overscroll-behavior: contain; overflow-anchor: none; scrollbar-color: var(--control-border) var(--canvas); }
  .board { position: relative; isolation: isolate; width: 100%; min-height: 100%; overflow: clip; background: var(--surface); }
  .canvas-surface { position: absolute; inset: 0; width: 100%; height: 100%; padding: 0; border: 0; border-radius: 0; background: transparent; cursor: default; }
  .canvas-surface:focus-visible { outline-offset: -3px; }
  .layout .canvas-surface { cursor: crosshair; touch-action: none; background-image: radial-gradient(var(--line-strong) 1px, transparent 1px); background-size: 24px 24px; }
  .tile { position: absolute; display: flex; flex-direction: column; min-width: 0; min-height: 0; overflow: hidden; border: 1px solid var(--line-strong); border-radius: var(--radius-card); background: var(--surface); }
  .tile.editing { border-color: var(--action); box-shadow: inset 0 0 0 1px var(--action); }
  .tile > header { flex: none; height: 34px; display: flex; align-items: center; gap: 8px; padding: 0 10px; border-bottom: 1px solid var(--line); background: var(--surface-2); cursor: default; }
  .tile > header strong, .tile :global(.move) { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font: 600 12.5px var(--font-ui); color: var(--ink); }
  .tile :global(.move) { height: 100%; padding: 0; border: 0; background: transparent; text-align: left; cursor: move; touch-action: none; }
  .chart { flex: 1; min-width: 0; min-height: 0; overflow: hidden; display: flex; padding: 6px; }
  .resize { position: absolute; right: 0; bottom: 0; z-index: 2; display: grid; place-items: center; width: 28px; height: 28px; border: 0; border-radius: var(--radius-md) 0 0 0; background: var(--surface-2); color: var(--action-dark); cursor: nwse-resize; touch-action: none; }
  .snap-guide { position: absolute; z-index: 5; background: var(--action); pointer-events: none; }
  .tabs :global(.name-input) { width: 180px; flex: none; }
  .tile > header :global(.name-input) { flex: 1; font: 600 12.5px var(--font-ui); }
  .draft { position: absolute; border: 1px dashed var(--action); background: color-mix(in srgb, var(--action-tint) 65%, transparent); pointer-events: none; }
  .pending-tile { z-index: 4; }
  .empty-slot { margin: auto; color: var(--muted); font-size: 12.5px; }
  button:focus-visible, .scroller:focus-visible { outline: 2px solid var(--action); outline-offset: -2px; }
  .empty { margin: auto; display: flex; flex-direction: column; align-items: center; gap: 6px; padding: 28px; color: var(--muted); font-size: 12.5px; text-align: center; }
  .empty strong { color: var(--ink); font-size: 14px; }
  .canvas-empty { position: absolute; inset: 0; pointer-events: none; }
  @media (prefers-reduced-motion: reduce) {
    .dashboard :global(button) { transition: none; }
  }
</style>
