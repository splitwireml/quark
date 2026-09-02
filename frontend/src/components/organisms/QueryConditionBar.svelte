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
    sqlOpen: boolean;
    onToggleSql: () => void;
    setSqlTrigger: (el: HTMLButtonElement | null) => void;
    storageError: string;
    columnsMenu: Snippet;
    joinMenu: Snippet;
    aggregateMenu: Snippet;
    dedupeMenu: Snippet;
  };

  let {
    inert = false, showBuilder, filters, sorts, dedupeColumns, activeSql, filterSummary,
    onRemoveFilter, onRemoveSort, onClearDedupe, onSaveView, canSaveView, onClearConditions,
    isSqlMode, onBackToFullTable, onBackToBuilder,
    columnSearch, setColumnSearch, onFindColumn, onColumnSearchKeydown, columnMatchCount,
    sqlOpen, onToggleSql, setSqlTrigger, storageError,
    columnsMenu, joinMenu, aggregateMenu, dedupeMenu
  }: Props = $props();

  function trigger(node: HTMLButtonElement) {
    setSqlTrigger(node);
    return { destroy: () => setSqlTrigger(null) };
  }
</script>

<section class="bar" aria-label="Dataset controls" {inert}>
  {@render columnsMenu()}
  {@render joinMenu()}
  {#if showBuilder}
    {@render aggregateMenu()}
    {@render dedupeMenu()}
    <div class="tokens" aria-label="Active conditions">
      {#each filters as filter, index (filter.column + index)}
        <Chip onRemove={() => onRemoveFilter(index)} removeLabel={`Remove filter ${filter.column}`} title={filterSummary(filter)}><b>{filter.column}</b></Chip>
      {/each}
      {#each sorts as sort, index (sort.column)}
        <Chip tone="accent" onRemove={() => onRemoveSort(sort.column)} removeLabel={`Remove sort ${sort.column}`}><b>{index + 1}. {sort.column}</b> {sort.direction === 'asc' ? '↑' : '↓'}</Chip>
      {/each}
      {#if dedupeColumns.length}
        <Chip tone="muted" onRemove={onClearDedupe} removeLabel="Clear dedupe keys"><b>Dedupe</b> <span title={dedupeColumns.join(', ')}>{dedupeColumns.join(', ')}</span></Chip>
      {/if}
      {#if filters.length === 0 && sorts.length === 0 && dedupeColumns.length === 0}
        <span class="muted">No filters, sorts, or dedupe applied</span>
      {/if}
    </div>
    {#if filters.length || sorts.length || dedupeColumns.length}
      <Button onclick={onSaveView} disabled={!canSaveView}>Save View</Button>
      <button type="button" class="link" onclick={onClearConditions}>Clear conditions</button>
    {/if}
    {#if isSqlMode}<Button onclick={onBackToFullTable}>Back to full table</Button>{/if}
  {:else}
    <div class="tokens"><Chip title={activeSql}>SQL view</Chip></div>
    <Button onclick={onSaveView} disabled={!activeSql}>Save View</Button>
    <Button onclick={onBackToBuilder}>Back to builder</Button>
  {/if}
  <TextInput type="search" glyph="⌕" value={columnSearch} oninput={(event: Event) => { setColumnSearch((event.currentTarget as HTMLInputElement).value); onFindColumn(); }} onkeydown={onColumnSearchKeydown} placeholder="Find column" aria-label="Find column" />
  <span class="match-count" aria-live="polite" aria-label={`${columnMatchCount} matching columns`}>{columnMatchCount}</span>
  <button use:trigger class="sql-trigger" class:active={sqlOpen} aria-expanded={sqlOpen} onclick={onToggleSql}>SQL view <small>⌘K</small></button>
  {#if storageError}<span class="error" role="alert">{storageError}</span>{/if}
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
  .tokens { display: flex; align-items: center; gap: 6px; flex-wrap: nowrap; }
  .muted { font-size: 11.5px; color: var(--faint); white-space: nowrap; }
  .link { font-size: 11.5px; color: var(--action); background: none; border: none; white-space: nowrap; }
  .match-count { font-family: var(--font-mono); font-size: 10.5px; color: var(--faint); flex: none; }
  .sql-trigger {
    margin-left: auto;
    display: inline-flex; align-items: center; gap: 6px;
    height: 24px; padding: 0 8px;
    border-radius: var(--radius-sm);
    border: 1px solid var(--control-border);
    background: var(--surface);
    font-size: 11px;
    color: var(--ink-2);
    flex: none;
  }
  .sql-trigger.active { border-color: var(--action); background: var(--action-tint); color: var(--action-dark); }
  .sql-trigger small { font-family: var(--font-mono); color: var(--faint); }
  .error { font-size: 11px; color: var(--error); flex: none; }
</style>
