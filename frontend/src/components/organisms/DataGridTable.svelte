<script lang="ts">
  import type { TableRows } from '../../lib/table-rows';
  import { onDestroy, tick } from 'svelte';
  import Icon from '../atoms/Icon.svelte';
  import RowScrollbar from '../atoms/RowScrollbar.svelte';
  import { scrollRowStep, smoothVelocity } from '../../lib/scroll-prefetch';
  import InsertionHandle from '../atoms/InsertionHandle.svelte';
  import ColumnHeaderCell from '../molecules/ColumnHeaderCell.svelte';
  import { rangeToHtml, rangeToText } from '../../lib/clipboard';
  import type { ColumnInfo, FilterCondition, SortCondition } from '../../lib/types';

  type LabelPart = { text: string; match: boolean };
  type CellMove = 'up' | 'down' | 'left' | 'right';
  type Placement = 'before' | 'after';
  type SelectedCell = { row: number; column: string; expanded?: boolean };
  type EditingCell = { row: number; column: string; value: string; original: string };

  type Props = {
    columns: ColumnInfo[];
    bodyColumns: ColumnInfo[];
    rows: TableRows;
    rowOffset: number;
    pinnedColumns: string[];
    onTogglePin: (column: ColumnInfo) => void;
    caption: string;
    rowDensity: string;
    fitColumnsToContent: boolean;
    canQuery: boolean;
    canInsert: boolean;
    canEdit: boolean;
    sorts: SortCondition[];
    filters: FilterCondition[];
    columnLabelParts: (name: string) => LabelPart[];
    isColumnProtected: (name: string) => boolean;
    generatedColumns: Record<string, string>;
    canPasteColumn: boolean;
    onCopyColumn: (column: ColumnInfo) => void;
    onPasteColumn: (column: ColumnInfo) => void;
    onSort: (column: ColumnInfo) => void;
    onFilter: (column: ColumnInfo, trigger?: HTMLButtonElement) => void;
    onProfile: (column: ColumnInfo, trigger?: HTMLButtonElement) => void;
    onHide: (name: string) => void;
    display: (value: unknown) => string;
    cellTitle: (column: ColumnInfo, value: unknown) => string;
    selectedColumn: string;
    onSelectColumn: (name: string) => void;
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
    totalRows: number;
    totalLabel: string;
    pageSize: number;
    seekDisabled: boolean;
    onSeekRow: (row: number) => void;
    // One cached non-current page and one transient snapshot window, positioned below.
    neighbor: { page: number; rows: TableRows } | null;
    ahead: { page: number; rows: TableRows }[];
    snapshot: { start: number; rows: TableRows } | null;
    onSnapshotRest: (row: number) => void;
    onThumbHeld: (held: boolean) => void;
    onThumbMove: () => void;
    onViewportNeed: (report: { firstRow: number; lastRow: number; velocity: number }) => void;
    // Clicks on cached/snapshot rows settle that position instead of selecting.
    onPreviewSelect: (absoluteRow: number, column: string) => void;
  };

  let {
    columns, bodyColumns, rows, rowOffset, pinnedColumns, onTogglePin, caption, rowDensity, fitColumnsToContent, canQuery, canInsert, canEdit, sorts, filters, columnLabelParts, isColumnProtected,
    generatedColumns, canPasteColumn, onCopyColumn, onPasteColumn,
    onSort, onFilter, onProfile, onHide, display, cellTitle,
    selectedColumn, onSelectColumn, selectedCell, editingCell, editSaving, onSelectCell, onExpandCell, onFilterCategoricalCell, onCellKeydown, onCollapseCell,
    onEditValue, onCommitEdit, onCancelEdit, aggregateRowTones, setTableScroll, onInsert, onModify, onDuplicate, onRename,
    renamingColumn, onStartRename, onRenameValue, onCommitRename, onCancelRename,
    onBeginReorder, onPreviewReorder, onCommitReorder, onCancelReorder,
    totalRows, totalLabel, pageSize, seekDisabled, onSeekRow,
    neighbor, ahead, snapshot, onSnapshotRest, onThumbHeld, onThumbMove, onViewportNeed, onPreviewSelect
  }: Props = $props();

  function sortFor(name: string): SortCondition | undefined { return sorts.find((sort) => sort.column === name); }
  // Rank is only worth showing once the sort is actually ordered across columns.
  function sortRankFor(name: string): number { return sorts.length > 1 ? sorts.findIndex((sort) => sort.column === name) + 1 : 0; }

  // The canvas spans the whole dataset, not just the loaded page: only rows near
  // the viewport render, positioned by absolute spacers. Loaded segments (current
  // page, previous page, lookahead, snapshot) fill their rows; missing rows keep
  // the last page moving until the controller backfills them. Rows are
  // a uniform height per density, so the window is pure arithmetic.
  const OVERSCAN = 8;
  // Matches the fixed column-header height, including its profile strip.
  const HEADER_HEIGHT = 48;
  let scrollTop = $state(0);
  let viewportHeight = $state(0);
  // Keep in step with the `--row-height` fallback on td below.
  const DEFAULT_ROW_HEIGHT = 34;
  const ROW_HEIGHTS: Record<string, number> = { compact: 26, default: DEFAULT_ROW_HEIGHT, comfortable: 42 };
  const rowHeight = $derived(ROW_HEIGHTS[rowDensity] ?? DEFAULT_ROW_HEIGHT);
  let velocity = $state(0);

  const bodyViewportHeight = $derived(Math.max(0, viewportHeight - HEADER_HEIGHT));
  const visibleRowCount = $derived(bodyViewportHeight / rowHeight);
  const pxPerRow = $derived(scrollRowStep(totalRows, rowHeight, bodyViewportHeight));
  const firstVisibleRow = $derived(Math.min(Math.max(0, totalRows - visibleRowCount), scrollTop / pxPerRow));
  const lastVisibleRow = $derived(Math.min(totalRows - 1, firstVisibleRow + visibleRowCount));

  type Segment = { kind: 'current' | 'neighbor' | 'snapshot' | 'retained'; start: number; rows: TableRows };
  const size = $derived(Math.max(1, Math.floor(pageSize)));
  const segments = $derived<Segment[]>([
    { kind: 'current', start: rowOffset, rows },
    ...(neighbor && neighbor.rows.length ? [{ kind: 'neighbor' as const, start: (neighbor.page - 1) * size, rows: neighbor.rows }] : []),
    ...ahead.map((entry) => ({ kind: 'neighbor' as const, start: (entry.page - 1) * size, rows: entry.rows })),
    ...(snapshot && snapshot.rows.length ? [{ kind: 'snapshot' as const, start: snapshot.start, rows: snapshot.rows }] : []),
  ]);
  function segmentAt(absolute: number): Segment | undefined {
    return segments.find((segment) => absolute >= segment.start && absolute < segment.start + segment.rows.length);
  }

  // Segment changes update cells in place; absolute row keys survive scrolling and cache promotion.
  type RenderRow = { abs: number; index: number; rows: TableRows; kind: Segment['kind'] };
  function windowFirst(): number { return Math.max(0, Math.floor(firstVisibleRow) - OVERSCAN); }
  const renderRows = $derived.by<RenderRow[]>(() => {
    const first = windowFirst();
    const last = Math.min(totalRows, Math.ceil(firstVisibleRow + visibleRowCount) + OVERSCAN);
    const list: RenderRow[] = [];
    for (let cursor = first; cursor < last; cursor += 1) {
      const segment = segmentAt(cursor);
      const kind = segment?.kind ?? 'retained';
      // Keep the previous data moving under the pointer while a destination is
      // pending. Retained values are non-interactive and hidden from AT.
      const source = segment?.rows ?? rows;
      const index = segment ? cursor - segment.start : cursor % Math.max(1, rows.length);
      if (!source.length) continue;
      list.push({ abs: cursor, index, rows: source, kind });
    }
    return list;
  });
  const topPad = $derived(Math.max(0, scrollTop - (firstVisibleRow - windowFirst()) * rowHeight));
  const bottomPad = $derived(Math.max(0, Math.min(10_000_000, totalRows * rowHeight) - topPad - renderRows.length * rowHeight));

  // Roving tabindex: the grid is one tab stop, not one per cell. Arrow keys move within it.
  const activeColumn = $derived(
    bodyColumns.some((column) => column.name === selectedCell?.column) ? selectedCell!.column : bodyColumns[0]?.name,
  );
  const activeRow = $derived(selectedCell?.row ?? -1);
  const bodyCellCount = $derived(bodyColumns.length * 2);
  const lastColumn = $derived(columns[columns.length - 1]);


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
    if (event.shiftKey && event.button === 2 && canInsert && generatedColumns[column.name] !== undefined) {
      onModify(column);
      return;
    }
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

  // Programmatic API for the parent (via bind:this): absolute dataset rows.
  export function scrollToAbsoluteRow(absolute: number, onlyIfOutside = false) {
    if (!scrollElement) return;
    if (onlyIfOutside && absolute >= firstVisibleRow && absolute < lastVisibleRow - 1) return;
    scrollElement.scrollTop = Math.max(0, absolute * pxPerRow);
    scrollTop = scrollElement.scrollTop;
  }

  let lastScrollTop = 0;
  let lastScrollTime = 0;
  let reportFrame = 0;
  let restTimer = 0;
  const REST_MS = 40;

  function emitViewport() {
    onViewportNeed({ firstRow: firstVisibleRow, lastRow: lastVisibleRow, velocity });
  }
  function clearViewportTimers() {
    if (reportFrame) cancelAnimationFrame(reportFrame);
    if (restTimer) clearTimeout(restTimer);
    reportFrame = 0;
    restTimer = 0;
  }

  function scrollHost(node: HTMLDivElement) {
    scrollElement = node;
    setTableScroll(node);
    viewportHeight = node.clientHeight;
    lastScrollTop = node.scrollTop;
    lastScrollTime = 0;
    const onScroll = () => {
      const now = performance.now();
      scrollTop = node.scrollTop;
      const elapsed = lastScrollTime > 0 ? Math.max(1, now - lastScrollTime) : 0;
      const instant = ((node.scrollTop - lastScrollTop) / pxPerRow / (elapsed || 16)) * 1000;
      velocity = smoothVelocity(velocity, instant);
      lastScrollTop = node.scrollTop;
      lastScrollTime = now;
      if (restTimer) clearTimeout(restTimer);
      restTimer = window.setTimeout(() => { restTimer = 0; velocity = 0; emitViewport(); }, REST_MS);
      // Coalesce pointer/wheel events into the next frame instead of waiting for a pause.
      if (!reportFrame) reportFrame = requestAnimationFrame(() => { reportFrame = 0; emitViewport(); });
    };
    // The compressed canvas changes scrollbar travel, never wheel/trackpad speed.
    const onWheel = (event: WheelEvent) => {
      if (pxPerRow >= rowHeight || event.ctrlKey || event.shiftKey || !event.deltaY) return;
      event.preventDefault();
      const unit = event.deltaMode === 1 ? rowHeight : event.deltaMode === 2 ? node.clientHeight : 1;
      node.scrollTop += event.deltaY * unit * pxPerRow / rowHeight;
      node.scrollLeft += event.deltaX * unit;
    };
    node.addEventListener('wheel', onWheel, { passive: false });
    node.addEventListener('scroll', onScroll, { passive: true });
    const observer = new ResizeObserver(() => {
      viewportHeight = node.clientHeight;
      emitViewport();
    });
    observer.observe(node);
    requestAnimationFrame(() => emitViewport());
    return {
      destroy: () => {
        node.removeEventListener('wheel', onWheel);
        node.removeEventListener('scroll', onScroll);
        observer.disconnect();
        clearViewportTimers();
        scrollElement = null;
        setTableScroll(null);
      },
    };
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

  // Range selection. Columns are indexes into bodyColumns; row mode pins the range to full rows.
  type Point = { row: number; column: number };
  let anchor = $state<Point | null>(null);
  let head = $state<Point | null>(null);
  let rowMode = $state(false);
  let dragging = $state(false);
  let shiftHeld = $state(false);

  const range = $derived(
    anchor && head
      ? {
          top: Math.min(anchor.row, head.row),
          bottom: Math.max(anchor.row, head.row),
          left: rowMode ? 0 : Math.min(anchor.column, head.column),
          right: rowMode ? bodyColumns.length - 1 : Math.max(anchor.column, head.column),
        }
      : null,
  );

  // A new page or query replaces the rows the range pointed at.
  $effect(() => { void rows; anchor = null; head = null; rowMode = false; });

  function startSelect(event: PointerEvent, row: number, column: number) {
    if (event.button !== 0 || editingCell) return;
    if ((event.target as HTMLElement).closest('input, button')) return;
    rowMode = event.shiftKey;
    anchor = { row, column };
    head = { row, column };
    dragging = true;
  }
  function startRowSelect(event: PointerEvent, row: number) {
    if (event.button !== 0 || editingCell) return;
    rowMode = true;
    anchor = { row, column: 0 };
    head = { row, column: 0 };
    dragging = true;
  }
  function extendTo(row: number, column: number) { if (dragging) head = { row, column }; }
  function extendRow(row: number) { if (dragging && head) head = { row, column: head.column }; }

  function cellText(row: number, name: string): string {
    const value = rows.cell(row, name);
    return value == null ? '' : display(value);
  }
  function rangeMatrix(): string[][] | null {
    if (!range) return null;
    const names = bodyColumns.slice(range.left, range.right + 1).map((column) => column.name);
    const matrix: string[][] = [];
    for (let row = range.top; row <= range.bottom; row += 1) {
      if (row >= 0 && row < rows.length) matrix.push(names.map((name) => cellText(row, name)));
    }
    return matrix.length ? matrix : null;
  }

  // The grid keeps its selection in state, not in a DOM range, so the browser has nothing to
  // copy and never fires a copy event. The shortcut writes to the clipboard itself instead.
  async function copyKeydown(event: KeyboardEvent) {
    if (!(event.metaKey || event.ctrlKey) || editingCell || (event.target as HTMLElement).closest('input, textarea')) return;
    const selected = columns.find((column) => column.name === selectedColumn);
    if (event.key.toLowerCase() === 'v' && selected && canPasteColumn && canInsert) {
      event.preventDefault();
      onPasteColumn(selected);
      return;
    }
    if (event.key.toLowerCase() !== 'c') return;
    const matrix = rangeMatrix();
    if (!matrix) {
      if (selected) { event.preventDefault(); onCopyColumn(selected); }
      return;
    }
    event.preventDefault();
    const text = rangeToText(matrix);
    try {
      await navigator.clipboard.write([
        new ClipboardItem({
          'text/plain': new Blob([text], { type: 'text/plain' }),
          'text/html': new Blob([rangeToHtml(matrix)], { type: 'text/html' }),
        }),
      ]);
    } catch {
      // Older or stricter engines only accept plain text.
      await navigator.clipboard.writeText(text);
    }
  }

  // Pinned columns freeze against the left edge. Their offsets are measured rather than
  // guessed: column widths depend on the header's own content and the fit-to-content mode.
  const GUTTER_WIDTH = 12;
  // A frozen region wider than this stops being a reference and starts being the table.
  const PIN_LIMIT = 0.3;
  let pinLefts = $state(new Map<string, number>());
  let pinRefused = $state<string | null>(null);
  const lastPinned = $derived(pinnedColumns[pinnedColumns.length - 1] ?? null);

  function headerFor(name: string): HTMLTableCellElement | null {
    return scrollElement?.querySelector<HTMLTableCellElement>(`th[data-column="${CSS.escape(name)}"]`) ?? null;
  }
  // A column and the insertion slot that trails it freeze as one unit.
  function pinnedWidth(header: HTMLTableCellElement): number {
    return header.getBoundingClientRect().width + (header.nextElementSibling?.getBoundingClientRect().width ?? 0);
  }

  $effect(() => {
    void columns;
    void fitColumnsToContent;
    void rowDensity;
    const lefts = new Map<string, number>();
    let x = GUTTER_WIDTH;
    for (const name of pinnedColumns) {
      const header = headerFor(name);
      if (!header) continue;
      lefts.set(name, x);
      x += pinnedWidth(header);
    }
    pinLefts = lefts;
  });

  export function requestPin(column: ColumnInfo) {
    if (pinnedColumns.includes(column.name)) { onTogglePin(column); return; }
    const header = headerFor(column.name);
    const room = (scrollElement?.clientWidth ?? 0) * PIN_LIMIT - GUTTER_WIDTH;
    const used = pinnedColumns.reduce((total, name) => { const pinned = headerFor(name); return total + (pinned ? pinnedWidth(pinned) : 0); }, 0);
    if (!header || used + pinnedWidth(header) > room) {
      pinRefused = column.name;
      setTimeout(() => { if (pinRefused === column.name) pinRefused = null; }, 400);
      return;
    }
    onTogglePin(column);
  }

  function focusEditor(node: HTMLInputElement) { queueMicrotask(() => { node.focus(); node.setSelectionRange(node.value.length, node.value.length); }); }
  function positionTailAction(event: PointerEvent) {
    const button = event.currentTarget as HTMLButtonElement;
    const rect = button.getBoundingClientRect();
    button.style.setProperty('--tail-x', `${event.clientX - rect.left + 12}px`);
    button.style.setProperty('--tail-y', `${event.clientY - rect.top + 12}px`);
  }
  function editorKeydown(event: KeyboardEvent) {
    event.stopPropagation();
    if (event.key === 'Escape') { event.preventDefault(); onCancelEdit(); return; }
    if (event.key === 'Enter') { event.preventDefault(); onCommitEdit(); return; }
    const moves: Record<string, CellMove> = { ArrowUp: 'up', ArrowDown: 'down', ArrowLeft: 'left', ArrowRight: 'right' };
    if (moves[event.key]) { event.preventDefault(); onCommitEdit(moves[event.key]); }
  }
</script>

<svelte:window
  onclick={closeContextMenuOnClick}
  onkeydown={(event) => { shiftHeld = event.shiftKey; closeContextMenuOnKeydown(event); }}
  onkeyup={(event) => { shiftHeld = event.shiftKey; }}
  onblur={() => { shiftHeld = false; dragging = false; }}
  onpointerup={() => { dragging = false; }}
/>

<!-- svelte-ignore a11y_no_noninteractive_tabindex -->
<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<div class="table-wrap">
<div use:scrollHost id="data-grid-scroll" class="table-scroll" role="region" tabindex="0" aria-label="Scrollable View table" ondragover={tableDragOver} ondrop={dropHeader} onkeydown={copyKeydown}>
  <div class="table-canvas" class:fit-columns={fitColumnsToContent}>
    <table role="grid" aria-rowcount={totalRows + 1} aria-colcount={bodyColumns.length + 1} class:row-hover={shiftHeld || rowMode}>
    <caption class="sr-only">{caption}</caption>
    <thead>
      <tr aria-rowindex="1">
        <th class="gutter gutter-head" scope="col"><span class="sr-only">Row</span></th>
        {#each columns as column, columnIndex (column.name)}
          <ColumnHeaderCell
            {column}
            selected={column.name === selectedColumn}
            generated={generatedColumns[column.name] !== undefined}
            onselect={() => { anchor = null; head = null; onSelectColumn(column.name); }}
            {fitColumnsToContent}
            labelParts={columnLabelParts(column.name)}
            sort={sortFor(column.name)}
            sortRank={sortRankFor(column.name)}
            filtered={filters.some((filter) => filter.column === column.name)}
            {canQuery}
            protectedColumn={isColumnProtected(column.name)}
            canHide={columns.length > 1}
            canReorder={canEdit}
            dragging={draggedName === column.name}
            pinned={pinnedColumns.includes(column.name)}
            pinnedLeft={pinLefts.get(column.name) ?? null}
            pinRefused={pinRefused === column.name}
            dropPlacement={dropTarget?.name === column.name ? dropTarget.placement : null}
            renaming={renamingColumn?.original === column.name}
            renameValue={renamingColumn?.original === column.name ? renamingColumn.value : column.name}
            renameSaving={!canEdit}
            onsort={() => onSort(column)}
            onfilter={(trigger) => onFilter(column, trigger)}
            onprofile={(trigger) => onProfile(column, trigger)}
            onhide={() => onHide(column.name)}
            onpin={() => requestPin(column)}
            onstartrename={() => onStartRename(column)}
            onrenamevalue={onRenameValue}
            oncommitrename={onCommitRename}
            oncancelrename={onCancelRename}
            oncontextmenu={(event) => openContextMenu(event, column)}
            ondragstart={(event) => startHeaderDrag(event, column.name)}
            ondragover={(event) => previewHeader(event, column.name)}
            ondragend={endHeaderDrag}
          />
          <th class="insertion-slot" scope="col" class:pinned={pinLefts.has(column.name)} class:pin-edge={lastPinned === column.name} style:left={pinLefts.has(column.name) ? `${pinLefts.get(column.name)! + (headerFor(column.name)?.getBoundingClientRect().width ?? 0)}px` : undefined}><InsertionHandle left={column.name} right={columns[columnIndex + 1]?.name ?? null} disabled={!canInsert} onclick={(event) => onInsert(column, columns[columnIndex + 1] ?? null, event.currentTarget as HTMLButtonElement)} /></th>
        {/each}
      </tr>
    </thead>
    <tbody aria-busy={renderRows.some((row) => row.kind === 'retained')}>
      {#if topPad > 0}
        <tr class="spacer" aria-hidden="true"><td class="gutter"></td><td colspan={bodyCellCount} style:height={`${topPad}px`}></td></tr>
      {/if}
      {#each renderRows as item (item.abs)}
            {@const abs = item.abs}
            {@const pending = item.kind === 'retained'}
            {@const inCurrent = item.kind === 'current'}
            {@const index = abs - rowOffset}
            {@const rowActive = inCurrent && !!range && index >= range.top && index <= range.bottom}
        <tr aria-rowindex={pending ? undefined : abs + 2} aria-hidden={pending || undefined} class:pending class:striped={abs % 2 === 1} class:aggregate-row={aggregateRowTones.length > 0} class:aggregate-row-alt={inCurrent && aggregateRowTones[index]}>
          <th
            scope="row"
            class="gutter"
            class:row-active={rowActive}
            title="Select row"
            onpointerdown={(event) => { if (inCurrent) startRowSelect(event, index); else if (!pending && bodyColumns[0]) onPreviewSelect(abs, bodyColumns[0].name); }}
            onpointerenter={() => { if (inCurrent) extendRow(index); }}
          ><span class="sr-only">Row {abs + 1}</span></th>
          {#each bodyColumns as column, columnIndex (column.name)}
            {@const value = item.rows.cell(item.index, column.name)}
            {@const selected = inCurrent && selectedCell?.row === index && selectedCell.column === column.name}
            {@const expanded = selected && selectedCell?.expanded}
            {@const editing = inCurrent && editingCell?.row === index && editingCell.column === column.name}
            {@const inRange = !!range && rowActive && columnIndex >= range.left && columnIndex <= range.right}
            <td
              tabindex={inCurrent && index === activeRow && column.name === activeColumn ? 0 : -1}
              aria-selected={selected}
              data-row={inCurrent ? index : undefined}
              data-column={column.name}
              class:selected-cell={selected}
              class:expanded-cell={expanded}
              class:editing-cell={editing}
              class:in-range={inRange}
              class:generated={generatedColumns[column.name] !== undefined}
              class:pinned={pinLefts.has(column.name)}
              style:left={pinLefts.get(column.name) === undefined ? undefined : `${pinLefts.get(column.name)}px`}
              onpointerdown={(event) => { if (inCurrent) startSelect(event, index, columnIndex); }}
              onpointerenter={() => { if (inCurrent) extendTo(index, columnIndex); }}
              onclick={(event) => { if (inCurrent) { onSelectCell(event, index, column.name); onExpandCell(event, index, column.name); } else if (!pending) onPreviewSelect(abs, column.name); }}
              ondblclick={() => { if (inCurrent) { if (!editing) onFilterCategoricalCell(column, value); } else if (!pending) onPreviewSelect(abs, column.name); }}
              onkeydown={(event) => { if (inCurrent) onCellKeydown(event, index, column.name); }}
              title={expanded || editing ? undefined : cellTitle(column, value)}
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
                <span>{display(value)}</span>
                {#if expanded}
                  <button class="collapse" onclick={(event) => { event.stopPropagation(); onCollapseCell(index, column.name); }} aria-label={`Collapse ${column.name}`}>Collapse</button>
                {/if}
              {/if}
            </td>
            <td class="insertion-gap" aria-hidden="true" class:pinned={pinLefts.has(column.name)} class:pin-edge={lastPinned === column.name} style:left={pinLefts.has(column.name) ? `${pinLefts.get(column.name)! + (headerFor(column.name)?.getBoundingClientRect().width ?? 0)}px` : undefined}></td>
          {/each}
        </tr>
      {/each}
      {#if bottomPad > 0}
        <tr class="spacer" aria-hidden="true"><td class="gutter"></td><td colspan={bodyCellCount} style:height={`${bottomPad}px`}></td></tr>
      {/if}
    </tbody>
    </table>
    {#if fitColumnsToContent && lastColumn}
      <button
        type="button"
        class="add-column-tail"
        disabled={!canInsert}
        aria-label={`Add column after ${lastColumn.name}`}
        onpointermove={positionTailAction}
        onclick={(event) => onInsert(lastColumn, null, event.currentTarget)}
      >
        <span class="tail-action"><Icon name="plus" size={14} /> Add column</span>
      </button>
    {/if}
  </div>
</div>
  <RowScrollbar
    {totalRows} {totalLabel} {firstVisibleRow} visibleRows={visibleRowCount} trackHeight={bodyViewportHeight} topOffset={HEADER_HEIGHT}
    controls="data-grid-scroll" disabled={seekDisabled} onSeek={onSeekRow}
    onDragLive={(row) => { onThumbMove(); scrollToAbsoluteRow(row); }} onDragRest={onSnapshotRest} onDragHeld={onThumbHeld}
  />
</div>

{#if contextMenu}
  <div bind:this={contextMenuElement} class="context-menu" role="menu" aria-label={`Actions for ${contextMenu.column.name}`} style:left={`${contextMenu.x}px`} style:top={`${contextMenu.y}px`}>
    <strong title={contextMenu.column.name}>{contextMenu.column.name}</strong>
    {#if canQuery}
      <button role="menuitem" onclick={() => runContextAction(() => onSort(contextMenu!.column))}>Sort</button>
      <button role="menuitem" onclick={() => runContextAction(() => onFilter(contextMenu!.column))}>Filter</button>
      {#if contextMenu.column.profile_kind}<button role="menuitem" onclick={() => runContextAction(() => onProfile(contextMenu!.column))}>Profile</button>{/if}
    {/if}
    <button role="menuitem" disabled={!canInsert} onclick={() => runContextAction(() => onModify(contextMenu!.column))}>{generatedColumns[contextMenu.column.name] !== undefined ? 'Edit formula' : 'Modify'}{#if generatedColumns[contextMenu.column.name] !== undefined}<kbd>⇧ right-click</kbd>{/if}</button>
    <button role="menuitem" onclick={() => runContextAction(() => onCopyColumn(contextMenu!.column))}>Copy column<kbd>⌘ C</kbd></button>
    <button role="menuitem" disabled={!canInsert || !canPasteColumn} onclick={() => runContextAction(() => onPasteColumn(contextMenu!.column))}>Paste column after<kbd>⌘ V</kbd></button>
    <button role="menuitem" disabled={!canInsert} onclick={() => runContextAction(() => onDuplicate(contextMenu!.column))}>Duplicate</button>
    <button role="menuitem" disabled={!canInsert} onclick={() => runContextAction(() => onRename(contextMenu!.column))}>Rename<kbd>dbl-click</kbd></button>
    <button role="menuitem" disabled={isColumnProtected(contextMenu.column.name) || columns.length <= 1} onclick={() => runContextAction(() => onHide(contextMenu!.column.name))}>Hide<kbd>⇧ dbl-click</kbd></button>
  </div>
{/if}

<style>
  .table-wrap { position: relative; width: 100%; height: 100%; }
  .table-scroll { overflow-anchor: none; width: 100%; height: 100%; overflow: auto; scrollbar-width: none; }
  .table-scroll::-webkit-scrollbar:vertical { display: none; }
  .table-scroll::-webkit-scrollbar:horizontal { height: 10px; }
  .table-scroll::-webkit-scrollbar-thumb:horizontal { background: var(--faint); border-radius: 8px; }
  .table-scroll::-webkit-scrollbar-track:horizontal { background: transparent; }
  .table-canvas { min-width: 100%; min-height: 100%; display: flex; align-items: stretch; }
  tbody { user-select: none; }
  table { min-width: 100%; border-collapse: separate; border-spacing: 0; font: 12px var(--font-mono); }
  .table-canvas.fit-columns table { width: max-content; min-width: 0; flex: none; }
  .add-column-tail {
    position: relative;
    min-width: 72px;
    flex: 1 0 72px;
    display: flex;
    align-items: flex-start;
    padding: 0;
    overflow: hidden;
    border: 0;
    border-left: 1px solid var(--line-soft);
    background-color: var(--surface-inset);
    background-image:
      linear-gradient(to bottom, transparent calc(var(--row-height, 34px) - 1px), var(--line-soft-2) calc(var(--row-height, 34px) - 1px)),
      linear-gradient(to right, transparent 31px, var(--line-soft-2) 31px, var(--line-soft-2) 32px, transparent 32px);
    background-position: 0 0;
    background-size: 100% var(--row-height, 34px), 32px 100%;
    color: var(--muted);
    cursor: pointer;
    isolation: isolate;
    text-align: left;
    animation: tail-reveal 180ms cubic-bezier(0.16, 1, 0.3, 1);
  }
  .add-column-tail::before {
    content: '';
    position: absolute;
    inset: 0;
    z-index: 0;
    pointer-events: none;
    background: linear-gradient(100deg, transparent 36%, color-mix(in srgb, var(--action) 7%, transparent) 48%, color-mix(in srgb, var(--action) 3%, transparent) 54%, transparent 66%);
    opacity: 0;
    transform: translateX(-100%);
  }
  .add-column-tail:hover:not(:disabled)::before,
  .add-column-tail:focus-visible::before {
    animation: data-pulse 1.152s linear infinite;
  }
  .tail-action {
    position: absolute;
    left: clamp(8px, var(--tail-x, 12px), calc(100% - 108px));
    top: clamp(8px, var(--tail-y, 60px), calc(100% - 36px));
    z-index: 1;
    display: inline-flex;
    align-items: center;
    gap: 6px;
    height: 28px;
    margin: 0;
    padding: 0 9px;
    border: 1px solid var(--control-border);
    border-radius: var(--radius-md);
    background: var(--surface);
    box-shadow: 0 5px 14px -11px rgba(31, 37, 51, 0.65);
    font: 500 11px var(--font-ui);
    white-space: nowrap;
    opacity: 0;
    pointer-events: none;
    transition: border-color 140ms ease, color 140ms ease, opacity 140ms ease;
  }
  .add-column-tail:hover:not(:disabled) .tail-action,
  .add-column-tail:focus-visible .tail-action { border-color: var(--line-strong); color: var(--ink); opacity: 1; }
  .add-column-tail:focus-visible { outline: 2px solid var(--action); outline-offset: -2px; }
  .add-column-tail:disabled { cursor: default; opacity: 0.55; }
  @keyframes tail-reveal { from { opacity: 0; background-position: 4px 0; } }
  @keyframes data-pulse {
    0% { opacity: 0; transform: translateX(-100%); }
    24% { opacity: 1; }
    100% { opacity: 0; transform: translateX(100%); }
  }
  .insertion-slot { position: sticky; top: 0; z-index: 4; width: 2px; min-width: 2px; height: 48px; padding: 0; border: 0; border-bottom: 1px solid var(--line); background: var(--surface-3); }
  td.insertion-gap { width: 2px; min-width: 2px; padding: 0; border-right: 0; background: color-mix(in srgb, var(--success) 4%, var(--surface)); }
  td {
    height: var(--row-height, 34px);
    max-width: 320px;
    padding: 0 8px;
    overflow: hidden;
    border-right: 1px solid var(--line-soft-2);
    border-bottom: 1px solid var(--line-soft-2);
    text-align: left;
    white-space: nowrap;
    text-overflow: ellipsis;
    background: var(--surface);
    transition: background-color 90ms ease;
  }
  tbody tr.striped td, tbody tr.striped .gutter { background: var(--surface-inset); }
  tbody tr.spacer td { padding: 0; border: 0; background: var(--surface); }
  tbody tr.pending { pointer-events: none; }
  tbody tr.aggregate-row td, tbody tr.aggregate-row .gutter { background: var(--surface); }
  tbody tr.aggregate-row.aggregate-row-alt td, tbody tr.aggregate-row.aggregate-row-alt .gutter { background: var(--surface-inset); }
  tbody tr td.generated, tbody tr.striped td.generated, tbody tr.aggregate-row td.generated { background: color-mix(in srgb, var(--action) 3%, var(--surface)); }
  /* Hover shades the cell under the pointer; holding shift widens it to the whole row. */
  tbody td[data-column]:hover { background: var(--action-tint); }
  tbody td.generated[data-column]:hover { background: var(--action-tint); }
  table.row-hover tbody tr:hover td[data-column],
  tbody tr:has(> .gutter:hover) td[data-column] { background: var(--action-tint); }
  tbody td[data-column].in-range { background: color-mix(in srgb, var(--action) 12%, var(--surface)); }
  .gutter {
    position: sticky;
    left: 0;
    z-index: 2;
    box-sizing: border-box;
    width: 12px;
    min-width: 12px;
    padding: 0;
    border-bottom: 1px solid var(--line-soft-2);
    background: var(--surface);
    cursor: pointer;
    user-select: none;
    transition: color 90ms ease, background-color 90ms ease;
  }
  /* A resting dot marks the strip as a target; it grows into the row's own bar on hover. */
  .gutter::before {
    content: '';
    position: absolute;
    left: 50%;
    top: 50%;
    width: 3px;
    height: 3px;
    border-radius: 999px;
    background: var(--faint);
    opacity: 0.55;
    translate: -50% -50%;
    transition: height 150ms cubic-bezier(0.16, 1, 0.3, 1), opacity 150ms ease, background-color 150ms ease;
  }
  .gutter:hover, .gutter.row-active { background: var(--surface-hover); }
  .gutter:hover::before, .gutter.row-active::before {
    height: calc(100% - 9px);
    opacity: 1;
    background: var(--action);
  }
  th.gutter-head::before, tbody tr.spacer .gutter::before { content: none; }
  th.gutter-head {
    top: 0;
    z-index: 5;
    height: 48px;
    border-bottom: 1px solid var(--line);
    background:
      repeating-linear-gradient(-45deg, var(--line-strong) 0 1px, transparent 1px 5px),
      var(--surface-3);
    cursor: default;
  }
  tbody tr.spacer .gutter { border-bottom: 0; cursor: default; }
  td.pinned, th.insertion-slot.pinned { position: sticky; z-index: 3; }
  th.insertion-slot.pinned { z-index: 5; }
  .pin-edge { box-shadow: 6px 0 8px -8px rgba(31, 37, 51, 0.55); }
  td.expanded-cell { white-space: normal; overflow: visible; position: relative; z-index: 2; box-shadow: var(--shadow-popover); }
  td.selected-cell { position: relative; z-index: 1; box-shadow: inset 0 0 0 2px var(--action); }
  td.editing-cell { padding: 0; overflow: visible; }
  td.editing-cell input { user-select: text; }
  td.editing-cell input { width: 100%; min-width: 120px; height: 100%; padding: 0 9px; border: 0; outline: 2px solid var(--action); outline-offset: -2px; background: var(--surface); color: var(--ink); font: inherit; }
  td.editing-cell input:disabled { opacity: 0.7; }
  .collapse { display: block; margin-top: 4px; font-size: 10.5px; color: var(--action); background: none; border: none; }
  .context-menu { position: fixed; z-index: 20; width: 176px; display: grid; padding: 4px; border: 1px solid var(--line-strong); border-radius: var(--radius-md); background: var(--surface); box-shadow: var(--shadow-popover); }
  .context-menu strong { min-width: 0; padding: 8px 9px; overflow: hidden; border-bottom: 1px solid var(--line); font-size: 12px; text-overflow: ellipsis; white-space: nowrap; }
  .context-menu button { display: flex; align-items: center; gap: 8px; min-height: 36px; padding: 0 9px; border: 0; border-radius: var(--radius-sm); background: transparent; color: var(--ink); text-align: left; }
  .context-menu kbd { margin-left: auto; font: 9.5px var(--font-mono); color: var(--faint); }
  .context-menu button:not(:disabled):hover, .context-menu button:not(:disabled):focus-visible { background: var(--surface-hover); }
  @media (prefers-reduced-motion: reduce) {
    td, .gutter, .gutter::before { transition: none; }
    .add-column-tail { animation: none; transition: none; }
    .add-column-tail::before { display: none; }
    .tail-action { transition: none; }
  }
</style>
