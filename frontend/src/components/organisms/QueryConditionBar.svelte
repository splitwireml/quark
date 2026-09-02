<script lang="ts">
  import type { Snippet } from 'svelte';
  import Button from '../atoms/Button.svelte';
  import Chip from '../atoms/Chip.svelte';
  import TextInput from '../atoms/TextInput.svelte';
  import type { FilterCondition, SortCondition } from '../../lib/types';

  type Props = {
    inert?: boolean;
    showBuilder: boolean;
    filters: FilterCondition[];
    sorts: SortCondition[];
    dedupeColumns: string[];
    activeSql: string;
    filterSummary: (filter: FilterCondition) => string;
    onToggleFilterConnector: (index: number) => void;
    onRemoveFilter: (index: number) => void;
    onRemoveSort: (column: string) => void;
    onClearDedupe: () => void;
    onSaveView: () => void;
    canSaveView: boolean;
    onClearConditions: () => void;
    isSqlMode: boolean;
    onBackToFullTable: () => void;
    onBackToBuilder: () => void;
    columnSearch: string;
    setColumnSearch: (value: string) => void;
    onFindColumn: () => void;
    onColumnSearchKeydown: (event: KeyboardEvent) => void;
    columnMatchCount: number;
    storageError: string;
    columnsMenu: Snippet;
    joinMenu: Snippet;
    aggregateMenu: Snippet;
    dedupeMenu: Snippet;
  };

  let {
    inert = false, showBuilder, filters, sorts, dedupeColumns, activeSql, filterSummary,
    onToggleFilterConnector, onRemoveFilter, onRemoveSort, onClearDedupe, onSaveView, canSaveView, onClearConditions,
    isSqlMode, onBackToFullTable, onBackToBuilder,
    columnSearch, setColumnSearch, onFindColumn, onColumnSearchKeydown, columnMatchCount,
    storageError,
    columnsMenu, joinMenu, aggregateMenu, dedupeMenu
  }: Props = $props();

  function scrollModifierRail(event: WheelEvent) {
    if (event.deltaX !== 0 || event.deltaY === 0) return;
    event.preventDefault();
    (event.currentTarget as HTMLElement).scrollLeft += event.deltaY;
  }
</script>

<section class="bar" aria-label="View controls" {inert}>
  {@render columnsMenu()}
  {@render joinMenu()}
  {#if showBuilder}
    {@render aggregateMenu()}
    <div class="dedupe-controls">
      {@render dedupeMenu()}
      {#if dedupeColumns.length}
        <Chip tone="muted" onRemove={onClearDedupe} removeLabel="Clear dedupe keys"><b>Dedupe</b> <span title={dedupeColumns.join(', ')}>{dedupeColumns.join(', ')}</span></Chip>
      {/if}
    </div>
    <!-- svelte-ignore a11y_no_noninteractive_tabindex (scrollable rail is intentionally keyboard-focusable) -->
    <div class="modifier-rail" role="region" aria-label="Active filters and sorts" tabindex="0" onwheel={scrollModifierRail}>
      {#each filters as filter, index (filter.column + index)}
        {#if index > 0}
          <button type="button" class="connector" aria-label={`Toggle connector before ${filter.column}; currently ${filter.connector === 'or' ? 'OR' : 'AND'}`} onclick={() => onToggleFilterConnector(index)}>{filter.connector === 'or' ? 'OR' : 'AND'}</button>
        {/if}
        <Chip tone="filter" onRemove={() => onRemoveFilter(index)} removeLabel={`Remove filter ${filter.column}`} title={filterSummary(filter)}><b>{filter.column}</b></Chip>
      {/each}
      {#each sorts as sort, index (sort.column)}
        <Chip tone="sort" onRemove={() => onRemoveSort(sort.column)} removeLabel={`Remove sort ${sort.column}`}><b>{index + 1}. {sort.column}</b> {sort.direction === 'asc' ? '↑' : '↓'}</Chip>
      {/each}
      {#if filters.length === 0 && sorts.length === 0}
        <span class="muted">No filters or sorts applied</span>
      {/if}
    </div>
    {#if filters.length || sorts.length || dedupeColumns.length}
      <Button onclick={onSaveView} disabled={!canSaveView}>Save View</Button>
      <button type="button" class="link" onclick={onClearConditions}>Clear conditions</button>
    {/if}
    {#if isSqlMode}<Button onclick={onBackToFullTable}>Back to active Version</Button>{/if}
  {:else}
    <div class="tokens"><Chip title={activeSql}>SQL View</Chip></div>
    <Button onclick={onSaveView} disabled={!activeSql}>Save View</Button>
    <Button onclick={onBackToBuilder}>Back to builder</Button>
  {/if}
  {#if storageError}<span class="error" role="alert">{storageError}</span>{/if}
  <div class="column-search">
    <TextInput type="search" glyph="⌕" value={columnSearch} oninput={(event: Event) => { setColumnSearch((event.currentTarget as HTMLInputElement).value); onFindColumn(); }} onkeydown={onColumnSearchKeydown} placeholder="Find column" aria-label="Find column" />
    <span class="match-count" aria-live="polite" aria-label={`${columnMatchCount} matching columns`}>{columnMatchCount}</span>
  </div>
</section>

<style>
  .bar {
    position: relative;
    flex: none;
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
    min-height: 40px;
    padding: 8px 20px;
    border-bottom: 1px solid var(--line);
    background: var(--surface-2);
  }
  .tokens, .dedupe-controls, .column-search { display: flex; align-items: center; gap: 6px; flex-wrap: nowrap; }
  .modifier-rail {
    display: flex;
    align-items: center;
    gap: 6px;
    flex: 1 1 12rem;
    min-width: 0;
    max-width: min(48rem, 45vw);
    overflow-x: auto;
    overflow-y: hidden;
    scrollbar-width: none;
    white-space: nowrap;
  }
  .modifier-rail::-webkit-scrollbar { display: none; }
  .modifier-rail:focus-visible { outline: 2px solid var(--action); outline-offset: 2px; }
  .connector {
    flex: none;
    height: 20px;
    padding: 0 6px;
    border: 1px solid var(--control-border);
    border-radius: 999px;
    background: var(--surface);
    color: var(--muted);
    font: 600 9px var(--font-mono);
  }
  .connector:hover { border-color: var(--action-tint-border); color: var(--action-dark); }
  .muted { font-size: 11.5px; color: var(--faint); white-space: nowrap; }
  .link { font-size: 11.5px; color: var(--action); background: none; border: none; white-space: nowrap; }
  .column-search { margin-left: auto; flex: none; }
  .match-count { font-family: var(--font-mono); font-size: 10.5px; color: var(--faint); flex: none; }
  .error { font-size: 11px; color: var(--error); flex: none; }
</style>
