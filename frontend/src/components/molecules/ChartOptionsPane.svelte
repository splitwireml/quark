<script lang="ts">
  import { tick } from 'svelte';
  import IconButton from '../atoms/IconButton.svelte';
  import { floatingTooltips } from '../../lib/tooltips';
  import Eyebrow from '../atoms/Eyebrow.svelte';
  import Icon from '../atoms/Icon.svelte';
  import TextInput from '../atoms/TextInput.svelte';
  import ChartTypeToggle from './ChartTypeToggle.svelte';
  import ChipToggleGroup from './ChipToggleGroup.svelte';
  import RoleChip from './RoleChip.svelte';
  import { barLayouts, groupColumns, metricAligns, metricCardMetrics, metricFonts, metricSizes, timeGrains, visualizeMetrics } from '../../lib/visualize';
  import type { AggregateMetric, BarLayout, ChartSpec, MetricAlign, MetricFont, MetricSize, TimeGrain, ChartSuggestion, ChartType, ColumnInfo, EncodingRole } from '../../lib/types';

  type Props = {
    columnSearch: string;
    setColumnSearch: (value: string) => void;
    columns: ColumnInfo[];
    selected: ColumnInfo[];
    onToggleColumn: (name: string) => void;
    suggestions: ChartSuggestion[];
    spec: ChartSpec | null;
    onSelectChart: (chart: ChartType) => void;
    onSelectMetric: (metric: AggregateMetric) => void;
    onSelectLayout: (layout: BarLayout) => void;
    onSelectGrain: (grain: TimeGrain) => void;
    activeGrain: TimeGrain | null;
    onSelectCard: (patch: { align?: MetricAlign; font?: MetricFont; size?: MetricSize }) => void;
    onOpenFormula: () => void;
    onClearFormula: () => void;
    roles: EncodingRole[];
    roleOf: (name: string) => EncodingRole | null;
    onSetRole: (name: string, role: EncodingRole) => void;
    onAddToDashboard?: () => boolean;
    editingTitle?: string;
    onFinishEditing?: () => void;
  };

  let {
    columnSearch, setColumnSearch, columns, selected,
    onToggleColumn, suggestions, spec, onSelectChart, onSelectMetric, onSelectLayout, onSelectGrain, activeGrain,
    onSelectCard, onOpenFormula, onClearFormula,
    roles, roleOf, onSetRole,
    onAddToDashboard, editingTitle, onFinishEditing
  }: Props = $props();

  let pane: HTMLElement;
  let minimized = $state(false);
  let offset = $state({ x: 0, y: 0 });
  let dragging = $state(false);
  let drag: { pointerId: number; x: number; y: number; offset: typeof offset; target: HTMLElement } | null = null;
  let animation: Animation | undefined;
  let addedSpec = $state.raw<ChartSpec | null>(null);
  let transitioning = $state(false);
  let selectedNames = $derived(new Set(selected.map((column) => column.name)));
  let groups = $derived.by(() => {
    const query = columnSearch.trim().toLowerCase();
    const visible = query ? columns.filter((column) => column.name.toLowerCase().includes(query)) : columns;
    return groupColumns(visible);
  });
  let isMetric = $derived(spec?.chart === 'metric');
  let formula = $derived(isMetric ? spec?.expression?.trim() ?? '' : '');
  let showAggregate = $derived(
    ((spec?.chart === 'bar' || spec?.chart === 'pie') && !!spec.encodings.value)
    || (spec?.chart === 'line' && !!spec.encodings.y)
    || (isMetric && !formula)
  );
  // A metric card collapses one column to one number, so it offers only the operations that can.
  let aggregateOptions = $derived(spec?.chart === 'metric'
    ? metricCardMetrics(selected.find((column) => column.name === spec.encodings.value))
    : visualizeMetrics);
  function bounded(x: number, y: number) {
    const parent = pane.parentElement!;
    return {
      x: Math.max(Math.min(0, pane.offsetWidth + 24 - parent.clientWidth), Math.min(0, x)),
      y: Math.max(0, Math.min(parent.clientHeight - 144, y))
    };
  }

  function startDrag(event: PointerEvent) {
    if (event.button !== 0 || minimized) return;
    animation?.cancel();
    const target = event.currentTarget as HTMLElement;
    target.setPointerCapture(event.pointerId);
    drag = { pointerId: event.pointerId, x: event.clientX, y: event.clientY, offset: { ...offset }, target };
    dragging = true;
    event.preventDefault();
  }

  function moveDrag(event: PointerEvent) {
    if (!drag || drag.pointerId !== event.pointerId) return;
    offset = bounded(drag.offset.x + event.clientX - drag.x, drag.offset.y + event.clientY - drag.y);
  }

  function stopDrag() {
    const previous = drag;
    drag = null;
    dragging = false;
    if (previous?.target.hasPointerCapture(previous.pointerId)) previous.target.releasePointerCapture(previous.pointerId);
  }

  function moveWithKeys(event: KeyboardEvent) {
    const direction = { ArrowLeft: [-16, 0], ArrowRight: [16, 0], ArrowUp: [0, -16], ArrowDown: [0, 16] }[event.key];
    if (!direction) return;
    event.preventDefault();
    offset = bounded(offset.x + direction[0], offset.y + direction[1]);
  }

  async function toggleMinimized() {
    if (transitioning) return;
    stopDrag();
    const before = pane.getBoundingClientRect();
    const reduce = matchMedia('(prefers-reduced-motion: reduce)').matches;
    const timing = { duration: 280, easing: 'cubic-bezier(0.22, 1, 0.36, 1)' };
    transitioning = true;
    if (!minimized && !reduce) {
      const parent = pane.parentElement!.getBoundingClientRect();
      animation = pane.animate([
        { transform: 'none', opacity: 1 },
        { transform: `translate(${parent.right - 52 - before.left}px, ${parent.top + 12 - before.top}px) scale(${40 / before.width}, ${40 / before.height})`, opacity: .2 }
      ], { ...timing, fill: 'forwards' });
      try { await animation.finished; } catch { transitioning = false; return; }
    }
    minimized = !minimized;
    await tick();
    animation?.cancel();
    if (!minimized) {
      offset = bounded(offset.x, offset.y);
      await tick();
      const after = pane.getBoundingClientRect();
      if (!reduce) animation = pane.animate([
        { transform: `translate(${before.left - after.left}px, ${before.top - after.top}px) scale(${before.width / after.width}, ${before.height / after.height})`, opacity: .2 },
        { transform: 'none', opacity: 1 }
      ], timing);
    }
    transitioning = false;
    pane.querySelector<HTMLButtonElement>(minimized ? '.restore' : '[aria-label="Minimize chart palette"]')?.focus({ preventScroll: true });
  }

  export function reveal() {
    if (transitioning) { animation?.cancel(); transitioning = false; }
    if (minimized) void toggleMinimized();
  }

  function observeBounds(node: HTMLElement) {
    const observer = new ResizeObserver(() => { if (!minimized) offset = bounded(offset.x, offset.y); });
    observer.observe(node.parentElement!);
    observer.observe(node);
    return () => { observer.disconnect(); animation?.cancel(); };
  }
