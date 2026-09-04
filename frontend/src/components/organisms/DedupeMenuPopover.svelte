<script lang="ts">
  import Button from '../atoms/Button.svelte';
  import Checkbox from '../atoms/Checkbox.svelte';
  import MenuPopover from '../molecules/MenuPopover.svelte';
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

<MenuPopover {open} {ontoggle} icon="duplicate" {label} width={260}>
  {#snippet header()}
    <strong>Dedupe</strong><span>{dedupeDraft.length} keys</span>
  {/snippet}
  <div class="list">
    {#each columns as column (column.name)}
      <Checkbox checked={dedupeDraft.includes(column.name)} label={column.name} title={column.name} onchange={(checked) => onToggle(column.name, checked)} />
    {/each}
  </div>
  <div class="actions">
    <Button onclick={onApply} disabled={dedupeDraft.length === 0}>Apply</Button>
    <Button onclick={onClear} disabled={dedupeAppliedCount === 0 && dedupeDraft.length === 0}>Clear</Button>
  </div>
</MenuPopover>

<style>
  .list { max-height: 260px; overflow-y: auto; display: flex; flex-direction: column; gap: 7px; padding: 2px; }
  .actions { display: flex; gap: 8px; }
</style>
