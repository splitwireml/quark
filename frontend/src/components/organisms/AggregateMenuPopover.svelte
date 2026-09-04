<script lang="ts">
  import Button from '../atoms/Button.svelte';
  import MenuPopover from '../molecules/MenuPopover.svelte';
  import Chip from '../atoms/Chip.svelte';
  import TextInput from '../atoms/TextInput.svelte';
  import type { AggregateMetric, AggregateRecipeItem, ColumnInfo } from '../../lib/types';

  type Props = {
    open: boolean;
    ontoggle: (event: Event) => void;
    label: string;
    aggregateColumnSearch: string;
    setAggregateColumnSearch: (value: string) => void;
    aggregateColumnMatches: ColumnInfo[];
    aggregateRecipe: AggregateRecipeItem[];
    focusedAggregateItemId: number | null;
    onAddColumn: (name: string) => void;
    onRemoveColumn: (id: number) => void;
    onFocusAggregate: (id: number) => void;
    onToggleRole: (id: number) => void;
    selectedAggregateColumn: ColumnInfo | undefined;
    availableMetrics: { value: AggregateMetric; label: string }[];
    aggregateMetrics: AggregateMetric[];
    onToggleMetric: (metric: AggregateMetric, checked: boolean) => void;
    canCreateAggregate: boolean;
    onCreateView: () => void;
    creating: boolean;
  };

  let {
    open, ontoggle, label, aggregateColumnSearch, setAggregateColumnSearch,
    aggregateColumnMatches, aggregateRecipe, focusedAggregateItemId,
    onAddColumn, onRemoveColumn, onFocusAggregate, onToggleRole,
    selectedAggregateColumn, availableMetrics, aggregateMetrics, onToggleMetric,
    canCreateAggregate, onCreateView, creating
  }: Props = $props();
</script>

