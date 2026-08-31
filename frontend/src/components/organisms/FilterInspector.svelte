<script lang="ts">
  import Button from '../atoms/Button.svelte';
  import FilterOperatorForm from '../molecules/FilterOperatorForm.svelte';
  import type { AggregateCount, CategoryValue, ColumnInfo, FilterOperator } from '../../lib/types';

  type Props = {
    column: ColumnInfo;
    isText: boolean;
    operators: { value: FilterOperator; label: string }[];
    operator: FilterOperator;
    value: string;
    setOperator: (v: FilterOperator) => void;
    setValue: (v: string) => void;
    onblurValue?: () => void;
    valueInput?: HTMLInputElement | HTMLSelectElement | null;
    onSubmitFilter: (event: SubmitEvent) => void;
    categorySearch: string;
    setCategorySearch: (v: string) => void;
    setCategoryInputRef: (el: HTMLInputElement | null) => void;
    onSearchCategories: (event: SubmitEvent) => void;
    categoryValues: CategoryValue[];
    categoriesLoading: boolean;
    categoriesError: string;
    categoryTotal: AggregateCount;
    categoryHasMore: boolean;
    onLoadMore: () => void;
    selectedCategories: string[];
    onToggleCategory: (value: string, checked: boolean) => void;
    onSelectVisible: () => void;
    onClearSelected: () => void;
    count: (value: AggregateCount) => string;
  };

  let {
    column, isText, operators, operator, value, setOperator, setValue, onblurValue, valueInput = $bindable(null), onSubmitFilter,
    categorySearch, setCategorySearch, setCategoryInputRef, onSearchCategories,
    categoryValues, categoriesLoading, categoriesError, categoryTotal, categoryHasMore, onLoadMore,
    selectedCategories, onToggleCategory, onSelectVisible, onClearSelected, count
  }: Props = $props();

  function categoryInputRef(node: HTMLInputElement) {
    setCategoryInputRef(node);
    return { destroy: () => setCategoryInputRef(null) };
  }
</script>

{#if isText}
  <section class="category-picker" aria-label={`Categories for ${column.name}`}>
    <form onsubmit={onSearchCategories}>
      <label>Find a category
        <input use:categoryInputRef type="search" value={categorySearch} oninput={(event) => setCategorySearch((event.currentTarget as HTMLInputElement).value)} placeholder="Search values" />
      </label>
      <Button type="submit" disabled={categoriesLoading}>Search</Button>
    </form>
    <div class="actions">
      <button type="button" onclick={onSelectVisible} disabled={categoriesLoading || categoryValues.length === 0}>Select visible</button>
      <button type="button" onclick={onClearSelected} disabled={selectedCategories.length === 0}>Clear</button>
      <span>{selectedCategories.length} selected</span>
    </div>
    {#if categoriesLoading && categoryValues.length === 0}
      <p class="state"><span class="spinner"></span>Loading values…</p>
    {:else if categoriesError}
      <p class="state error">{categoriesError}</p>
    {:else}
      <div class="list">
        {#each categoryValues as item (item.value)}
          <label class="row">
            <input type="checkbox" checked={selectedCategories.includes(item.value)} onchange={(event) => onToggleCategory(item.value, (event.currentTarget as HTMLInputElement).checked)} />
            <span class="box" aria-hidden="true">{#if selectedCategories.includes(item.value)}✓{/if}</span>
            <span title={item.value}>{item.value}</span>
            <small>{count(item.count)}</small>
          </label>
        {:else}
          <p class="state">No matching values.</p>
        {/each}
      </div>
      <div class="page-row">
        <span>{categoryValues.length.toLocaleString()} / {count(categoryTotal)}</span>
        {#if categoryHasMore}<button type="button" onclick={onLoadMore} disabled={categoriesLoading}>{categoriesLoading ? 'Loading…' : 'Load more'}</button>{/if}
      </div>
    {/if}
  </section>
  <details class="advanced">
    <summary>Advanced condition</summary>
    <FilterOperatorForm {column} {operators} {operator} {value} {setOperator} {setValue} {onblurValue} bind:valueInput onsubmit={onSubmitFilter} />
  </details>
{:else}
  <FilterOperatorForm {column} {operators} {operator} {value} {setOperator} {setValue} {onblurValue} bind:valueInput onsubmit={onSubmitFilter} />
{/if}

<style>
  .category-picker { display: flex; flex-direction: column; gap: 10px; }
  form { display: flex; align-items: flex-end; gap: 8px; }
  form label { flex: 1; display: flex; flex-direction: column; gap: 5px; font-size: 11px; color: var(--muted); }
  form input { height: 28px; padding: 0 9px; border-radius: var(--radius-md); border: 1px solid var(--control-border); font-size: 12px; color: var(--ink); }
  .actions { display: flex; align-items: center; gap: 10px; font-size: 11px; color: var(--muted); }
  .actions button { color: var(--action); background: none; border: none; }
  .actions button:disabled { color: var(--placeholder); }
  .state { font-size: 12px; color: var(--muted); display: flex; align-items: center; gap: 8px; }
  .state.error { color: var(--error); }
  .list { max-height: 260px; overflow-y: auto; display: flex; flex-direction: column; gap: 1px; border: 1px solid var(--line); border-radius: var(--radius-md); padding: 4px; }
  .row { display: flex; align-items: center; gap: 8px; height: 26px; padding: 0 6px; border-radius: var(--radius-sm); cursor: pointer; font-size: 12px; }
  .row:hover { background: var(--surface-hover); }
  .row input { position: absolute; width: 1px; height: 1px; opacity: 0; }
  .box { display: inline-flex; align-items: center; justify-content: center; flex: none; width: 12px; height: 12px; border-radius: 2px; border: 1px solid var(--placeholder); color: #fff; font-size: 8px; }
  input:checked + .box { background: var(--action); border-color: var(--action); }
  .row span:not(.box) { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .row small { color: var(--faint); font-family: var(--font-mono); }
  .page-row { display: flex; align-items: center; justify-content: space-between; font-size: 11px; color: var(--muted); }
  .advanced { margin-top: 14px; border-top: 1px solid var(--line); padding-top: 12px; }
  .advanced summary { font-size: 12px; color: var(--action); cursor: pointer; }
</style>
