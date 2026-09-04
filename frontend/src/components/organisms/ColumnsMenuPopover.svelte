<script lang="ts">
  import { onDestroy, tick } from 'svelte';
  import MenuPopover from '../molecules/MenuPopover.svelte';
  import IconButton from '../atoms/IconButton.svelte';
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
    columnMenuRegex: boolean;
    setColumnMenuRegex: (value: boolean) => void;
    columnMenuRegexError: string;
    columnTypes: string[];
    columnTypeCounts: Record<string, number>;
    isTypeShown: (type: string) => boolean;
    toggleShownType: (type: string, checked: boolean) => void;
    typeToggleDisabled: (type: string) => boolean;
    nullThreshold: number;
    setNullThreshold: (value: number) => void;
    onApplyThreshold: () => void;
    onHideAll: () => void;
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
    columnMenuRegex, setColumnMenuRegex, columnMenuRegexError,
    columnTypes, columnTypeCounts, isTypeShown, toggleShownType, typeToggleDisabled,
    nullThreshold, setNullThreshold, onApplyThreshold, onHideAll, onShowAll, hiddenCount,
    columnMenuItems, hiddenColumns, isColumnProtected, visibleColumnsLength, onToggleColumn,
    orderedColumnNames, onBeginReorder, onPreviewReorder, onCommitReorder, onCancelReorder,
    onMoveColumn, onRegexVisibility
  }: Props = $props();

  let matchesVisible = $state(true);
  let listElement: HTMLDivElement | null = null;
  let draggedName = $state<string | null>(null);
  let dropTarget = $state<{ name: string; placement: Placement } | null>(null);
  let lastPlacement = '';
  let lastClientY = 0;
  let edgeFrame = 0;
  let animations: Animation[] = [];
  let nullScrubPointer: number | null = null;
  let nullScrubStartX = 0;
  let nullScrubStartY = 0;
  let nullScrubStartValue = 0;

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

  function updateSearch(value: string) {
    setColumnMenuSearch(value);
    if (columnMenuRegex) matchesVisible = true;
  }

  function toggleSearchMode() {
    setColumnMenuRegex(!columnMenuRegex);
    matchesVisible = true;
  }

  function applyRegex() {
    if (!columnMenuSearch.trim() || columnMenuRegexError) return;
    if (!onRegexVisibility(columnMenuSearch, false, matchesVisible ? 'hide' : 'show')) matchesVisible = !matchesVisible;
  }

  function startNullScrub(event: PointerEvent) {
    if (event.button !== 0) return;
    event.preventDefault();
    nullScrubPointer = event.pointerId;
    nullScrubStartX = event.clientX;
    nullScrubStartY = event.clientY;
    nullScrubStartValue = nullThreshold;
    (event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
  }

  function scrubNullThreshold(event: PointerEvent) {
    if (event.pointerId !== nullScrubPointer) return;
    const delta = Math.round(event.clientX - nullScrubStartX - (event.clientY - nullScrubStartY));
    setNullThreshold(Math.min(100, Math.max(0, nullScrubStartValue + delta)));
  }

  function stopNullScrub(event: PointerEvent) {
    if (event.pointerId === nullScrubPointer) nullScrubPointer = null;
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
      <label for="column-menu-search" class="sr-only">{columnMenuRegex ? 'Search columns by regular expression' : 'Search columns'}</label>
      <TextInput
        id="column-menu-search" type="search" glyph="⌕" mono={columnMenuRegex} value={columnMenuSearch}
        oninput={(event: Event) => updateSearch((event.currentTarget as HTMLInputElement).value)}
        placeholder={columnMenuRegex ? 'Match columns with regex' : 'Search columns'}
        aria-invalid={columnMenuRegex && !!columnMenuRegexError}
        aria-describedby={columnMenuRegexError ? 'column-regex-error' : undefined}
      >
        {#snippet trailing()}
          {#if columnMenuRegex}
            <IconButton
              type="button" icon={matchesVisible ? 'eye' : 'eye-off'}
              label={`${matchesVisible ? 'Hide' : 'Show'} matching columns`}
              data-tip-align="end"
              onclick={applyRegex} disabled={!columnMenuSearch.trim() || !!columnMenuRegexError}
            />
          {/if}
          <IconButton
            type="button" glyph=".*" active={columnMenuRegex}
            label={columnMenuRegex ? 'Use plain text search' : 'Use regular expression'}
            data-tip-align="end"
            aria-pressed={columnMenuRegex} onclick={toggleSearchMode}
          />
        {/snippet}
      </TextInput>
      {#if columnMenuRegexError}<p id="column-regex-error" class="regex-error" role="alert">{columnMenuRegexError}</p>{/if}
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
    <div class="threshold-row" role="group" aria-label="Hide columns by null percentage">
      <div class="null-control" data-tip="Drag the label right or up to increase; click the value to type" data-tip-position="top" data-tip-align="start">
        <label
          for="null-threshold" class="scrub-label"
          onpointerdown={startNullScrub} onpointermove={scrubNullThreshold}
          onpointerup={stopNullScrub} onpointercancel={stopNullScrub}
        >Nulls</label>
        <label class="threshold">≥
          <input id="null-threshold" type="number" min="0" max="100" step="1" value={nullThreshold} oninput={(event) => setNullThreshold(Number((event.currentTarget as HTMLInputElement).value))} aria-label="Null percentage threshold" />
          %
        </label>
      </div>
      <IconButton type="button" glyph="×" label={`Hide columns at least ${nullThreshold}% null`} data-tip-position="top" onclick={onApplyThreshold} />
      <div class="visibility-actions" role="group" aria-label="All column visibility">
        <IconButton type="button" icon="eye-off" label="Hide all columns" data-tip-position="top" onclick={onHideAll} disabled={visibleCount === 0} />
        <IconButton type="button" icon="eye" label="Show all columns" data-tip-position="top" data-tip-align="end" onclick={onShowAll} disabled={hiddenCount === 0} />
      </div>
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
  .search :global(.field) { display: flex; width: 100%; }
  .regex-error { margin: 0; font-size: 11px; color: var(--error); }
  .type-row { display: flex; flex-wrap: wrap; gap: 5px; padding: 9px 11px 0; }
  .threshold-row { display: flex; flex-wrap: wrap; align-items: center; gap: 6px; padding: 9px 11px 11px; border-bottom: 1px solid var(--line-soft); }
  .null-control { display: inline-flex; align-items: center; gap: 5px; height: 26px; padding: 0 8px; border: 1px solid var(--control-border); border-radius: var(--radius-md); color: var(--muted); font-size: 11px; }
  .scrub-label { cursor: ew-resize; user-select: none; touch-action: none; }
  .threshold { display: inline-flex; align-items: center; gap: 3px; color: var(--ink-2); }
  .threshold input { width: 30px; height: 20px; padding: 0; text-align: center; border: 0; border-radius: var(--radius-sm); background: var(--surface-2); color: var(--ink); font-family: var(--font-mono); font-size: 10.5px; }
  .threshold input:focus-visible { outline: none; box-shadow: inset 0 -1px var(--ink); }
  .threshold input { appearance: textfield; }
  .threshold input::-webkit-inner-spin-button, .threshold input::-webkit-outer-spin-button { appearance: none; margin: 0; }
  .visibility-actions { display: flex; align-items: center; gap: 2px; margin-left: auto; }
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
