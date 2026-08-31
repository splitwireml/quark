<script lang="ts">
  import Button from '../atoms/Button.svelte';
  import TextInput from '../atoms/TextInput.svelte';
  import type { ColumnInfo } from '../../lib/types';

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
  };

  let {
    open, ontoggle, visibleCount, totalCount, columnMenuSearch, setColumnMenuSearch,
    columnTypes, columnTypeCounts, isTypeShown, toggleShownType, typeToggleDisabled,
    nullThreshold, setNullThreshold, onHideFullyEmpty, onApplyThreshold, onShowAll, hiddenCount,
    columnMenuItems, hiddenColumns, isColumnProtected, visibleColumnsLength, onToggleColumn
  }: Props = $props();
</script>

<details class="popover-host" {open} {ontoggle}>
  <summary class="trigger" class:active={open}>Columns <small>{visibleCount}/{totalCount}</small></summary>
  <div class="popover">
    <header><strong>Columns</strong><span>{visibleCount} of {totalCount} visible</span></header>
    <div class="search">
      <label for="column-menu-search" class="sr-only">Search columns</label>
      <TextInput id="column-menu-search" type="search" glyph="⌕" value={columnMenuSearch} oninput={(event: Event) => setColumnMenuSearch((event.currentTarget as HTMLInputElement).value)} placeholder="Search columns" />
    </div>
    <div class="type-row" role="group" aria-label="Show by type">
      {#each columnTypes as type (type)}
        <button type="button" class="type-chip" class:on={isTypeShown(type)} disabled={typeToggleDisabled(type)} aria-pressed={isTypeShown(type)} aria-label={`${type}: ${columnTypeCounts[type] ?? 0} columns`} onclick={() => toggleShownType(type, !isTypeShown(type))}>
          {#if isTypeShown(type)}<span class="check">✓</span>{:else}<span class="check empty"></span>{/if}
          {type} <b>{columnTypeCounts[type] ?? 0}</b>
        </button>
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
    <div class="list">
      {#each columnMenuItems as column (column.name)}
        {@const disabled = isColumnProtected(column.name) || (!hiddenColumns.includes(column.name) && visibleColumnsLength <= 1)}
        <label class="row" class:disabled>
          <input type="checkbox" checked={!hiddenColumns.includes(column.name)} {disabled} onchange={(event) => onToggleColumn(column.name, (event.currentTarget as HTMLInputElement).checked)} />
          <span class="box" aria-hidden="true">{#if !hiddenColumns.includes(column.name)}✓{/if}</span>
          <span title={column.name}>{column.name}</span>
          <small>{column.type}</small>
          <small class:warn={column.null_fraction >= 0.3}>{(column.null_fraction * 100).toFixed(0)}%</small>
        </label>
      {:else}
        <p class="muted">No matching columns.</p>
      {/each}
    </div>
    <footer><span>Dedupe keys stay visible</span><span class="esc">esc</span></footer>
  </div>
</details>

<style>
  .popover-host { position: relative; }
  summary { list-style: none; cursor: pointer; }
  summary::-webkit-details-marker { display: none; }
  .trigger {
    display: inline-flex; align-items: center; gap: 6px;
    height: 30px; padding: 0 12px;
    border-radius: var(--radius-lg);
    border: 1px solid var(--control-border);
    background: var(--surface);
    font-size: 12.5px;
    font-weight: 500;
    color: var(--ink-2);
  }
  .trigger:hover { border-color: var(--faint); }
  .trigger.active { border-color: var(--action); background: var(--action-tint); color: var(--action-dark); }
  .trigger small { font-family: var(--font-mono); color: var(--faint); }
  .trigger.active small { color: var(--action-dark); }
  .popover {
    position: absolute;
    top: calc(100% + 6px);
    left: 0;
    z-index: 10;
    width: 340px;
    border-radius: var(--radius-xl);
    background: var(--surface);
    border: 1px solid var(--line-strong);
    box-shadow: var(--shadow-popover-wide);
    overflow: hidden;
  }
  header { display: flex; align-items: center; gap: 8px; height: 32px; padding: 0 11px; border-bottom: 1px solid var(--line-soft); background: var(--surface-2); }
  header strong { font-size: 11.5px; }
  header span { margin-left: auto; font-family: var(--font-mono); font-size: 10px; color: var(--faint); }
  .search { padding: 9px 11px 0; }
  .type-row { display: flex; flex-wrap: wrap; gap: 5px; padding: 9px 11px 0; }
  .type-chip {
    display: inline-flex; align-items: center; gap: 5px;
    height: 24px; padding: 0 8px;
    border-radius: var(--radius-sm);
    border: 1px solid var(--control-border);
    background: var(--surface);
    font-family: var(--font-mono);
    font-size: 10.5px;
    color: var(--muted);
  }
  .type-chip.on { border-color: var(--action-tint-border); background: var(--action-tint); color: var(--action-dark); }
  .type-chip .check { width: 11px; height: 11px; display: inline-flex; align-items: center; justify-content: center; border-radius: 2px; font-size: 7.5px; }
  .type-chip.on .check { background: var(--action); color: #fff; }
  .type-chip .check.empty { border: 1px solid var(--control-border); }
  .type-chip:disabled { opacity: 0.5; }
  .threshold-row { display: flex; flex-wrap: wrap; align-items: center; gap: 6px; padding: 9px 11px 11px; border-bottom: 1px solid var(--line-soft); }
  .threshold { display: inline-flex; align-items: center; gap: 5px; font-size: 11px; color: var(--ink-2); }
  .threshold input { width: 34px; height: 22px; text-align: center; border-radius: var(--radius-sm); border: 1px solid var(--control-border); }
  .link { font-size: 11px; color: var(--action); background: none; border: none; }
  .link:disabled { color: var(--placeholder); }
  .list { max-height: 260px; overflow-y: auto; padding: 6px 6px 0; display: flex; flex-direction: column; }
  .row { display: flex; align-items: center; gap: 8px; height: 28px; padding: 0 6px; border-radius: var(--radius-md); cursor: pointer; }
  .row:hover { background: var(--surface-hover); }
  .row.disabled { opacity: 0.5; cursor: not-allowed; }
  .row input { position: absolute; width: 1px; height: 1px; opacity: 0; }
  .row .box {
    display: inline-flex; align-items: center; justify-content: center; flex: none;
    width: 12px; height: 12px; border-radius: 2px; border: 1px solid var(--placeholder);
    color: #fff; font-size: 8px;
  }
  .row input:checked + .box { background: var(--action); border-color: var(--action); }
  .row > span:not(.box) { font-family: var(--font-mono); font-size: 11px; color: var(--ink); flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .row small { font-family: var(--font-mono); font-size: 9.5px; color: var(--faint); flex: none; }
  .row small.warn { color: var(--warning); }
  .muted { padding: 12px; font-size: 12px; color: var(--muted); }
  footer { display: flex; align-items: center; gap: 8px; height: 34px; padding: 0 11px; border-top: 1px solid var(--line-soft); background: var(--surface-2); font-size: 11px; color: var(--muted-2); }
  .esc { margin-left: auto; font-family: var(--font-mono); color: var(--placeholder); }
</style>
