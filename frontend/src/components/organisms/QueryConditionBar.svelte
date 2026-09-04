<script lang="ts">
  import type { Snippet } from 'svelte';
  import Button from '../atoms/Button.svelte';
  import Chip from '../atoms/Chip.svelte';
  import IconButton from '../atoms/IconButton.svelte';
  import TextInput from '../atoms/TextInput.svelte';
  import { dismissable } from '../../lib/dismiss';
  import type { FilterCondition, RowDensity, SortCondition } from '../../lib/types';

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
    rowDensity: RowDensity;
    setRowDensity: (density: RowDensity) => void;
    tableExpanded: boolean;
    onToggleExpanded: () => void;
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
    rowDensity, setRowDensity, tableExpanded, onToggleExpanded,
    columnsMenu, joinMenu, aggregateMenu, dedupeMenu
  }: Props = $props();

  const densities: { value: RowDensity; icon: 'density-compact' | 'density-default' | 'density-comfortable'; label: string }[] = [
    { value: 'compact', icon: 'density-compact', label: 'Compact rows' },
    { value: 'default', icon: 'density-default', label: 'Default rows' },
    { value: 'comfortable', icon: 'density-comfortable', label: 'Comfortable rows' }
  ];

  let densityOpen = $state(false);

  function chooseDensity(density: RowDensity) {
    setRowDensity(density);
    densityOpen = false;
  }

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
  <div class="view-controls">
    <div class="density-anchor">
      <IconButton
        type="button" size="md" icon="spacing" active={densityOpen}
        label="Row spacing" data-menu-trigger
        aria-expanded={densityOpen} aria-haspopup="true"
        onclick={() => densityOpen = !densityOpen}
      />
      {#if densityOpen}
        <div class="density-menu" use:dismissable={() => densityOpen = false} role="group" aria-label="Row spacing">
          {#each densities as density (density.value)}
            <IconButton
              type="button" size="md" icon={density.icon} label={density.label}
              active={rowDensity === density.value}
              onclick={() => chooseDensity(density.value)}
            />
          {/each}
        </div>
      {/if}
    </div>
    <IconButton
      type="button" size="md" icon={tableExpanded ? 'collapse' : 'expand'}
      label={tableExpanded ? 'Exit expanded table view' : 'Expand table to fill the window'}
      onclick={onToggleExpanded}
    />
  </div>
  <div class="column-search">
    <TextInput type="search" size="md" glyph="⌕" value={columnSearch} oninput={(event: Event) => { setColumnSearch((event.currentTarget as HTMLInputElement).value); onFindColumn(); }} onkeydown={onColumnSearchKeydown} placeholder="Find column" aria-label="Find column" />
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
  .link { font-size: 11.5px; color: var(--action); background: none; border: none; white-space: nowrap; }
  .view-controls { display: flex; align-items: center; gap: 4px; margin-left: auto; flex: none; }
  .density-anchor { position: relative; display: flex; }
  .density-menu {
    position: absolute;
    top: calc(100% + 6px);
    left: 50%;
    z-index: 12;
    display: flex;
    gap: 2px;
    padding: 4px;
    border: 1px solid var(--line-strong);
    border-radius: var(--radius-xl);
    background: var(--surface);
    box-shadow: var(--shadow-popover);
    transform-origin: top center;
    animation: density-in 200ms cubic-bezier(0.32, 0.72, 0, 1);
    translate: -50% 0;
  }
  @keyframes density-in {
    from { opacity: 0; transform: translateY(-6px) scale(0.94); }
    to { opacity: 1; transform: none; }
  }
  .column-search { flex: none; }
  .match-count { font-family: var(--font-mono); font-size: 10.5px; color: var(--faint); flex: none; }
  .error { font-size: 11px; color: var(--error); flex: none; }
</style>