<MenuPopover {open} {ontoggle} icon="sigma" {label} width={360}>
  {#snippet header()}
    <strong>Aggregate</strong><span>{aggregateRecipe.length} {aggregateRecipe.length === 1 ? 'step' : 'steps'}</span>
  {/snippet}
    <div class="search">
      <label for="aggregate-menu-search" class="sr-only">Find a column</label>
      <TextInput id="aggregate-menu-search" type="search" glyph="⌕" value={aggregateColumnSearch} oninput={(event: Event) => setAggregateColumnSearch((event.currentTarget as HTMLInputElement).value)} placeholder="Find a column" />
    </div>
    <fieldset class="field-picker">
      <legend>Columns</legend>
      <div class="field-list">
        {#each aggregateColumnMatches as column (column.name)}
          {@const occurrenceCount = aggregateRecipe.filter((item) => item.column === column.name).length}
          <button
            type="button"
            class="field-option"
            class:used={occurrenceCount > 0}
            aria-label={occurrenceCount ? `Add another ${column.name} step; ${occurrenceCount} already in recipe` : `Add ${column.name} to aggregation recipe`}
            onclick={() => onAddColumn(column.name)}
          >
            <span title={column.name}>{column.name}</span>
            <small>{occurrenceCount ? `${occurrenceCount} in recipe · Add` : 'Add'}</small>
          </button>
        {:else}
          <p class="empty">No matching columns</p>
        {/each}
      </div>
    </fieldset>
    {#if aggregateRecipe.length}
      <fieldset class="recipe">
        <legend>Recipe</legend>
        <div class="columns" role="list" aria-label="Aggregation recipe">
          {#each aggregateRecipe as item (item.id)}
            {@const isAggregate = item.metrics !== null}
            {@const alreadyGrouped = isAggregate && aggregateRecipe.some((candidate) => candidate.id !== item.id && candidate.column === item.column && candidate.metrics === null)}
            <div class="column-role" role="listitem">
              <Chip tone={isAggregate ? 'accent' : 'muted'} onRemove={() => onRemoveColumn(item.id)} removeLabel={`Remove ${isAggregate ? 'Measure' : 'Group'} ${item.column} from aggregation recipe`}>
                <button
                  type="button"
                  class="role-toggle"
                  disabled={alreadyGrouped}
                  aria-label={alreadyGrouped ? `${item.column} already has a Group step` : `Change ${item.column} from ${isAggregate ? 'Measure to Group' : 'Group to Measure'}`}
                  title={alreadyGrouped ? 'Already grouped in this recipe' : `Change to ${isAggregate ? 'Group' : 'Measure'}`}
                  onclick={() => onToggleRole(item.id)}
                >{isAggregate ? 'Measure' : 'Group'}</button>
                <button
                  type="button"
                  class="field-name"
                  class:focused={isAggregate && focusedAggregateItemId === item.id}
                  disabled={!isAggregate}
                  aria-label={isAggregate ? `Edit operations for ${item.column}` : undefined}
                  onclick={() => onFocusAggregate(item.id)}
                >{item.column}</button>
              </Chip>
            </div>
          {/each}
        </div>
        <p class="note">Repeat a column to use it as both a Group and a Measure. Measure fields apply the operations below.</p>
      </fieldset>
      {#if selectedAggregateColumn}
        <fieldset class="metrics">
          <legend>Operations for {selectedAggregateColumn.name}</legend>
          {#each availableMetrics as metric (metric.value)}
            <button type="button" class="metric-chip" class:on={aggregateMetrics.includes(metric.value)} aria-pressed={aggregateMetrics.includes(metric.value)} onclick={() => onToggleMetric(metric.value, !aggregateMetrics.includes(metric.value))}>{metric.label}</button>
          {/each}
        </fieldset>
      {/if}
      <Button variant="primary" onclick={onCreateView} disabled={creating || !canCreateAggregate}>{creating ? 'Creating…' : 'Create view'}</Button>
    {/if}
</MenuPopover>

<style>
  .search :global(.field) { width: 100%; }
  .field-picker, .recipe, .metrics { min-width: 0; border: none; margin: 0; padding: 0; }
  .field-list { max-height: 168px; overflow-y: auto; display: flex; flex-direction: column; gap: 2px; padding: 2px; }
  .field-option {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    min-height: 30px;
    padding: 4px 8px;
    border: 0;
    border-radius: var(--radius-md);
    background: transparent;
    color: var(--ink);
    text-align: left;
  }
  .field-option:hover, .field-option:focus-visible { background: var(--surface-hover); }
  .field-option span { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font: 11.5px var(--font-mono); }
  .field-option small { flex: none; font: 10px var(--font-mono); color: var(--faint); }
  .field-option.used small { color: var(--action-dark); }
  .empty { margin: 4px 0; font-size: 11px; color: var(--faint); }
  .columns { display: flex; flex-wrap: wrap; align-items: center; gap: 6px; }
  .column-role { display: inline-flex; min-width: 0; animation: recipe-in 160ms cubic-bezier(0.16, 1, 0.3, 1); }
  .column-role :global(.chip) { height: 28px; gap: 4px; padding-left: 4px; }
  .column-role :global(.remove) { width: 22px; height: 24px; }
  .role-toggle, .field-name { min-width: 0; height: 24px; padding: 0 4px; border: 0; border-radius: 2px; background: transparent; color: inherit; font: inherit; }
  .role-toggle { font-size: 9px; font-weight: 600; letter-spacing: 0.05em; text-transform: uppercase; }
  .role-toggle:hover:not(:disabled), .role-toggle:focus-visible, .field-name:hover:not(:disabled), .field-name:focus-visible { background: color-mix(in srgb, currentColor 10%, transparent); }
  .role-toggle:disabled { opacity: 0.5; cursor: not-allowed; }
  .field-name { max-width: 148px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .field-name:disabled { opacity: 1; cursor: default; }
  .field-name.focused { box-shadow: inset 0 -2px currentColor; }
  .note { margin: 6px 0 0; font-size: 11px; line-height: 1.45; color: var(--faint); }
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
  @keyframes recipe-in { from { opacity: 0; transform: translateY(-3px) scale(0.96); } to { opacity: 1; transform: none; } }
  @media (prefers-reduced-motion: reduce) { .column-role { animation: none; } }
</style>
