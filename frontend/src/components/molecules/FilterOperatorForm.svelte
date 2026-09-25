<script lang="ts">
  import Button from '../atoms/Button.svelte';
  import type { ColumnInfo, FilterOperator } from '../../lib/types';

  type Props = {
    column: ColumnInfo;
    operators: { value: FilterOperator; label: string }[];
    operator: FilterOperator;
    value: string;
    upperValue: string;
    setOperator: (value: FilterOperator) => void;
    setValue: (value: string) => void;
    setUpperValue: (value: string) => void;
    onblurValue?: () => void;
    onblurUpperValue?: () => void;
    valueInput?: HTMLInputElement | HTMLSelectElement | null;
    onsubmit: (event: SubmitEvent) => void;
  };

  let { column, operators, operator, value, upperValue, setOperator, setValue, setUpperValue, onblurValue, onblurUpperValue, valueInput = $bindable(null), onsubmit }: Props = $props();
  let noValue = $derived(operator === 'is_null' || operator === 'not_null');
  let isBoolean = $derived(column.type.toLowerCase() === 'boolean');
</script>

<form class="filter-form" {onsubmit}>
  <label>Operator
    <select value={operator} onchange={(event) => setOperator((event.currentTarget as HTMLSelectElement).value as FilterOperator)}>
      {#each operators as op (op.value)}<option value={op.value}>{op.label}</option>{/each}
    </select>
  </label>
  {#if !noValue}
    <label>{operator === 'between' ? 'From' : 'Value'}
      {#if isBoolean}
        <select bind:this={valueInput} value={value} onchange={(event) => setValue((event.currentTarget as HTMLSelectElement).value)}>
          <option value="" disabled>Select value</option>
          <option value="true">True</option>
          <option value="false">False</option>
        </select>
      {:else}
        <input
          bind:this={valueInput}
          type="text"
          inputmode={column.numeric ? 'decimal' : undefined}
          value={value}
          oninput={(event) => setValue((event.currentTarget as HTMLInputElement).value)}
          onblur={onblurValue}
        />
      {/if}
    </label>
    {#if operator === 'between'}
      <label>To
        <input type="text" inputmode={column.numeric ? 'decimal' : undefined} value={upperValue} oninput={(event) => setUpperValue(event.currentTarget.value)} onblur={onblurUpperValue} />
      </label>
    {/if}
  {/if}
  <Button variant="primary" type="submit" disabled={!noValue && (value === '' || (operator === 'between' && upperValue === ''))}>Apply filter</Button>
</form>

<style>
  .filter-form { display: flex; flex-direction: column; gap: 10px; }
  label { display: flex; flex-direction: column; gap: 5px; font-size: 11px; color: var(--muted); }
  select, input {
    height: 30px;
    padding: 0 9px;
    border-radius: var(--radius-md);
    border: 1px solid var(--control-border);
    background: var(--surface);
    font-size: 12.5px;
    color: var(--ink);
  }
</style>
