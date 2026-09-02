<script lang="ts">
  import NullGauge from '../atoms/NullGauge.svelte';
  import type { ColumnInfo, SortCondition } from '../../lib/types';

  type LabelPart = { text: string; match: boolean };
  type Placement = 'before' | 'after';

  type Props = {
    column: ColumnInfo;
    labelParts: LabelPart[];
    sort: SortCondition | undefined;
    filtered: boolean;
    canQuery: boolean;
    protectedColumn: boolean;
    canHide: boolean;
    canReorder: boolean;
    dragging: boolean;
    dropPlacement: Placement | null;
    renaming: boolean;
    renameValue: string;
    renameSaving: boolean;
    onsort: () => void;
    onfilter: (trigger: HTMLButtonElement) => void;
    onprofile: (trigger: HTMLButtonElement) => void;
    onhide: () => void;
    onstartrename: () => void;
    onrenamevalue: (value: string) => void;
    oncommitrename: () => void;
    oncancelrename: () => void;
    oncontextmenu: (event: MouseEvent) => void;
    ondragstart: (event: DragEvent) => void;
    ondragover: (event: DragEvent) => void;
    ondragend: (event: DragEvent) => void;
  };

  let {
    column, labelParts, sort, filtered, canQuery, protectedColumn, canHide, canReorder, dragging, dropPlacement,
    renaming, renameValue, renameSaving, onsort, onfilter, onprofile, onhide, onstartrename, onrenamevalue,
    oncommitrename, oncancelrename, oncontextmenu, ondragstart, ondragover, ondragend
  }: Props = $props();
  let width = $derived(Math.max(168, Math.min(360, column.name.length * 9 + (column.profile_kind ? 150 : 118))));

  function focusRename(node: HTMLInputElement) { queueMicrotask(() => { node.focus(); node.select(); }); }
  function renameKeydown(event: KeyboardEvent) {
    event.stopPropagation();
    if (event.key === 'Escape') { event.preventDefault(); oncancelrename(); }
    else if (event.key === 'Enter') { event.preventDefault(); oncommitrename(); }
  }
</script>

<th
  data-column={column.name}
  tabindex="-1"
  scope="col"
  {oncontextmenu}
  {ondragover}
  class:dragging
  class:drop-before={dropPlacement === 'before'}
  class:drop-after={dropPlacement === 'after'}
  style:min-width={`${width}px`}
>
  <div class="head">
    <div class="label">
      {#if renaming}
        <input use:focusRename value={renameValue} disabled={renameSaving} aria-label={`Rename column ${column.name}`} oninput={(event) => onrenamevalue(event.currentTarget.value)} onkeydown={renameKeydown} onblur={oncommitrename} onclick={(event) => event.stopPropagation()} ondblclick={(event) => event.stopPropagation()} />
      {:else}
        <strong title={`${column.name} — Double-click to rename`} ondblclick={(event) => { event.stopPropagation(); onstartrename(); }}>
          {#each labelParts as part, index ((part.match ? 'm' : 't') + index + part.text)}
            {#if part.match}<mark>{part.text}</mark>{:else}{part.text}{/if}
          {/each}
        </strong>
      {/if}
      <small>{column.type}</small>
    </div>
    <div class="actions">
      <button
        type="button"
        class="drag-handle"
        draggable={canReorder && !renaming}
        disabled={!canReorder || renaming}
        aria-label={`Drag to reorder column ${column.name}`}
        aria-pressed={dragging}
        title={`Drag to reorder ${column.name}`}
        {ondragstart}
        {ondragend}
        onclick={(event) => event.stopPropagation()}
      >⋮⋮</button>
      {#if canQuery}
        <button class:on={!!sort} onclick={onsort} aria-label={`Sort ${column.name}`} title="Sort">{sort?.direction === 'asc' ? '↑' : sort?.direction === 'desc' ? '↓' : '↕'}</button>
        <button class:on={filtered} onclick={(event) => onfilter(event.currentTarget as HTMLButtonElement)} aria-label={`Filter ${column.name}`} title="Filter">⌕</button>
        {#if column.profile_kind}
          <button onclick={(event) => onprofile(event.currentTarget as HTMLButtonElement)} aria-label={`Profile column ${column.name}`} title="Profile">▥</button>
        {/if}
      {/if}
      <button onclick={onhide} disabled={protectedColumn || !canHide} aria-label={`Hide column ${column.name}`} title="Hide">×</button>
    </div>
    <NullGauge fraction={column.null_fraction} />
  </div>
</th>

<style>
  th {
    position: sticky;
    top: 0;
    z-index: 3;
    height: 48px;
    padding: 6px 10px 8px;
    box-sizing: border-box;
    background: var(--surface-3);
    border-right: 1px solid var(--line-soft);
    border-bottom: 1px solid var(--line);
    text-align: left;
    vertical-align: top;
  }
  th.dragging { opacity: 0.55; }
  th.drop-before { box-shadow: inset 3px 0 var(--action); }
  th.drop-after { box-shadow: inset -3px 0 var(--action); }
  .head { display: flex; flex-direction: column; gap: 4px; height: 100%; }
  .label { display: flex; align-items: baseline; gap: 6px; min-width: 0; }
  strong {
    font-family: var(--font-mono);
    font-size: 11.5px;
    font-weight: 600;
    color: var(--ink);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  strong mark { background: var(--action-tint); color: var(--action-dark); }
  .label input { min-width: 0; width: 100%; height: 24px; padding: 0 6px; border: 1px solid var(--action); border-radius: var(--radius-sm); background: var(--surface); color: var(--ink); font: 11.5px var(--font-mono); }
  small { font-family: var(--font-mono); font-size: 9px; letter-spacing: 0.04em; text-transform: uppercase; color: var(--faint); flex: none; }
  .actions { display: flex; align-items: center; gap: 3px; height: 16px; }
  .actions button {
    display: flex; align-items: center; justify-content: center;
    width: 16px; height: 16px;
    border-radius: var(--radius-sm);
    background: var(--surface);
    border: 1px solid var(--line);
    color: var(--muted);
    font-size: 9px;
    line-height: 1;
  }
  .actions button:hover:not(:disabled) { border-color: var(--faint); color: var(--ink); }
  .actions button.on, .actions .drag-handle[aria-pressed="true"] { color: var(--action); border-color: var(--action-tint-border); background: var(--action-tint); }
  .actions button:disabled { opacity: 0.35; }
  .drag-handle { cursor: grab; letter-spacing: -3px; padding-right: 3px; }
  .drag-handle:active { cursor: grabbing; }
</style>
