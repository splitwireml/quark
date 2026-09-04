<script lang="ts">
  import { onDestroy, tick } from 'svelte';
  import MenuPopover from '../molecules/MenuPopover.svelte';
  import Button from '../atoms/Button.svelte';
  import ToggleChip from '../atoms/ToggleChip.svelte';
  import TextInput from '../atoms/TextInput.svelte';
  import type { ColumnInfo } from '../../lib/types';

  type Placement = 'before' | 'after';

  type Props = {
    open: boolean;
    ontoggle: (event: Event) => void;
    visibleCount: number;
    totalCount: number;
    columnMenuSearch: string;
    setColumnMenuSearch: (value: string) => void;
    columnTypes: string[];
    columnTypeCounts: Record<string, number>;
    isTypeShown: (type: string) => boolean;
    toggleShownType: (type: string, checked: boolean) => void;
    typeToggleDisabled: (type: string) => boolean;
    nullThreshold: number;
    setNullThreshold: (value: number) => void;
    onHideFullyEmpty: () => void;
    onApplyThreshold: () => void;
    onShowAll: () => void;
    hiddenCount: number;
    columnMenuItems: ColumnInfo[];
    hiddenColumns: string[];
    isColumnProtected: (name: string) => boolean;
    visibleColumnsLength: number;
    onToggleColumn: (name: string, checked: boolean) => void;
    orderedColumnNames: string[];
    onBeginReorder: () => void;
    onPreviewReorder: (dragged: string, target: string, placement: Placement) => void;
    onCommitReorder: () => void;
    onCancelReorder: () => void;
    onMoveColumn: (name: string, direction: -1 | 1) => void;
    onRegexVisibility: (pattern: string, invert: boolean, action: 'show' | 'hide') => string;
  };

  let {
    open, ontoggle, visibleCount, totalCount, columnMenuSearch, setColumnMenuSearch,
    columnTypes, columnTypeCounts, isTypeShown, toggleShownType, typeToggleDisabled,
    nullThreshold, setNullThreshold, onHideFullyEmpty, onApplyThreshold, onShowAll, hiddenCount,
    columnMenuItems, hiddenColumns, isColumnProtected, visibleColumnsLength, onToggleColumn,
    orderedColumnNames, onBeginReorder, onPreviewReorder, onCommitReorder, onCancelReorder,
    onMoveColumn, onRegexVisibility
  }: Props = $props();

  let pattern = $state('');
  let invert = $state(false);
  let regexError = $state('');
  let listElement: HTMLDivElement | null = null;
  let draggedName = $state<string | null>(null);
  let dropTarget = $state<{ name: string; placement: Placement } | null>(null);
  let lastPlacement = '';
  let lastClientY = 0;
  let edgeFrame = 0;
  let animations: Animation[] = [];

  function rowPositions(): Map<string, number> {
    return new Map([...(listElement?.querySelectorAll<HTMLElement>('.row[data-column]') ?? [])].map((row) => [row.dataset.column!, row.getBoundingClientRect().top]));
  }

  function cancelAnimations() {
    for (const animation of animations) animation.cancel();
    animations = [];
  }

  function animateRows(before: Map<string, number>) {
    cancelAnimations();
    if (matchMedia('(prefers-reduced-motion: reduce)').matches) return;
    for (const row of listElement?.querySelectorAll<HTMLElement>('.row[data-column]') ?? []) {
      const previous = before.get(row.dataset.column!);
      if (previous === undefined) continue;
      const delta = previous - row.getBoundingClientRect().top;
      if (Math.abs(delta) < 0.5) continue;
      animations.push(row.animate([{ transform: `translateY(${delta}px)` }, { transform: 'translateY(0)' }], { duration: 160, easing: 'ease-out' }));
    }
  }

  function startEdgeScroll() {
    if (!edgeFrame) edgeFrame = requestAnimationFrame(edgeScroll);
  }

  function edgeScroll() {
    edgeFrame = 0;
    if (!draggedName || !listElement) return;
    const rect = listElement.getBoundingClientRect();
    const edge = 48;
    const speed = lastClientY < rect.top + edge
      ? -14 * Math.min(1, (rect.top + edge - lastClientY) / edge)
      : lastClientY > rect.bottom - edge
        ? 14 * Math.min(1, (lastClientY - rect.bottom + edge) / edge)
        : 0;
    if (speed) listElement.scrollTop += speed;
    edgeFrame = requestAnimationFrame(edgeScroll);
  }

  function stopEdgeScroll() {
    if (edgeFrame) cancelAnimationFrame(edgeFrame);
    edgeFrame = 0;
  }

  function startMenuDrag(event: DragEvent, name: string) {
    const target = event.target as HTMLElement;
    if (target.closest('input, label, button:not(.drag-handle)') || !event.dataTransfer) { event.preventDefault(); return; }
    draggedName = name;
    dropTarget = null;
    lastPlacement = '';
    lastClientY = event.clientY;
    event.dataTransfer.effectAllowed = 'move';
    event.dataTransfer.setData('text/plain', name);
    onBeginReorder();
    startEdgeScroll();
  }

  async function previewRow(event: DragEvent, target: string) {
    if (!draggedName || draggedName === target) return;
    event.preventDefault();
    if (event.dataTransfer) event.dataTransfer.dropEffect = 'move';
    lastClientY = event.clientY;
    const rect = (event.currentTarget as HTMLElement).getBoundingClientRect();
    const placement: Placement = event.clientY < rect.top + rect.height / 2 ? 'before' : 'after';
    const key = `${target}:${placement}`;
    dropTarget = { name: target, placement };
    if (key === lastPlacement) return;
    lastPlacement = key;
    const before = rowPositions();
    onPreviewReorder(draggedName, target, placement);
    await tick();
    if (draggedName) animateRows(before);
  }

  function listDragOver(event: DragEvent) {
    if (!draggedName) return;
    event.preventDefault();
    lastClientY = event.clientY;
    startEdgeScroll();
  }

  function dropRow(event: DragEvent) {
    if (!draggedName) return;
    event.preventDefault();
    onCommitReorder();
    clearDrag();
  }

  function endMenuDrag() {
    if (!draggedName) return;
    onCancelReorder();
    clearDrag();
  }

  function clearDrag() {
    draggedName = null;
    dropTarget = null;
    lastPlacement = '';
    stopEdgeScroll();
    cancelAnimations();
  }

  function menuKeydown(event: KeyboardEvent) {
    if (event.key !== 'Escape' || !draggedName) return;
    event.preventDefault();
    onCancelReorder();
    clearDrag();
  }

  async function moveOneStep(name: string, direction: -1 | 1) {
    const before = rowPositions();
    onMoveColumn(name, direction);
    await tick();
    animateRows(before);
  }

  function setPattern(value: string) {
    pattern = value;
    regexError = '';
  }

  function applyRegex(action: 'show' | 'hide') {
    regexError = onRegexVisibility(pattern, invert, action);
  }

  onDestroy(() => {
    if (draggedName) onCancelReorder();
    stopEdgeScroll();
    cancelAnimations();
  });
