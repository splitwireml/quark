<script lang="ts">
  import Button from '../atoms/Button.svelte';
  import Icon from '../atoms/Icon.svelte';
  import type { ColumnInfo } from '../../lib/types';

  type Props = {
    open: boolean;
    ontoggle: (event: Event) => void;
    label: string;
    columns: ColumnInfo[];
    dedupeDraft: string[];
    onToggle: (name: string, checked: boolean) => void;
    onApply: () => void;
    onClear: () => void;
    dedupeAppliedCount: number;
  };
  let { open, ontoggle, label, columns, dedupeDraft, onToggle, onApply, onClear, dedupeAppliedCount }: Props = $props();
</script>

<details class="popover-host" {open} {ontoggle}>
  <summary class="trigger menu-trigger" class:active={open}><Icon name="duplicate" size={14} /><span class="menu-label"><span>{label}</span></span></summary>
  <div class="popover">
    <div class="list">
      {#each columns as column (column.name)}
        <label class="row">
          <input type="checkbox" checked={dedupeDraft.includes(column.name)} onchange={(event) => onToggle(column.name, (event.currentTarget as HTMLInputElement).checked)} />
          <span class="box" aria-hidden="true">{#if dedupeDraft.includes(column.name)}✓{/if}</span>
          {column.name}
        </label>
      {/each}
    </div>
    <div class="actions">
      <Button onclick={onApply} disabled={dedupeDraft.length === 0}>Apply</Button>
      <Button onclick={onClear} disabled={dedupeAppliedCount === 0 && dedupeDraft.length === 0}>Clear</Button>
    </div>
  </div>
</details>

<style>
  .popover-host { position: relative; }
  summary { list-style: none; cursor: pointer; }
  summary::-webkit-details-marker { display: none; }
  .popover {
    position: absolute; top: calc(100% + 6px); left: 0; z-index: 10;
    width: 260px; padding: 10px;
    border-radius: var(--radius-xl);
    background: var(--surface);
    border: 1px solid var(--line-strong);
    box-shadow: var(--shadow-popover);
    display: flex; flex-direction: column; gap: 10px;
  }
  .list { max-height: 260px; overflow-y: auto; display: flex; flex-direction: column; gap: 1px; }
  .row { display: flex; align-items: center; gap: 8px; height: 28px; padding: 0 6px; border-radius: var(--radius-md); cursor: pointer; font-size: 12px; color: var(--ink-2); }
  .row:hover { background: var(--surface-hover); }
  .row input { position: absolute; width: 1px; height: 1px; opacity: 0; }
  .box {
    display: inline-flex; align-items: center; justify-content: center; flex: none;
    width: 12px; height: 12px; border-radius: 2px; border: 1px solid var(--glyph);
    color: #fff; font-size: 8px;
  }
  input:checked + .box { background: var(--action); border-color: var(--action); }
  .actions { display: flex; gap: 8px; }
</style>
