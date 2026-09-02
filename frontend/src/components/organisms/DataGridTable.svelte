<script lang="ts">
  import { onDestroy, tick } from 'svelte';
  import InsertionHandle from '../atoms/InsertionHandle.svelte';
  import ColumnHeaderCell from '../molecules/ColumnHeaderCell.svelte';
  import type { ColumnInfo, FilterCondition, SortCondition } from '../../lib/types';

  type LabelPart = { text: string; match: boolean };
  type CellMove = 'up' | 'down' | 'left' | 'right';
  type Placement = 'before' | 'after';
  type SelectedCell = { row: number; column: string; expanded?: boolean };
  type EditingCell = { row: number; column: string; value: string; original: string };

  type Props = {
    columns: ColumnInfo[];
    bodyColumns: ColumnInfo[];
    rows: Record<string, unknown>[];
    caption: string;
    canQuery: boolean;
    canInsert: boolean;
    canEdit: boolean;
    sorts: SortCondition[];
    filters: FilterCondition[];
    columnLabelParts: (name: string) => LabelPart[];
    isColumnProtected: (name: string) => boolean;
    onSort: (column: ColumnInfo) => void;
    onFilter: (column: ColumnInfo, trigger?: HTMLButtonElement) => void;
    onProfile: (column: ColumnInfo, trigger?: HTMLButtonElement) => void;
    onHide: (name: string) => void;
    display: (value: unknown) => string;
    cellTitle: (column: ColumnInfo, value: unknown) => string;
    selectedCell: SelectedCell | null;
    editingCell: EditingCell | null;
    editSaving: boolean;
    onSelectCell: (event: MouseEvent, row: number, column: string) => void;
    onExpandCell: (event: MouseEvent, row: number, column: string) => void;
    onFilterCategoricalCell: (column: ColumnInfo, value: unknown) => void;
    onCellKeydown: (event: KeyboardEvent, row: number, column: string) => void;
    onCollapseCell: (row: number, column: string) => void;
    onEditValue: (value: string) => void;
    onCommitEdit: (move?: CellMove) => void;
    onCancelEdit: () => void;
    aggregateRowTones: boolean[];
    setTableScroll: (el: HTMLDivElement | null) => void;
    onInsert: (left: ColumnInfo, right: ColumnInfo | null, trigger: HTMLButtonElement) => void;
    onModify: (column: ColumnInfo) => void;
    onDuplicate: (column: ColumnInfo) => void;
    onRename: (column: ColumnInfo) => void;
    renamingColumn: { original: string; value: string } | null;
    onStartRename: (column: ColumnInfo) => void;
    onRenameValue: (value: string) => void;
    onCommitRename: () => void;
    onCancelRename: () => void;
    onBeginReorder: () => void;
    onPreviewReorder: (dragged: string, target: string, placement: Placement) => void;
    onCommitReorder: () => void;
    onCancelReorder: () => void;
  };

  let {
    columns, bodyColumns, rows, caption, canQuery, canInsert, canEdit, sorts, filters, columnLabelParts, isColumnProtected,
    onSort, onFilter, onProfile, onHide, display, cellTitle,
    selectedCell, editingCell, editSaving, onSelectCell, onExpandCell, onFilterCategoricalCell, onCellKeydown, onCollapseCell,
    onEditValue, onCommitEdit, onCancelEdit, aggregateRowTones, setTableScroll, onInsert, onModify, onDuplicate, onRename,
    renamingColumn, onStartRename, onRenameValue, onCommitRename, onCancelRename,
    onBeginReorder, onPreviewReorder, onCommitReorder, onCancelReorder
  }: Props = $props();

  function sortFor(name: string): SortCondition | undefined { return sorts.find((sort) => sort.column === name); }

  let contextMenu = $state<{ column: ColumnInfo; x: number; y: number } | null>(null);
  let contextMenuElement = $state<HTMLDivElement | null>(null);
  let scrollElement: HTMLDivElement | null = null;
  let draggedName = $state<string | null>(null);
  let dropTarget = $state<{ name: string; placement: Placement } | null>(null);
  let lastPlacement = '';
  let lastClientX = 0;
  let edgeFrame = 0;
  let animations: Animation[] = [];

  async function openContextMenu(event: MouseEvent, column: ColumnInfo) {
    event.preventDefault();
    contextMenu = { column, x: event.clientX, y: event.clientY };
    await tick();
    if (!contextMenu || !contextMenuElement) return;
    const rect = contextMenuElement.getBoundingClientRect();
    contextMenu = { column, x: Math.max(8, Math.min(event.clientX, innerWidth - rect.width - 8)), y: Math.max(8, Math.min(event.clientY, innerHeight - rect.height - 8)) };
    contextMenuElement.querySelector<HTMLButtonElement>('button:not(:disabled)')?.focus();
  }
  function runContextAction(action: () => void) { action(); contextMenu = null; }
  function closeContextMenuOnClick(event: MouseEvent) { if (contextMenu && !contextMenuElement?.contains(event.target as Node)) contextMenu = null; }
  function closeContextMenuOnKeydown(event: KeyboardEvent) {
    if (event.key !== 'Escape') return;
    contextMenu = null;
    if (draggedName) {
      event.preventDefault();
      onCancelReorder();
      clearDrag();
    }
  }

  function scrollHost(node: HTMLDivElement) {
    scrollElement = node;
    setTableScroll(node);
    return { destroy: () => { scrollElement = null; setTableScroll(null); } };
  }

  function headerPositions(): Map<string, number> {
    return new Map([...(scrollElement?.querySelectorAll<HTMLTableCellElement>('th[data-column]') ?? [])].map((header) => [header.dataset.column!, header.getBoundingClientRect().left]));
  }

  function cancelAnimations() {
    for (const animation of animations) animation.cancel();
    animations = [];
  }

  function animateHeaders(before: Map<string, number>) {
    cancelAnimations();
    if (matchMedia('(prefers-reduced-motion: reduce)').matches) return;
    for (const header of scrollElement?.querySelectorAll<HTMLTableCellElement>('th[data-column]') ?? []) {
      const previous = before.get(header.dataset.column!);
      if (previous === undefined) continue;
      const delta = previous - header.getBoundingClientRect().left;
      if (Math.abs(delta) < 0.5) continue;
      animations.push(header.animate([{ transform: `translateX(${delta}px)` }, { transform: 'translateX(0)' }], { duration: 160, easing: 'ease-out' }));
    }
  }

  function startEdgeScroll() {
    if (!edgeFrame) edgeFrame = requestAnimationFrame(edgeScroll);
  }

  function edgeScroll() {
    edgeFrame = 0;
    if (!draggedName || !scrollElement) return;
    const rect = scrollElement.getBoundingClientRect();
    const edge = 48;
    const speed = lastClientX < rect.left + edge
      ? -18 * Math.min(1, (rect.left + edge - lastClientX) / edge)
      : lastClientX > rect.right - edge
        ? 18 * Math.min(1, (lastClientX - rect.right + edge) / edge)
        : 0;
    if (speed) scrollElement.scrollLeft += speed;
    edgeFrame = requestAnimationFrame(edgeScroll);
  }

  function stopEdgeScroll() {
    if (edgeFrame) cancelAnimationFrame(edgeFrame);
    edgeFrame = 0;
  }

  function startHeaderDrag(event: DragEvent, name: string) {
    if (!canEdit || !event.dataTransfer) { event.preventDefault(); return; }
    draggedName = name;
    dropTarget = null;
    lastPlacement = '';
    lastClientX = event.clientX;
    event.dataTransfer.effectAllowed = 'move';
    event.dataTransfer.setData('text/plain', name);
    onBeginReorder();
    startEdgeScroll();
  }

  async function previewHeader(event: DragEvent, target: string) {
    if (!draggedName || draggedName === target) return;
    event.preventDefault();
    if (event.dataTransfer) event.dataTransfer.dropEffect = 'move';
    lastClientX = event.clientX;
    const rect = (event.currentTarget as HTMLTableCellElement).getBoundingClientRect();
    const placement: Placement = event.clientX < rect.left + rect.width / 2 ? 'before' : 'after';
    const key = `${target}:${placement}`;
    dropTarget = { name: target, placement };
    if (key === lastPlacement) return;
    lastPlacement = key;
    const before = headerPositions();
    onPreviewReorder(draggedName, target, placement);
    await tick();
    if (draggedName) animateHeaders(before);
  }

  function tableDragOver(event: DragEvent) {
    if (!draggedName) return;
    event.preventDefault();
    lastClientX = event.clientX;
    startEdgeScroll();
  }

  function dropHeader(event: DragEvent) {
    if (!draggedName) return;
    event.preventDefault();
    onCommitReorder();
    clearDrag();
  }

  function endHeaderDrag() {
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

  onDestroy(() => {
    if (draggedName) onCancelReorder();
    stopEdgeScroll();
    cancelAnimations();
  });

  function focusEditor(node: HTMLInputElement) { queueMicrotask(() => { node.focus(); node.setSelectionRange(node.value.length, node.value.length); }); }
  function editorKeydown(event: KeyboardEvent) {
    event.stopPropagation();
    if (event.key === 'Escape') { event.preventDefault(); onCancelEdit(); return; }
    if (event.key === 'Enter') { event.preventDefault(); onCommitEdit(); return; }
    const moves: Record<string, CellMove> = { ArrowUp: 'up', ArrowDown: 'down', ArrowLeft: 'left', ArrowRight: 'right' };
    if (moves[event.key]) { event.preventDefault(); onCommitEdit(moves[event.key]); }
  }
</script>

<svelte:window onclick={closeContextMenuOnClick} onkeydown={closeContextMenuOnKeydown} />

<!-- svelte-ignore a11y_no_noninteractive_tabindex -->
<div use:scrollHost class="table-scroll" role="region" tabindex="0" aria-label="Scrollable View table" ondragover={tableDragOver} ondrop={dropHeader}>
  <table>
    <caption class="sr-only">{caption}</caption>
    <thead>
      <tr>
        {#each columns as column, columnIndex (column.name)}
          <ColumnHeaderCell
            {column}
            labelParts={columnLabelParts(column.name)}
            sort={sortFor(column.name)}
            filtered={filters.some((filter) => filter.column === column.name)}
            {canQuery}
            protectedColumn={isColumnProtected(column.name)}
            canHide={columns.length > 1}
            canReorder={canEdit}
            dragging={draggedName === column.name}
            dropPlacement={dropTarget?.name === column.name ? dropTarget.placement : null}
            renaming={renamingColumn?.original === column.name}
            renameValue={renamingColumn?.original === column.name ? renamingColumn.value : column.name}
            renameSaving={!canEdit}
            onsort={() => onSort(column)}
            onfilter={(trigger) => onFilter(column, trigger)}
            onprofile={(trigger) => onProfile(column, trigger)}
            onhide={() => onHide(column.name)}
            onstartrename={() => onStartRename(column)}
            onrenamevalue={onRenameValue}
            oncommitrename={onCommitRename}
            oncancelrename={onCancelRename}
            oncontextmenu={(event) => openContextMenu(event, column)}
            ondragstart={(event) => startHeaderDrag(event, column.name)}
            ondragover={(event) => previewHeader(event, column.name)}
            ondragend={endHeaderDrag}
          />
          <th class="insertion-slot" scope="col"><InsertionHandle left={column.name} right={columns[columnIndex + 1]?.name ?? null} disabled={!canInsert} onclick={(event) => onInsert(column, columns[columnIndex + 1] ?? null, event.currentTarget as HTMLButtonElement)} /></th>
        {/each}
      </tr>
    </thead>
    <tbody>
      {#each rows as row, index (row)}
        <tr class:aggregate-row={aggregateRowTones.length > 0} class:aggregate-row-alt={aggregateRowTones[index]}>
          {#each bodyColumns as column (column.name)}
            {@const selected = selectedCell?.row === index && selectedCell.column === column.name}
            {@const expanded = selected && selectedCell?.expanded}
            {@const editing = editingCell?.row === index && editingCell.column === column.name}
            <td
              tabindex="0"
              data-row={index}
              data-column={column.name}
              class:selected-cell={selected}
              class:expanded-cell={expanded}
              class:editing-cell={editing}
              onclick={(event) => { onSelectCell(event, index, column.name); onExpandCell(event, index, column.name); }}
              ondblclick={() => { if (!editing) onFilterCategoricalCell(column, row[column.name]); }}
              onkeydown={(event) => onCellKeydown(event, index, column.name)}
              title={expanded || editing ? undefined : cellTitle(column, row[column.name])}
            >
              {#if editing}
                <input
                  use:focusEditor
                  type="text"
                  value={editingCell.value}
                  disabled={editSaving || !canEdit}
                  aria-label={`Edit row ${index + 1}, ${column.name}`}
                  oninput={(event) => onEditValue(event.currentTarget.value)}
                  onkeydown={editorKeydown}
                  onblur={() => onCommitEdit()}
                  onclick={(event) => event.stopPropagation()}
                />
              {:else}
                <span>{display(row[column.name])}</span>
                {#if expanded}
                  <button class="collapse" onclick={(event) => { event.stopPropagation(); onCollapseCell(index, column.name); }} aria-label={`Collapse ${column.name}`}>Collapse</button>
                {/if}
              {/if}
            </td>
            <td class="insertion-gap" aria-hidden="true"></td>
          {/each}
        </tr>
      {/each}
    </tbody>
  </table>
</div>

{#if contextMenu}
  <div bind:this={contextMenuElement} class="context-menu" role="menu" aria-label={`Actions for ${contextMenu.column.name}`} style:left={`${contextMenu.x}px`} style:top={`${contextMenu.y}px`}>
    <strong title={contextMenu.column.name}>{contextMenu.column.name}</strong>
    {#if canQuery}
      <button role="menuitem" onclick={() => runContextAction(() => onSort(contextMenu!.column))}>Sort</button>
      <button role="menuitem" onclick={() => runContextAction(() => onFilter(contextMenu!.column))}>Filter</button>
      {#if contextMenu.column.profile_kind}<button role="menuitem" onclick={() => runContextAction(() => onProfile(contextMenu!.column))}>Profile</button>{/if}
    {/if}
    <button role="menuitem" disabled={!canInsert} onclick={() => runContextAction(() => onModify(contextMenu!.column))}>Modify</button>
    <button role="menuitem" disabled={!canInsert} onclick={() => runContextAction(() => onDuplicate(contextMenu!.column))}>Duplicate</button>
    <button role="menuitem" disabled={!canInsert} onclick={() => runContextAction(() => onRename(contextMenu!.column))}>Rename</button>
    <button role="menuitem" disabled={isColumnProtected(contextMenu.column.name) || columns.length <= 1} onclick={() => runContextAction(() => onHide(contextMenu!.column.name))}>Hide</button>
  </div>
{/if}

<style>
  .table-scroll { width: 100%; height: 100%; overflow: auto; }
  table { min-width: 100%; border-collapse: separate; border-spacing: 0; font: 12px var(--font-mono); }
  .insertion-slot { position: sticky; top: 0; z-index: 4; width: 2px; min-width: 2px; height: 48px; padding: 0; border: 0; border-bottom: 1px solid var(--line); background: var(--surface-3); }
  td.insertion-gap { width: 2px; min-width: 2px; padding: 0; border-right: 0; background: color-mix(in srgb, var(--success) 4%, var(--surface)); }
  td {
    height: var(--row-height, 34px);
    max-width: 320px;
    padding: 0 10px;
    overflow: hidden;
    border-right: 1px solid var(--line-soft-2);
    border-bottom: 1px solid var(--line-soft-2);
    text-align: left;
    white-space: nowrap;
    text-overflow: ellipsis;
    background: var(--surface);
  }
  tbody tr:nth-child(even) td { background: var(--surface-inset); }
  tbody tr:hover td { background: var(--action-tint); }
  tbody tr.aggregate-row td { background: var(--surface); }
  tbody tr.aggregate-row.aggregate-row-alt td { background: var(--surface-inset); }
  td.expanded-cell { white-space: normal; overflow: visible; position: relative; z-index: 2; box-shadow: var(--shadow-popover); }
  td.selected-cell { position: relative; z-index: 1; box-shadow: inset 0 0 0 2px var(--action); }
  td.editing-cell { padding: 0; overflow: visible; }
  td.editing-cell input { width: 100%; min-width: 120px; height: 100%; padding: 0 9px; border: 0; outline: 2px solid var(--action); outline-offset: -2px; background: var(--surface); color: var(--ink); font: inherit; }
  td.editing-cell input:disabled { opacity: 0.7; }
  .collapse { display: block; margin-top: 4px; font-size: 10.5px; color: var(--action); background: none; border: none; }
  .context-menu { position: fixed; z-index: 20; width: 176px; display: grid; padding: 4px; border: 1px solid var(--line-strong); border-radius: var(--radius-md); background: var(--surface); box-shadow: var(--shadow-popover); }
  .context-menu strong { min-width: 0; padding: 8px 9px; overflow: hidden; border-bottom: 1px solid var(--line); font-size: 12px; text-overflow: ellipsis; white-space: nowrap; }
  .context-menu button { min-height: 36px; padding: 0 9px; border: 0; border-radius: var(--radius-sm); background: transparent; color: var(--ink); text-align: left; }
  .context-menu button:not(:disabled):hover, .context-menu button:not(:disabled):focus-visible { background: var(--surface-hover); }
</style>
