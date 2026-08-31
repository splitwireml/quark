<script lang="ts">
  import Button from '../atoms/Button.svelte';
  import Checkbox from '../atoms/Checkbox.svelte';
  import type { DatasetInfo } from '../../lib/types';
  import type { JoinKey } from '../../lib/join-sql';

  type Side = 'left' | 'right';
  type Props = {
    open: boolean;
    ontoggle: (event: Event) => void;
    datasets: DatasetInfo[];
    leftDataset: DatasetInfo | undefined;
    rightDataset: DatasetInfo | undefined;
    joinDataset: string;
    onSelectDataset: (id: string) => void;
    joinKeys: JoinKey[];
    onUpdateKey: (index: number, side: keyof JoinKey, value: string) => void;
    onAddKey: () => void;
    onRemoveKey: (index: number) => void;
    joinLeftColumns: string[];
    joinRightColumns: string[];
    onToggleColumn: (side: Side, column: string, checked: boolean) => void;
    onSelectAll: (side: Side) => void;
    onSelectNone: (side: Side) => void;
    saveJoinView: boolean;
    setSaveJoinView: (value: boolean) => void;
    joinViewName: string;
    setJoinViewName: (value: string) => void;
    onRun: () => void;
    canRun: boolean;
    running: boolean;
  };

  let {
    open, ontoggle, datasets, leftDataset, rightDataset, joinDataset, onSelectDataset,
    joinKeys, onUpdateKey, onAddKey, onRemoveKey, joinLeftColumns, joinRightColumns,
    onToggleColumn, onSelectAll, onSelectNone, saveJoinView, setSaveJoinView,
    joinViewName, setJoinViewName, onRun, canRun, running
  }: Props = $props();
</script>

