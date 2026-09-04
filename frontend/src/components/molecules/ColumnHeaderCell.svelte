<script lang="ts">
  import Icon from '../atoms/Icon.svelte';
  import NullGauge from '../atoms/NullGauge.svelte';
  import type { ColumnInfo, SortCondition } from '../../lib/types';

  type LabelPart = { text: string; match: boolean };
  type Placement = 'before' | 'after';

  type Props = {
    column: ColumnInfo;
    fitColumnsToContent: boolean;
    labelParts: LabelPart[];
    sort: SortCondition | undefined;
    sortRank: number;
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
    column, fitColumnsToContent, labelParts, sort, sortRank, filtered, canQuery, protectedColumn, canHide, canReorder, dragging, dropPlacement,
    renaming, renameValue, renameSaving, onsort, onfilter, onprofile, onhide, onstartrename, onrenamevalue,
    oncommitrename, oncancelrename, oncontextmenu, ondragstart, ondragover, ondragend
  }: Props = $props();

  let width = $derived(Math.max(160, Math.min(360, column.name.length * 9 + 104)));
  let hideable = $derived(canHide && !protectedColumn);
  let nullPercent = $derived(Math.round(column.null_fraction * 100));
  let draggable = $derived(canReorder && !renaming);
  // The whole header is the drag handle, so the press has to read on the header itself.
  let pressed = $state(false);
  let sortIcon = $derived(sort?.direction === 'asc' ? 'sort-asc' as const : sort?.direction === 'desc' ? 'sort-desc' as const : 'sort' as const);
  let title = $derived(
    `${column.name} — double-click to rename${hideable ? ' · shift-double-click to hide' : ''}`,
  );

  function focusRename(node: HTMLInputElement) { queueMicrotask(() => { node.focus(); node.select(); }); }
  function renameKeydown(event: KeyboardEvent) {
    event.stopPropagation();
    if (event.key === 'Escape') { event.preventDefault(); oncancelrename(); }
    else if (event.key === 'Enter') { event.preventDefault(); oncommitrename(); }
  }
  function onHeaderPointerDown(event: PointerEvent) {
    if (!draggable || (event.target as HTMLElement).closest('button, input')) return;
    pressed = true;
  }
  function onHeaderDoubleClick(event: MouseEvent) {
    if (renaming || (event.target as HTMLElement).closest('button, input')) return;
    event.preventDefault();
    if (event.shiftKey) { if (hideable) onhide(); return; }
    onstartrename();
  }
</script>

<th
  data-column={column.name}
  tabindex="-1"
  scope="col"
  {draggable}
  aria-sort={sort ? (sort.direction === 'asc' ? 'ascending' : 'descending') : 'none'}
  {oncontextmenu}
  {ondragover}
  {ondragend}
  ondragstart={(event) => { pressed = false; ondragstart(event); }}
  onpointerdown={onHeaderPointerDown}
  onpointerup={() => (pressed = false)}
  onpointerleave={() => (pressed = false)}
  onpointercancel={() => (pressed = false)}
  ondblclick={onHeaderDoubleClick}
  class:dragging
  class:pressed
  class:grabbable={draggable}
  class:drop-before={dropPlacement === 'before'}
  class:drop-after={dropPlacement === 'after'}
  style:min-width={fitColumnsToContent ? '0' : `${width}px`}