</script>

<svelte:window onkeydown={menuKeydown} />

<MenuPopover {open} {ontoggle} icon="columns" label="Columns" hint={`${visibleCount}/${totalCount}`} width={400} flush>
  {#snippet header()}
    <strong>Columns</strong><span>{visibleCount} of {totalCount} visible</span>
  {/snippet}
    <div class="search">
      <label for="column-menu-search" class="sr-only">Search columns</label>
      <TextInput id="column-menu-search" type="search" glyph="⌕" value={columnMenuSearch} oninput={(event: Event) => setColumnMenuSearch((event.currentTarget as HTMLInputElement).value)} placeholder="Search columns" />
    </div>
    <div class="regex-panel">
      <label for="column-regex">Select columns by regular expression</label>
      <div class="regex-input">
        <TextInput id="column-regex" value={pattern} oninput={(event: Event) => setPattern((event.currentTarget as HTMLInputElement).value)} placeholder="Pattern, e.g. id$" aria-invalid={!!regexError} aria-describedby={regexError ? 'column-regex-error' : undefined} />
        <label class="invert"><input type="checkbox" checked={invert} onchange={(event) => invert = event.currentTarget.checked} /> Invert</label>
      </div>
      <div class="regex-actions">
        <Button type="button" onclick={() => applyRegex('show')}>Show selected</Button>
        <Button type="button" onclick={() => applyRegex('hide')}>Hide selected</Button>
      </div>
      {#if regexError}<p id="column-regex-error" class="regex-error" role="alert">{regexError}</p>{/if}
    </div>
    <div class="type-row" role="group" aria-label="Show by type">
      {#each columnTypes as type (type)}
        <ToggleChip
          on={isTypeShown(type)} label={type} badge={columnTypeCounts[type] ?? 0}
          disabled={typeToggleDisabled(type)} aria-label={`${type}: ${columnTypeCounts[type] ?? 0} columns`}
          onclick={() => toggleShownType(type, !isTypeShown(type))}
        />
      {/each}
    </div>
    <div class="threshold-row">
      <Button onclick={onHideFullyEmpty}>Hide 100% null</Button>
      <label class="threshold">Hide ≥
        <input type="number" min="0" max="100" step="1" value={nullThreshold} oninput={(event) => setNullThreshold(Number((event.currentTarget as HTMLInputElement).value))} aria-label="Null percentage" />
        % null
      </label>
      <Button onclick={onApplyThreshold}>Apply</Button>
      <button type="button" class="link" onclick={onShowAll} disabled={hiddenCount === 0}>Show all</button>
    </div>
    <div bind:this={listElement} class="list" role="list" ondragover={listDragOver} ondrop={dropRow}>
      {#each columnMenuItems as column (column.name)}
        {@const visibilityDisabled = isColumnProtected(column.name) || (!hiddenColumns.includes(column.name) && visibleColumnsLength <= 1)}
        {@const orderIndex = orderedColumnNames.indexOf(column.name)}
        <div
          class="row"
          class:dragging={draggedName === column.name}
          class:drop-before={dropTarget?.name === column.name && dropTarget.placement === 'before'}
          class:drop-after={dropTarget?.name === column.name && dropTarget.placement === 'after'}
          data-column={column.name}
          role="listitem"
          draggable="true"
          ondragstart={(event) => startMenuDrag(event, column.name)}
          ondragover={(event) => previewRow(event, column.name)}
          ondragend={endMenuDrag}
        >
          <button type="button" class="drag-handle" draggable="true" aria-label={`Drag to reorder column ${column.name}`} aria-pressed={draggedName === column.name} title={`Drag to reorder ${column.name}`} onclick={(event) => event.stopPropagation()}>⋮⋮</button>
          <label class="visibility" class:disabled={visibilityDisabled}>
            <input type="checkbox" checked={!hiddenColumns.includes(column.name)} disabled={visibilityDisabled} onchange={(event) => onToggleColumn(column.name, event.currentTarget.checked)} />
            <span class="box" aria-hidden="true">{#if !hiddenColumns.includes(column.name)}✓{/if}</span>
            <span class="name" title={column.name}>{column.name}</span>
            <small>{column.type}</small>
            <small class:warn={column.null_fraction >= 0.3}>{(column.null_fraction * 100).toFixed(0)}%</small>
          </label>
          <div class="move-actions">
            <button type="button" disabled={orderIndex <= 0} aria-label={`Move ${column.name} up`} title="Move up" onclick={(event) => { event.stopPropagation(); void moveOneStep(column.name, -1); }}>↑</button>
            <button type="button" disabled={orderIndex < 0 || orderIndex >= orderedColumnNames.length - 1} aria-label={`Move ${column.name} down`} title="Move down" onclick={(event) => { event.stopPropagation(); void moveOneStep(column.name, 1); }}>↓</button>
          </div>
        </div>
      {:else}
        <p class="muted">No matching columns.</p>
      {/each}
    </div>
  {#snippet footer()}
    <span class="hint">Dedupe keys stay visible</span><span class="esc">esc</span>
  {/snippet}
</MenuPopover>

<style>
  .search { padding: 9px 11px 0; }
  .regex-panel { display: grid; gap: 6px; padding: 9px 11px 0; }
  .regex-panel > label { font-size: 10.5px; color: var(--muted); }
  .regex-input, .regex-actions { display: flex; align-items: center; gap: 7px; }
  .regex-input :global(.field) { flex: 1; }
  .invert { display: inline-flex; align-items: center; gap: 4px; white-space: nowrap; font-size: 11px; color: var(--ink-2); }
  .regex-error { margin: 0; font-size: 11px; color: var(--error); }
  .type-row { display: flex; flex-wrap: wrap; gap: 5px; padding: 9px 11px 0; }
  .threshold-row { display: flex; flex-wrap: wrap; align-items: center; gap: 6px; padding: 9px 11px 11px; border-bottom: 1px solid var(--line-soft); }
  .threshold { display: inline-flex; align-items: center; gap: 5px; font-size: 11px; color: var(--ink-2); }
  .threshold input { width: 34px; height: 22px; text-align: center; border-radius: var(--radius-sm); border: 1px solid var(--control-border); }
  .link { font-size: 11px; color: var(--action); background: none; border: none; }
  .link:disabled { color: var(--disabled); }
  .list { max-height: 260px; overflow-y: auto; padding: 6px; display: flex; flex-direction: column; }
  .row { display: flex; align-items: center; gap: 6px; min-height: 30px; padding: 0 4px; border-radius: var(--radius-md); }
  .row:hover { background: var(--surface-hover); }
  .row.dragging { opacity: 0.55; }
  .row.drop-before { box-shadow: inset 0 2px var(--action); }
  .row.drop-after { box-shadow: inset 0 -2px var(--action); }
  .drag-handle { flex: none; width: 20px; height: 22px; padding-right: 3px; border: 0; border-radius: var(--radius-sm); background: transparent; color: var(--muted); cursor: grab; letter-spacing: -3px; }
  .drag-handle:hover, .drag-handle[aria-pressed="true"] { background: var(--action-tint); color: var(--action); }
  .drag-handle:active { cursor: grabbing; }
  .visibility { min-width: 0; flex: 1; display: flex; align-items: center; gap: 7px; cursor: pointer; }
  .visibility.disabled { opacity: 0.5; cursor: not-allowed; }
  .visibility input { position: absolute; width: 1px; height: 1px; opacity: 0; }
  .box {
    display: inline-flex; align-items: center; justify-content: center; flex: none;
    width: 12px; height: 12px; border-radius: 2px; border: 1px solid var(--glyph);
    color: #fff; font-size: 8px;
  }
  .visibility input:checked + .box { background: var(--action); border-color: var(--action); }
  .name { font-family: var(--font-mono); font-size: 11px; color: var(--ink); flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .row small { max-width: 62px; overflow: hidden; text-overflow: ellipsis; font-family: var(--font-mono); font-size: 9.5px; color: var(--faint); flex: none; }
  .row small.warn { color: var(--warning); }
  .move-actions { display: flex; gap: 2px; flex: none; }
  .move-actions button { width: 20px; height: 22px; border: 1px solid var(--line); border-radius: var(--radius-sm); background: var(--surface); color: var(--muted); font-size: 10px; }
  .move-actions button:hover:not(:disabled) { border-color: var(--faint); color: var(--ink); }
  .move-actions button:disabled { opacity: 0.3; }
  .muted { padding: 12px; font-size: 12px; color: var(--muted); }
  .hint { font-size: 11px; color: var(--faint); }
  .esc { margin-left: auto; font-family: var(--font-mono); font-size: 11px; color: var(--faint); }
</style>
