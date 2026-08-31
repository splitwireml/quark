<script lang="ts">
  import Button from '../atoms/Button.svelte';
  import Chip from '../atoms/Chip.svelte';
  import type { AggregateMetric, ColumnInfo } from '../../lib/types';

  type Props = {
    open: boolean;
    ontoggle: (event: Event) => void;
    label: string;
    aggregateColumnSearch: string;
    setAggregateColumnSearch: (value: string) => void;
    aggregateColumn: string;
    aggregateColumnMatches: ColumnInfo[];
    aggregateColumns: string[];
    onAddColumn: (name: string) => void;
    onRemoveColumn: (name: string) => void;
    selectedAggregateColumn: ColumnInfo | undefined;
    availableMetrics: { value: AggregateMetric; label: string }[];
    aggregateMetrics: AggregateMetric[];
    onToggleMetric: (metric: AggregateMetric, checked: boolean) => void;
    onCreateView: () => void;
    creating: boolean;
  };

  let {
    open, ontoggle, label, aggregateColumnSearch, setAggregateColumnSearch, aggregateColumn,
    aggregateColumnMatches, aggregateColumns, onAddColumn, onRemoveColumn, selectedAggregateColumn,
    availableMetrics, aggregateMetrics, onToggleMetric, onCreateView, creating
  }: Props = $props();
</script>

<details class="popover-host" {open} {ontoggle}>
  <summary class="trigger" class:active={open}>{label}</summary>
  <div class="popover">
    <label class="search">Find a column
      <input type="search" value={aggregateColumnSearch} oninput={(event) => setAggregateColumnSearch((event.currentTarget as HTMLInputElement).value)} placeholder="Type to filter columns" />
    </label>
    <label class="search">Add column
      <select value={aggregateColumn} onchange={(event) => onAddColumn((event.currentTarget as HTMLSelectElement).value)}>
        <option value="">Choose index or aggregate column</option>
        {#each aggregateColumnMatches as column (column.name)}
          {#if !aggregateColumns.includes(column.name)}<option value={column.name}>{column.name}</option>{/if}
        {/each}
      </select>
    </label>
    {#if aggregateColumns.length}
      <div class="columns">
        {#each aggregateColumns as column, index (column)}
          <Chip tone="accent" onRemove={() => onRemoveColumn(column)} removeLabel={`Remove aggregate column ${column}`}>
            <b>{index < aggregateColumns.length - 1 || aggregateColumns.length === 1 ? 'Index' : 'Aggregate'}</b> {column}
          </Chip>
        {/each}
      </div>
      <p class="note">Earlier columns are indexes. One column + Count shows its distribution.</p>
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
  .search input, .search select { height: 28px; padding: 0 8px; border-radius: var(--radius-md); border: 1px solid var(--control-border); font-size: 12px; }
  .columns { display: flex; flex-wrap: wrap; gap: 5px; }
  .note { margin: 0; font-size: 11px; line-height: 1.5; color: var(--muted-2); }
  .metrics { border: none; margin: 0; padding: 0; display: flex; flex-wrap: wrap; gap: 5px; }
  legend { font-family: var(--font-mono); font-size: 9px; letter-spacing: 0.07em; text-transform: uppercase; color: var(--faint-2); padding: 0; margin-bottom: 4px; }
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