<details class="popover-host" {open} {ontoggle}>
  <summary class="trigger" class:active={open}>Joins</summary>
  <div class="popover">
    <header><strong>INNER JOIN</strong><span>same source</span></header>
    <label class="field">Dataset
      <select value={joinDataset} onchange={(event) => onSelectDataset((event.currentTarget as HTMLSelectElement).value)}>
        <option value="">Choose another dataset</option>
        {#each datasets as dataset (dataset.id)}
          <option value={dataset.id}>{dataset.schema}.{dataset.name}</option>
        {/each}
      </select>
    </label>

    {#if rightDataset && leftDataset}
      <fieldset class="keys">
        <legend>Equality keys</legend>
        {#each joinKeys as key, index (`${index}-${key.left}-${key.right}`)}
          <div class="key-row">
            <select aria-label={`Left key ${index + 1}`} value={key.left} onchange={(event) => onUpdateKey(index, 'left', (event.currentTarget as HTMLSelectElement).value)}>
              <option value="">Left column</option>
              {#each leftDataset.columns as column (column)}<option value={column}>{column}</option>{/each}
            </select>
            <span>=</span>
            <select aria-label={`Right key ${index + 1}`} value={key.right} onchange={(event) => onUpdateKey(index, 'right', (event.currentTarget as HTMLSelectElement).value)}>
              <option value="">Right column</option>
              {#each rightDataset.columns as column (column)}<option value={column}>{column}</option>{/each}
            </select>
            <Button variant="ghost" type="button" aria-label={`Remove key pair ${index + 1}`} onclick={() => onRemoveKey(index)}>×</Button>
          </div>
        {/each}
        <Button type="button" onclick={onAddKey}>Add key pair</Button>
      </fieldset>

      <div class="columns">
        <section>
          <header><strong title={leftDataset.name}>{leftDataset.name}</strong><span>{joinLeftColumns.length} selected</span></header>
          <div class="select-actions"><Button type="button" onclick={() => onSelectAll('left')}>All</Button><Button type="button" onclick={() => onSelectNone('left')}>None</Button></div>
          <div class="column-list">
            {#each leftDataset.columns as column (column)}
              <Checkbox checked={joinLeftColumns.includes(column)} label={column} title={column} onchange={(checked) => onToggleColumn('left', column, checked)} />
            {/each}
          </div>
        </section>
        <section>
          <header><strong title={rightDataset.name}>{rightDataset.name}</strong><span>{joinRightColumns.length} selected</span></header>
          <div class="select-actions"><Button type="button" onclick={() => onSelectAll('right')}>All</Button><Button type="button" onclick={() => onSelectNone('right')}>None</Button></div>
          <div class="column-list">
            {#each rightDataset.columns as column (column)}
              <Checkbox checked={joinRightColumns.includes(column)} label={column} title={column} onchange={(checked) => onToggleColumn('right', column, checked)} />
            {/each}
          </div>
        </section>
      </div>

      <Checkbox checked={saveJoinView} label="Save view in this browser" onchange={setSaveJoinView} />
      {#if saveJoinView}
        <label class="field">Optional view name
          <input type="text" maxlength="64" value={joinViewName} oninput={(event) => setJoinViewName((event.currentTarget as HTMLInputElement).value)} placeholder="Joined view" />
        </label>
      {/if}
      <Button variant="primary" type="button" onclick={onRun} disabled={running || !canRun}>{running ? 'Running…' : 'Run INNER JOIN'}</Button>
    {/if}
  </div>
</details>

<style>
  .popover-host { position: relative; }
  summary { list-style: none; cursor: pointer; }
  summary::-webkit-details-marker { display: none; }
  .trigger {
    display: inline-flex; align-items: center; height: 30px; padding: 0 12px;
    border-radius: var(--radius-lg); border: 1px solid var(--control-border);
    background: var(--surface); font-size: 12.5px; font-weight: 500; color: var(--ink-2);
  }
  .trigger:hover { border-color: var(--faint); }
  .trigger.active { border-color: var(--action); background: var(--action-tint); color: var(--action-dark); }
  .popover {
    position: absolute; top: calc(100% + 6px); left: 0; z-index: 10;
    width: min(620px, calc(100vw - 32px)); max-height: min(80vh, 640px); overflow-y: auto;
    padding: 12px; border-radius: var(--radius-xl); background: var(--surface);
    border: 1px solid var(--line-strong); box-shadow: var(--shadow-popover-wide);
    display: flex; flex-direction: column; gap: 10px;
  }
  header { display: flex; align-items: baseline; justify-content: space-between; gap: 8px; }
  header span { font: 10px var(--font-mono); color: var(--faint); white-space: nowrap; }
  .field { display: flex; flex-direction: column; gap: 5px; font-size: 11px; color: var(--muted); }
  .field select, .field input, .key-row select { height: 28px; min-width: 0; padding: 0 8px; border-radius: var(--radius-md); border: 1px solid var(--control-border); font-size: 12px; }
  .keys { display: flex; flex-direction: column; gap: 6px; margin: 0; padding: 8px; border: 1px solid var(--line); border-radius: var(--radius-md); }
  legend { padding: 0 4px; font-size: 11px; font-weight: 600; color: var(--muted); }
  .key-row { display: grid; grid-template-columns: minmax(0, 1fr) auto minmax(0, 1fr) auto; align-items: center; gap: 6px; }
  .columns { min-height: 0; display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 10px; }
  .columns section { min-width: 0; border: 1px solid var(--line); border-radius: var(--radius-md); overflow: hidden; }
  .columns header { padding: 8px; }
  .columns header strong { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .select-actions { display: flex; gap: 6px; padding: 0 8px 8px; }
  .column-list { max-height: 200px; overflow-y: auto; display: flex; flex-direction: column; gap: 8px; padding: 8px; border-top: 1px solid var(--line); }
  @media (max-width: 760px) {
    .popover { width: calc(100vw - 24px); }
    .columns { grid-template-columns: 1fr; }
    .column-list { max-height: 140px; }
  }
</style>