</script>

<svelte:window onpointermove={moveDrag} onpointerup={stopDrag} onpointercancel={stopDrag} onblur={stopDrag} />
<aside class="pane" class:minimized class:dragging aria-label="Chart options" bind:this={pane}
  style:--palette-y={`${offset.y}px`}
  style:translate={minimized ? 'none' : `${offset.x}px ${offset.y}px`}
  {@attach floatingTooltips} {@attach observeBounds}>
  {#if minimized}
    <button type="button" class="restore" aria-label="Restore chart palette" data-tip="Restore chart palette" onclick={toggleMinimized}>
      <Icon name="columns" size={16} />
    </button>
  {:else}
    <header class="pane-header">
      <button type="button" class="drag-handle" aria-label="Move chart palette" data-tip="Drag to move. Arrow keys also move the palette."
        onpointerdown={startDrag} onlostpointercapture={stopDrag} onkeydown={moveWithKeys}>{editingTitle ? 'Editing this chart' : 'Chart maker'}</button>
      <span class="dock-icon"><IconButton size="md" icon="arrow-right" label="Minimize chart palette" onclick={toggleMinimized} /></span>
    </header>
    <div class="pane-content">
      {#if editingTitle}
        <div class="editing-context">
          <strong title={editingTitle}>{editingTitle}</strong>
          <button type="button" onclick={onFinishEditing}>Done</button>
        </div>
        {#if !spec}<p class="muted" role="status">Choose columns for a supported chart. Your last chart is kept until then.</p>{/if}
      {/if}
      {#if spec && onAddToDashboard}
        <div class="pane-block add-dashboard">
          <button type="button" class="menu-trigger" data-tip="Add to dashboard" aria-label="Add to dashboard"
            onclick={() => { addedSpec = onAddToDashboard?.() ? spec : null; }}>
            <Icon name="plus" size={14} />
            <span class="menu-label"><span>Add</span></span>
          </button>
          <span class="added" role="status">{addedSpec === spec ? 'Added to dashboard' : ''}</span>
        </div>
      {/if}
      <div class="pane-block">
        <Eyebrow>Chart</Eyebrow>
        <ChartTypeToggle {suggestions} selected={spec?.chart ?? null} onSelect={onSelectChart} />
      </div>
      {#if showAggregate}
        <div class="pane-block">
          <Eyebrow>Aggregate</Eyebrow>
          <ChipToggleGroup
            label="Aggregation"
            options={aggregateOptions.map((metric) => ({ value: metric.value, label: metric.label, tip: metric.tip }))}
            selected={spec?.metric ?? (spec?.chart === 'line' ? 'avg' : null)}
            onSelect={(value) => onSelectMetric(value as AggregateMetric)}
          />
        </div>
      {/if}
      {#if isMetric || !spec}
        <div class="pane-block">
          <Eyebrow>Formula</Eyebrow>
          {#if formula}
            <code class="formula" title={formula}>{formula}</code>
            <ChipToggleGroup label="Formula actions" selected={null}
              options={[{ value: 'edit', label: 'Edit', tip: 'Change this formula' }, { value: 'clear', label: 'Clear', tip: 'Drop the formula and pick a column instead' }]}
              onSelect={(value) => value === 'edit' ? onOpenFormula() : onClearFormula()} />
          {:else}
            <ChipToggleGroup label="Formula" selected={null}
              options={[{ value: 'write', label: 'ƒx Write a formula', tip: 'Derive one value from any expression' }]}
              onSelect={onOpenFormula} />
          {/if}
        </div>
      {/if}
      {#if isMetric}
        <div class="pane-block">
          <Eyebrow>Card</Eyebrow>
          <ChipToggleGroup label="Alignment" options={metricAligns} selected={spec?.align ?? 'center'}
            onSelect={(value) => onSelectCard({ align: value as MetricAlign })} />
          <ChipToggleGroup label="Number font" options={metricFonts} selected={spec?.font ?? 'sans'}
            onSelect={(value) => onSelectCard({ font: value as MetricFont })} />
          <ChipToggleGroup label="Number size" options={metricSizes} selected={spec?.size ?? 'fit'}
            onSelect={(value) => onSelectCard({ size: value as MetricSize })} />
        </div>
      {/if}
      {#if spec?.chart === 'bar' && spec.encodings.group}
        <div class="pane-block">
          <Eyebrow>Layout</Eyebrow>
          <ChipToggleGroup label="Bar layout" options={barLayouts} selected={spec.layout ?? 'grouped'}
            onSelect={(value) => onSelectLayout(value as BarLayout)} />
        </div>
      {/if}
      {#if spec?.chart === 'line'}
        <div class="pane-block">
          <Eyebrow>Grain</Eyebrow>
          <ChipToggleGroup label="Time grain" options={timeGrains} selected={activeGrain}
            onSelect={(value) => onSelectGrain(value as TimeGrain)} />
        </div>
      {/if}
      {#if selected.length}
        <div class="pane-block">
          <Eyebrow>Selected</Eyebrow>
          <div class="chips">
            {#each selected as column (column.name)}
              <RoleChip
                name={column.name}
                role={roleOf(column.name)}
                {roles}
                onSetRole={(role) => onSetRole(column.name, role)}
                onRemove={() => onToggleColumn(column.name)}
              />
            {/each}
          </div>
        </div>
      {/if}
      <div class="search">
        <label for="visualize-column-search" class="sr-only">Find a column to add</label>
        <TextInput
          id="visualize-column-search"
          type="search"
          glyph="⌕"
          value={columnSearch}
          oninput={(event: Event) => setColumnSearch((event.currentTarget as HTMLInputElement).value)}
          placeholder="Find a column"
        />
      </div>
      {#each groups as group (group.label)}
        <div class="pane-block">
          <Eyebrow>{group.label}</Eyebrow>
          <div class="fields" role="list" aria-label={group.label}>
            {#each group.columns as column (column.name)}
              {@const on = selectedNames.has(column.name)}
              <button
                type="button"
                class="field"
                class:on
                aria-pressed={on}
                onclick={() => onToggleColumn(column.name)}
              >
                <span title={column.name}>{column.name}</span>
                <small>{column.type}</small>
              </button>
            {/each}
          </div>
        </div>
      {:else}
        <p class="muted">{columns.length ? 'No matching columns' : 'No chartable columns'}</p>
      {/each}
    </div>
  {/if}
</aside>

<style>
  .pane {
    position: absolute;
    top: 12px;
    right: 12px;
    z-index: 3;
    display: flex;
    flex-direction: column;
    width: min(244px, calc(100% - 24px));
    max-height: calc(100% - 24px - var(--palette-y, 0px));
    overflow: visible;
    border: 1px solid var(--line-strong);
    border-radius: var(--radius-2xl);
    background: var(--surface);
    box-shadow: var(--shadow-panel);
    transform-origin: top left;
  }
  .pane-block { display: flex; flex-direction: column; gap: 6px; min-width: 0; }
  .formula { min-width: 0; overflow: hidden; padding: 5px 7px; border: 1px solid var(--line); border-radius: var(--radius-sm); background: var(--surface-2); color: var(--ink); font: 11px var(--font-mono); text-overflow: ellipsis; white-space: nowrap; }
  .pane-header { flex: none; display: flex; align-items: center; gap: 6px; padding: 6px 8px 6px 12px; border-bottom: 1px solid var(--line); }
  .drag-handle { flex: 1; min-width: 0; height: 30px; padding: 0; border: 0; background: transparent; color: var(--muted); text-align: left; font: 500 12px var(--font-ui); cursor: grab; touch-action: none; }
  .dragging .drag-handle { cursor: grabbing; }
  .dock-icon { position: relative; display: flex; }
  .dock-icon::after { content: ''; position: absolute; right: 5px; top: 9px; height: 12px; border-right: 1.5px solid currentColor; color: var(--muted); pointer-events: none; }
  .editing-context { display: flex; align-items: center; gap: 8px; }
  .editing-context strong { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font: 500 12px var(--font-ui); color: var(--action-dark); }
  .editing-context button { flex: none; min-height: 28px; border: 1px solid var(--control-border); border-radius: var(--radius-md); padding: 0 8px; background: var(--surface); color: var(--ink); font: 12px var(--font-ui); }
  .editing-context button:hover { background: var(--action-tint); border-color: var(--action); }
  .pane-content { min-height: 0; display: flex; flex-direction: column; gap: 12px; padding: 12px; overflow: auto; overscroll-behavior: contain; }
  .pane.minimized { top: 12px; right: 12px; width: 40px; height: 40px; }
  .restore { display: grid; place-items: center; width: 100%; height: 100%; border: 0; border-radius: inherit; color: var(--action-dark); background: var(--surface); }
  .restore:hover { background: var(--action-tint); }
  .restore:active { transform: scale(.96); }
  .add-dashboard { flex-direction: row; align-items: center; }
  .add-dashboard .menu-trigger { flex: none; }
  .added { color: var(--muted); font-size: 11px; }
  .pane :global([data-tip]::after) { display: none; }
  .search :global(.field) { width: 100%; }
  .chips { display: flex; flex-direction: column; gap: 4px; }
  .fields { display: flex; flex-direction: column; gap: 2px; }
  .field {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    min-height: 28px;
    padding: 4px 8px;
    border: 0;
    border-radius: var(--radius-md);
    background: transparent;
    color: var(--ink);
    text-align: left;
  }
  .field:hover, .field:focus-visible { background: var(--surface-hover); }
  .field.on { background: var(--action-tint); color: var(--action-dark); }
  .field span { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font: 11.5px var(--font-mono); }
  .field small { flex: none; font: 10px var(--font-mono); color: var(--faint); }
  .field.on small { color: var(--action-dark); }
  .muted { margin: 0; font-size: 11px; color: var(--faint); }
</style>
