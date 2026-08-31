<script lang="ts">
  import Button from '../atoms/Button.svelte';
  import Checkbox from '../atoms/Checkbox.svelte';
  import TextInput from '../atoms/TextInput.svelte';
  import MultiSelectDropdown from '../molecules/MultiSelectDropdown.svelte';
  import SelectDropdown from '../molecules/SelectDropdown.svelte';
  import type { AggregateCount, DatasetInfo, JoinRelationship, JoinWorkspaceResponse, NodeInfo } from '../../lib/types';

  type Side = 'left' | 'right';
  type Props = {
    open: boolean;
    ontoggle: (event: Event) => void;
    sources: NodeInfo[];
    rightSourceId: string;
    onSelectSource: (id: string) => void;
    rightDatasets: DatasetInfo[];
    datasetsLoading: boolean;
    joinSelectionError: string;
    leftDataset: DatasetInfo | undefined;
    rightDataset: DatasetInfo | undefined;
    joinDataset: string;
    onSelectDataset: (id: string) => void;
    joinLeftKeys: string[];
    joinRightKeys: string[];
    onSetKeys: (side: Side, columns: string[]) => void;
    joinLeftColumns: string[];
    joinRightColumns: string[];
    onToggleColumn: (side: Side, column: string, checked: boolean) => void;
    onSelectAll: (side: Side) => void;
    onSelectNone: (side: Side) => void;
    joinPreview: JoinWorkspaceResponse | null;
    previewLoading: boolean;
    previewError: string;
    onCheck: () => void;
    canCheck: boolean;
    count: (value: AggregateCount) => string;
    crossSource: boolean;
    saveJoinView: boolean;
    setSaveJoinView: (value: boolean) => void;
    joinViewName: string;
    setJoinViewName: (value: string) => void;
    onRun: () => void;
    canRun: boolean;
    running: boolean;
  };

  let {
    open, ontoggle, sources, rightSourceId, onSelectSource, rightDatasets, datasetsLoading,
    joinSelectionError, leftDataset, rightDataset, joinDataset, onSelectDataset,
    joinLeftKeys, joinRightKeys, onSetKeys, joinLeftColumns, joinRightColumns,
    onToggleColumn, onSelectAll, onSelectNone, joinPreview, previewLoading, previewError,
    onCheck, canCheck, count, crossSource, saveJoinView, setSaveJoinView,
    joinViewName, setJoinViewName, onRun, canRun, running
  }: Props = $props();

  let sourceOptions = $derived(sources.map((source) => ({ value: source.id, label: source.name, description: source.source })));
  let datasetOptions = $derived(rightDatasets.map((dataset) => ({ value: dataset.id, label: `${dataset.schema}.${dataset.name}`, description: dataset.type })));
  let leftKeyOptions = $derived((leftDataset?.columns ?? []).map((column) => ({ value: column, label: column })));
  let rightKeyOptions = $derived((rightDataset?.columns ?? []).map((column) => ({ value: column, label: column })));
  let keysValid = $derived(joinLeftKeys.length > 0 && joinLeftKeys.length === joinRightKeys.length);

  const relationshipLabels: Record<JoinRelationship, string> = {
    cartesian: 'Cartesian product',
    one_to_one: 'One to one',
    one_to_many: 'One to many',
    many_to_one: 'Many to one',
    many_to_many: 'Many to many'
  };
</script>