>
  <div class="head">
    <div class="label">
      {#if renaming}
        <input use:focusRename value={renameValue} disabled={renameSaving} aria-label={`Rename column ${column.name}`} oninput={(event) => onrenamevalue(event.currentTarget.value)} onkeydown={renameKeydown} onblur={oncommitrename} onclick={(event) => event.stopPropagation()} />
      {:else}
        <strong {title}>
          {#each labelParts as part, index ((part.match ? 'm' : 't') + index + part.text)}
            {#if part.match}<mark>{part.text}</mark>{:else}{part.text}{/if}
          {/each}
        </strong>
        <small>{column.type}</small>
        {#if canQuery}
          <div class="actions">
            <button type="button" class:on={!!sort} onclick={onsort} aria-label={`Sort ${column.name}`} data-tip="Sort">
              <Icon name={sortIcon} size={13} />
              {#if sortRank}<span class="rank">{sortRank}</span>{/if}
            </button>
            <button type="button" class:on={filtered} onclick={(event) => onfilter(event.currentTarget as HTMLButtonElement)} aria-label={`Filter ${column.name}`} data-tip="Filter">
              <Icon name="filter" size={13} />
            </button>
          </div>
        {/if}
      {/if}
    </div>
    {#if canQuery && column.profile_kind}
      <button
        type="button"
        class="profile"
        aria-label={`Profile column ${column.name}`}
        data-tip={nullPercent ? `Profile · ${nullPercent}% null` : 'Profile'}
        data-tip-align="start"
        onclick={(event) => onprofile(event.currentTarget as HTMLButtonElement)}
      >
        <Icon name="histogram" size={12} />
        <NullGauge fraction={column.null_fraction} />
      </button>
    {:else}
      <div class="profile static"><NullGauge fraction={column.null_fraction} /></div>
    {/if}
  </div>
</th>

<style>
  th {
    position: sticky;
    top: 0;
    z-index: 3;
    height: 48px;
    padding: 0 8px;
    box-sizing: border-box;
    background: var(--surface-3);
    border-right: 1px solid var(--line-soft);
    border-bottom: 1px solid var(--line);
    text-align: left;
    vertical-align: top;
    transition: background-color 120ms ease;
  }
  th.grabbable { cursor: grab; }
  th.pressed { cursor: grabbing; background: var(--surface-hover); }
  th.dragging { opacity: 0.55; }
  th.drop-before { box-shadow: inset 3px 0 var(--action); }
  th.drop-after { box-shadow: inset -3px 0 var(--action); }

  .head { display: flex; flex-direction: column; height: 100%; }
  /* The press travels through the contents; the sticky cell itself must not move. */
  .head { transition: transform 110ms cubic-bezier(0.16, 1, 0.3, 1); }
  th.pressed .head { transform: translateY(1px); }

  .label { display: flex; align-items: center; gap: 6px; flex: 1; min-width: 0; }
  strong {
    font-family: var(--font-mono);
    font-size: 12px;
    font-weight: 600;
    color: var(--ink);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  strong mark { background: var(--action-tint); color: var(--action-dark); }
  .label input { min-width: 0; width: 100%; height: 24px; padding: 0 6px; border: 1px solid var(--action); border-radius: var(--radius-sm); background: var(--surface); color: var(--ink); font: 12px var(--font-mono); }
  small { font-family: var(--font-mono); font-size: 10.5px; letter-spacing: 0.04em; text-transform: uppercase; color: var(--faint); flex: none; }

  .actions { display: flex; align-items: center; gap: 2px; margin-left: auto; flex: none; }
  .actions button {
    position: relative;
    display: flex; align-items: center; justify-content: center;
    width: 22px; height: 22px;
    border-radius: var(--radius-sm);
    border: 1px solid transparent;
    background: transparent;
    color: var(--glyph);
    transition: color 120ms ease, background-color 120ms ease, border-color 120ms ease;
  }
  th:hover .actions button, th:focus-within .actions button { color: var(--muted); }
  .actions button:hover { background: var(--surface); border-color: var(--control-border); color: var(--ink); }
  .actions button.on { color: var(--action); border-color: var(--action-tint-border); background: var(--action-tint); }
  .actions button:focus-visible { outline: 2px solid var(--action); outline-offset: 1px; }
  .rank {
    position: absolute;
    top: 0; right: 0;
    font: 500 9px/1 var(--font-mono);
    color: var(--action-dark);
  }

  /* Profiling reads as a seam of data under the title: tinted where it opens, plain where it cannot. */
  .profile {
    display: flex;
    align-items: center;
    gap: 7px;
    height: 19px;
    margin: 0 -8px;
    padding: 0 8px;
    border: 0;
    border-top: 1px solid var(--action-tint-border);
    background: color-mix(in srgb, var(--action) 8%, var(--surface));
    box-shadow: inset 0 -1px transparent;
    color: var(--faint);
    text-align: left;
    transition: background-color 130ms ease, color 130ms ease, box-shadow 130ms ease;
  }
  .profile.static { background: transparent; border-top-color: var(--line-soft); }
  .profile:not(.static) { cursor: pointer; }
  .profile:not(.static):hover,
  .profile:not(.static):focus-visible {
    background: var(--action-tint);
    color: var(--action);
    box-shadow: inset 0 -1px var(--action);
  }
  .profile:focus-visible { outline: 2px solid var(--action); outline-offset: -2px; }

  @media (prefers-reduced-motion: reduce) {
    th, .head, .actions button, .profile { transition: none; }
    th.pressed .head { transform: none; }
  }
</style>
