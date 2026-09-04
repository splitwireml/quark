<script lang="ts">
  import Button from '../atoms/Button.svelte';
  import Checkbox from '../atoms/Checkbox.svelte';
  import Chip from '../atoms/Chip.svelte';
  import IconButton from '../atoms/IconButton.svelte';
  import type { AggregateMetric, ColumnInfo } from '../../lib/types';

  type Props = {
    open: boolean;
    ontoggle: (event: Event) => void;
    label: string;
    aggregateColumnSearch: string;
    setAggregateColumnSearch: (value: string) => void;
    aggregateColumnMatches: ColumnInfo[];
    aggregateColumns: string[];
    aggregateFields: string[];
    focusedAggregateColumn: string;
    onToggleColumn: (name: string, checked: boolean) => void;
    onRemoveColumn: (name: string) => void;
    onFocusAggregate: (name: string) => void;
    onToggleRole: (name: string) => void;
    selectedAggregateColumn: ColumnInfo | undefined;
    availableMetrics: { value: AggregateMetric; label: string }[];
    aggregateMetrics: AggregateMetric[];
    onToggleMetric: (metric: AggregateMetric, checked: boolean) => void;
    onCreateView: () => void;
    creating: boolean;
  };

  let {
    open, ontoggle, label, aggregateColumnSearch, setAggregateColumnSearch,
    aggregateColumnMatches, aggregateColumns, aggregateFields, focusedAggregateColumn,
    onToggleColumn, onRemoveColumn, onFocusAggregate, onToggleRole,
    selectedAggregateColumn, availableMetrics, aggregateMetrics, onToggleMetric, onCreateView, creating
  }: Props = $props();

  function promoteWithKeyboard(event: KeyboardEvent, column: string) {
    if (event.key !== 'Enter' && event.key !== ' ') return;
    event.preventDefault();
    onToggleRole(column);
  }
</script>

<details class="popover-host" {open} {ontoggle}>
  <summary class="trigger" class:active={open}>{label}</summary>
  <div class="popover">
    <label class="search">Find a column
      <input type="search" value={aggregateColumnSearch} oninput={(event) => setAggregateColumnSearch((event.currentTarget as HTMLInputElement).value)} placeholder="Type to filter columns" />
    </label>
    <fieldset class="field-picker">
      <legend>Columns</legend>
      <div class="field-list">
        {#each aggregateColumnMatches as column (column.name)}
          <Checkbox checked={aggregateColumns.includes(column.name)} label={column.name} title={column.name} onchange={(checked) => onToggleColumn(column.name, checked)} />
        {:else}
          <p class="empty">No matching columns</p>
        {/each}
      </div>
    </fieldset>
    {#if aggregateColumns.length}
      <div class="columns" aria-label="Selected aggregation fields">
        {#each aggregateColumns as column (column)}
          {@const isAggregate = aggregateFields.includes(column)}
          <div class="column-role">
            <button
              type="button"
              class="role-pill"
              class:focused={isAggregate && focusedAggregateColumn === column}
              aria-label={isAggregate ? `Focus metrics for aggregate ${column}; double-click to make index` : `Promote index ${column} to aggregate`}
              aria-pressed={isAggregate ? focusedAggregateColumn === column : undefined}
              title={isAggregate ? 'Click to edit metrics · Double-click to make Index' : 'Double-click or press Enter to make Aggregate'}
              onclick={() => isAggregate && onFocusAggregate(column)}
              ondblclick={() => onToggleRole(column)}
              onkeydown={(event) => !isAggregate && promoteWithKeyboard(event, column)}
            >
              <Chip tone={isAggregate ? 'accent' : 'muted'}><b>{isAggregate ? 'Aggregate' : 'Index'}</b> {column}</Chip>
            </button>
            <IconButton type="button" glyph="×" label={`Remove ${isAggregate ? 'aggregate' : 'index'} field ${column}`} onclick={() => onRemoveColumn(column)} />
          </div>
        {/each}
      </div>
      <p class="note">Double-click a pill to switch between Index and Aggregate.</p>
      {#if selectedAggregateColumn}
        <fieldset class="metrics">
          <legend>Metrics for {selectedAggregateColumn.name}</legend>
          {#each availableMetrics as metric (metric.value)}
            <button type="button" class="metric-chip" class:on={aggregateMetrics.includes(metric.value)} onclick={() => onToggleMetric(metric.value, !aggregateMetrics.includes(metric.value))}>{metric.label}</button>
          {/each}
        </fieldset>
      {/if}
      <Button variant="primary" onclick={onCreateView} disabled={creating || !selectedAggregateColumn || aggregateMetrics.length === 0}>{creating ? 'Creating…' : 'Create view'}</Button>
    {/if}
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
  .popover {
    position: absolute; top: calc(100% + 6px); left: 0; z-index: 10;
    width: 300px; padding: 12px;
    border-radius: var(--radius-xl);
    background: var(--surface);
    border: 1px solid var(--line-strong);
    box-shadow: var(--shadow-popover);
    display: flex; flex-direction: column; gap: 10px;
  }
  .search { display: flex; flex-direction: column; gap: 5px; font-size: 11px; color: var(--muted); }
  .search input { height: 28px; padding: 0 8px; border-radius: var(--radius-md); border: 1px solid var(--control-border); font-size: 12px; }
  .field-picker, .metrics { border: none; margin: 0; padding: 0; }
  .field-list { max-height: 144px; overflow-y: auto; display: flex; flex-direction: column; gap: 7px; padding: 2px; }
  .empty { margin: 4px 0; font-size: 11px; color: var(--faint); }
  .columns { display: flex; flex-direction: column; gap: 5px; }
  .column-role { display: flex; align-items: center; gap: 3px; }
  .role-pill { min-width: 0; padding: 0; border: 0; border-radius: var(--radius-sm); background: transparent; }
  .role-pill.focused { outline: 2px solid var(--action); outline-offset: 1px; }
  .role-pill:focus-visible { outline: 2px solid var(--action); outline-offset: 1px; }
  .note { margin: 0; font-size: 11px; line-height: 1.5; color: var(--faint); }
  .metrics { display: flex; flex-wrap: wrap; gap: 5px; }
  legend { font-family: var(--font-mono); font-size: 9px; letter-spacing: 0.07em; text-transform: uppercase; color: var(--faint); padding: 0; margin-bottom: 4px; }
  .metric-chip {
    height: 26px; padding: 0 9px;
    border-radius: var(--radius-sm);
    border: 1px solid var(--control-border);
    background: var(--surface);
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--muted);
  }
  .metric-chip.on { border-color: var(--ink-fill); background: var(--ink-fill); color: #fff; }
</style>
