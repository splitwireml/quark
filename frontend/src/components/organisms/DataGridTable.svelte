<script lang="ts">
  import { tick } from 'svelte';
  import InsertionHandle from '../atoms/InsertionHandle.svelte';
  import ColumnHeaderCell from '../molecules/ColumnHeaderCell.svelte';
  import type { ColumnInfo, FilterCondition, SortCondition } from '../../lib/types';

  type LabelPart = { text: string; match: boolean };
  type CellMove = 'up' | 'down' | 'left' | 'right';
  type SelectedCell = { row: number; column: string; expanded?: boolean };
  type EditingCell = { row: number; column: string; value: string; original: string };

  type Props = {
    columns: ColumnInfo[];
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
  };

  let {
    columns, rows, caption, canQuery, canInsert, canEdit, sorts, filters, columnLabelParts, isColumnProtected,
    onSort, onFilter, onProfile, onHide, display, cellTitle,
    selectedCell, editingCell, editSaving, onSelectCell, onExpandCell, onFilterCategoricalCell, onCellKeydown, onCollapseCell,
    onEditValue, onCommitEdit, onCancelEdit, aggregateRowTones, setTableScroll, onInsert
  }: Props = $props();

  function sortFor(name: string): SortCondition | undefined { return sorts.find((sort) => sort.column === name); }

  let contextMenu = $state<{ column: ColumnInfo; x: number; y: number } | null>(null);
  let contextMenuElement = $state<HTMLDivElement | null>(null);

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
  function closeContextMenuOnKeydown(event: KeyboardEvent) { if (event.key === 'Escape') contextMenu = null; }

  function scrollHost(node: HTMLDivElement) {
    setTableScroll(node);
    return { destroy: () => setTableScroll(null) };
  }

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
<div use:scrollHost class="table-scroll" role="region" tabindex="0" aria-label="Scrollable dataset table">
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
            onsort={() => onSort(column)}
            onfilter={(trigger) => onFilter(column, trigger)}
            onprofile={(trigger) => onProfile(column, trigger)}
            onhide={() => onHide(column.name)}
            oncontextmenu={(event) => openContextMenu(event, column)}
          />
          <th class="insertion-slot" scope="col"><InsertionHandle left={column.name} right={columns[columnIndex + 1]?.name ?? null} disabled={!canInsert} onclick={(event) => onInsert(column, columns[columnIndex + 1] ?? null, event.currentTarget as HTMLButtonElement)} /></th>
        {/each}
      </tr>
    </thead>
    <tbody>
      {#each rows as row, index (index)}
        <tr class:aggregate-row={aggregateRowTones.length > 0} class:aggregate-row-alt={aggregateRowTones[index]}>
          {#each columns as column (column.name)}
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