<details class="popover-host" {open} {ontoggle}>
  <summary class="trigger" class:active={open}>Joins</summary>
  <div class="popover">
    <header><strong>INNER JOIN</strong><span>{crossSource ? 'cross source' : 'same source'}</span></header>

    <div class="selectors">
      <SelectDropdown label="Source" options={sourceOptions} value={rightSourceId} onchange={onSelectSource} placeholder="Choose a source" />
      <SelectDropdown label="Sheet" options={datasetOptions} value={joinDataset} onchange={onSelectDataset} placeholder={datasetsLoading ? 'Loading sheets…' : 'Choose a sheet'} disabled={!rightSourceId || datasetsLoading} />
    </div>
    {#if joinSelectionError}<p class="error" role="alert">{joinSelectionError}</p>{/if}

    {#if rightDataset && leftDataset}
      <fieldset class="keys">
        <legend>Equality keys</legend>
        <div class="key-selectors">
          <MultiSelectDropdown label="Left key" options={leftKeyOptions} selected={joinLeftKeys} onchange={(columns) => onSetKeys('left', columns)} placeholder="Choose columns" />
          <span aria-hidden="true">=</span>
          <MultiSelectDropdown label="Right key" options={rightKeyOptions} selected={joinRightKeys} onchange={(columns) => onSetKeys('right', columns)} placeholder="Choose columns" />
        </div>
        <p class:invalid={!keysValid}>Selected columns pair by order. Choose the same non-zero number on both sides to run. ({joinLeftKeys.length} left, {joinRightKeys.length} right)</p>
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

      <div class="preview-actions">
        <Button type="button" onclick={onCheck} disabled={!canCheck || previewLoading || running}>{previewLoading ? 'Checking…' : 'Check join'}</Button>
        <Button variant="primary" type="button" onclick={onRun} disabled={running || previewLoading || !canRun}>{running ? 'Running…' : 'Run INNER JOIN'}</Button>
      </div>
      {#if previewError}<p class="error" role="alert">{previewError}</p>{/if}
      {#if joinPreview}
        <section class="preview" aria-label="Join cardinality preview">
          <header><strong>Join preview</strong>{#if joinPreview.cartesian_risk}<span class="risk">Cartesian risk</span>{/if}</header>
          <dl>
            <div><dt>Left rows</dt><dd>{count(joinPreview.left_rows)}</dd></div>
            <div><dt>Right rows</dt><dd>{count(joinPreview.right_rows)}</dd></div>
            <div><dt>Output rows</dt><dd>{count(joinPreview.output_rows)}</dd></div>
            <div><dt>Relationship</dt><dd>{relationshipLabels[joinPreview.relationship]}</dd></div>
          </dl>
        </section>
      {/if}

      <div class:disabled={crossSource} inert={crossSource} aria-disabled={crossSource}>
        <Checkbox checked={crossSource ? false : saveJoinView} label="Save view in this browser" onchange={setSaveJoinView} />
      </div>
      {#if crossSource}<p class="session-note">Session-only cross-source view</p>{/if}
      {#if saveJoinView && !crossSource}
        <label class="field">Optional view name
          <TextInput type="text" maxlength="64" value={joinViewName} oninput={(event: Event) => setJoinViewName((event.currentTarget as HTMLInputElement).value)} placeholder="Joined view" />
        </label>
      {/if}
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
    width: min(620px, calc(100vw - 32px)); min-height: min(600px, 80vh); max-height: min(80vh, 680px); overflow-y: auto;
    padding: 12px; border-radius: var(--radius-xl); background: var(--surface);
    border: 1px solid var(--line-strong); box-shadow: var(--shadow-popover-wide);
    display: flex; flex-direction: column; gap: 10px;
  }
  header { display: flex; align-items: baseline; justify-content: space-between; gap: 8px; }
  header span { font: 10px var(--font-mono); color: var(--faint); white-space: nowrap; }
  .selectors { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 8px; }
  .field { display: flex; flex-direction: column; gap: 5px; font-size: 11px; color: var(--muted); }
  .keys { display: flex; flex-direction: column; gap: 7px; margin: 0; padding: 8px; border: 1px solid var(--line); border-radius: var(--radius-md); }
  legend { padding: 0 4px; font-size: 11px; font-weight: 600; color: var(--muted); }
  .key-selectors { display: grid; grid-template-columns: minmax(0, 1fr) auto minmax(0, 1fr); align-items: end; gap: 7px; }
  .key-selectors > span { padding-bottom: 8px; color: var(--muted); }
  .keys p, .error, .session-note { margin: 0; font-size: 10.5px; color: var(--muted); }
  .keys p.invalid, .error { color: var(--error); }
  .columns { min-height: 0; display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 10px; }
  .columns section { min-width: 0; border: 1px solid var(--line); border-radius: var(--radius-md); overflow: hidden; }
  .columns header { padding: 8px; }
  .columns header strong { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .select-actions { display: flex; gap: 6px; padding: 0 8px 8px; }
  .column-list { max-height: 200px; overflow-y: auto; display: flex; flex-direction: column; gap: 8px; padding: 8px; border-top: 1px solid var(--line); }
  .preview-actions { display: flex; justify-content: flex-end; gap: 8px; }
  .preview { padding: 9px; border: 1px solid var(--line); border-radius: var(--radius-md); background: var(--surface-inset); }
  .preview header { margin-bottom: 8px; }
  .preview .risk { padding: 2px 6px; border: 1px solid var(--warning-fill); border-radius: 999px; color: var(--warning); }
  dl { display: grid; grid-template-columns: repeat(4, minmax(0, 1fr)); gap: 8px; margin: 0; }
  dl div { min-width: 0; }
  dt { margin-bottom: 2px; font-size: 10px; color: var(--muted); }
  dd { margin: 0; overflow-wrap: anywhere; font: 11px var(--font-mono); color: var(--ink); }
  .disabled { opacity: 0.5; }
  .session-note { margin-top: -7px; }
  @media (max-width: 760px) {
    .popover { width: calc(100vw - 24px); }
    .selectors, .columns { grid-template-columns: 1fr; }
    .column-list { max-height: 140px; }
    dl { grid-template-columns: repeat(2, minmax(0, 1fr)); }
  }
</style>
