<script lang="ts">
  import Button from '../atoms/Button.svelte';
  import Checkbox from '../atoms/Checkbox.svelte';

  import MultiSelectDropdown from '../molecules/MultiSelectDropdown.svelte';
  import SelectDropdown from '../molecules/SelectDropdown.svelte';
  import type { AggregateCount, DatasetInfo, JoinRelationship, JoinWorkspaceResponse, NodeInfo } from '../../lib/types';

  type Side = 'left' | 'right';
  type Props = {
    open: boolean;
    ontoggle: (event: Event) => void;
    sources: NodeInfo[];
    joinLeftNodeId: string;
    joinRightNodeId: string;
    onSelectSource: (side: Side, id: string) => void;
    joinLeftDatasets: DatasetInfo[];
    joinRightDatasets: DatasetInfo[];
    joinLeftDatasetsLoading: boolean;
    joinRightDatasetsLoading: boolean;
    joinSelectionError: string;
    joinLeftDataset: DatasetInfo | undefined;
    joinRightDataset: DatasetInfo | undefined;
    joinLeftDatasetId: string;
    joinRightDatasetId: string;
    selectJoinDataset: (side: Side, id: string) => void;
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

    onRun: () => void;
    canRun: boolean;
    running: boolean;
  };

  let {
    open, ontoggle, sources, joinLeftNodeId, joinRightNodeId, onSelectSource,
    joinLeftDatasets, joinRightDatasets, joinLeftDatasetsLoading, joinRightDatasetsLoading,
    joinSelectionError, joinLeftDataset, joinRightDataset, joinLeftDatasetId, joinRightDatasetId, selectJoinDataset,
    joinLeftKeys, joinRightKeys, onSetKeys, joinLeftColumns, joinRightColumns,
    onToggleColumn, onSelectAll, onSelectNone, joinPreview, previewLoading, previewError,
    onCheck, canCheck, count, crossSource, onRun, canRun, running
  }: Props = $props();

  let sourceOptions = $derived(sources.map((source) => ({ value: source.id, label: source.name, description: source.source })));
  let members = $derived([
    { side: 'left' as Side, label: 'First member', sourceId: joinLeftNodeId, datasets: joinLeftDatasets, datasetId: joinLeftDatasetId, loading: joinLeftDatasetsLoading },
    { side: 'right' as Side, label: 'Second member', sourceId: joinRightNodeId, datasets: joinRightDatasets, datasetId: joinRightDatasetId, loading: joinRightDatasetsLoading }
  ]);
  let leftKeyOptions = $derived((joinLeftDataset?.columns ?? []).map((column) => ({ value: column, label: column })));
  let rightKeyOptions = $derived((joinRightDataset?.columns ?? []).map((column) => ({ value: column, label: column })));
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

    <div class="members">
      {#each members as member (member.side)}
        <section class="member">
          <strong>{member.label}</strong>
          <SelectDropdown label="Source" options={sourceOptions} value={member.sourceId} onchange={(id) => onSelectSource(member.side, id)} placeholder="Choose a source" />
          <div class="sheet-pills" aria-label={`${member.label} sheets`}>
            {#if member.loading}
              <span class="member-note">Loading sheets…</span>
            {:else if !member.sourceId}
              <span class="member-note">Choose a source</span>
            {:else if member.datasets.length === 0}
              <span class="member-note">No sheets</span>
            {:else}
              {#each member.datasets as dataset (dataset.id)}
                <Button
                  type="button"
                  active={dataset.id === member.datasetId}
                  title={`${dataset.schema}.${dataset.name}`}
                  style="height: 24px; max-width: 100%; padding: 0 8px; overflow: hidden; border-radius: var(--radius-lg); font: 11px var(--font-mono); text-overflow: ellipsis;"
                  onclick={() => selectJoinDataset(member.side, dataset.id)}
                >{dataset.name}</Button>
              {/each}
            {/if}
          </div>
        </section>
      {/each}
    </div>
    {#if joinSelectionError}<p class="error" role="alert">{joinSelectionError}</p>{/if}

    {#if joinRightDataset && joinLeftDataset}
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
          <header><strong title={joinLeftDataset.name}>{joinLeftDataset.name}</strong><span>{joinLeftColumns.length} selected</span></header>
          <div class="select-actions"><Button type="button" onclick={() => onSelectAll('left')}>All</Button><Button type="button" onclick={() => onSelectNone('left')}>None</Button></div>
          <div class="column-list">
            {#each joinLeftDataset.columns as column (column)}
              <Checkbox checked={joinLeftColumns.includes(column)} label={column} title={column} onchange={(checked) => onToggleColumn('left', column, checked)} />
            {/each}
          </div>
        </section>
        <section>
          <header><strong title={joinRightDataset.name}>{joinRightDataset.name}</strong><span>{joinRightColumns.length} selected</span></header>
          <div class="select-actions"><Button type="button" onclick={() => onSelectAll('right')}>All</Button><Button type="button" onclick={() => onSelectNone('right')}>None</Button></div>
          <div class="column-list">
            {#each joinRightDataset.columns as column (column)}
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


    {/if}
  </div>
</details>

<style>
  .popover-host { position: static; }
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
    /* ponytail: fixed to the stable query-bar row; calculate from the trigger if the shell becomes dynamic. */
    position: fixed; top: 125px; left: clamp(12px, calc((100vw - 620px) / 2), 245px); z-index: 10;
    width: min(620px, calc(100vw - 24px)); min-height: min(600px, calc(100vh - 137px)); max-height: min(680px, calc(100vh - 137px)); overflow-y: auto;
    padding: 12px; border-radius: var(--radius-xl); background: var(--surface);
    border: 1px solid var(--line-strong); box-shadow: var(--shadow-popover-wide);
    display: flex; flex-direction: column; gap: 10px;
  }
  header { display: flex; align-items: baseline; justify-content: space-between; gap: 8px; }
  header span { font: 10px var(--font-mono); color: var(--faint); white-space: nowrap; }
  .members { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 8px; }
  .member { min-width: 0; display: flex; flex-direction: column; gap: 6px; padding: 8px; }
  .member > strong { font-size: 11px; color: var(--ink-2); }
  .sheet-pills { min-height: 24px; display: flex; align-items: center; align-content: flex-start; gap: 4px; flex-wrap: wrap; }
  .member-note { font-size: 10.5px; color: var(--faint); }

  .keys { display: flex; flex-direction: column; gap: 7px; margin: 0; padding: 8px; border: 1px solid var(--line); border-radius: var(--radius-md); }
  legend { padding: 0 4px; font-size: 11px; font-weight: 600; color: var(--muted); }
  .key-selectors { display: grid; grid-template-columns: minmax(0, 1fr) auto minmax(0, 1fr); align-items: end; gap: 7px; }
  .key-selectors > span { padding-bottom: 8px; color: var(--muted); }
  .keys p, .error { margin: 0; font-size: 10.5px; color: var(--muted); }
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

  @media (max-width: 760px) {
    .popover { width: calc(100% - 24px); }
    .members, .columns { grid-template-columns: 1fr; }
    .column-list { max-height: 140px; }
    dl { grid-template-columns: repeat(2, minmax(0, 1fr)); }
  }
</style>
