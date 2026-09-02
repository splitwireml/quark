<script lang="ts">
  import Button from '../atoms/Button.svelte';
  import Checkbox from '../atoms/Checkbox.svelte';
  import MultiSelectDropdown from '../molecules/MultiSelectDropdown.svelte';
  import SelectDropdown from '../molecules/SelectDropdown.svelte';
  import type { AggregateCount, JoinRelationship, JoinWorkspaceResponse, ViewHistory } from '../../lib/types';

  type Side = 'left' | 'right';
  type Props = {
    open: boolean;
    ontoggle: (event: Event) => void;
    views: ViewHistory[];
    joinLeftViewId: string;
    joinRightViewId: string;
    onSelectView: (side: Side, id: string) => void;
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
    onRun: () => void;
    canRun: boolean;
    running: boolean;
  };

  let {
    open, ontoggle, views, joinLeftViewId, joinRightViewId, onSelectView,
    joinLeftKeys, joinRightKeys, onSetKeys, joinLeftColumns, joinRightColumns,
    onToggleColumn, onSelectAll, onSelectNone, joinPreview, previewLoading, previewError,
    onCheck, canCheck, count, onRun, canRun, running
  }: Props = $props();

  const activeVersion = (view: ViewHistory | undefined) => view?.versions.find((version) => version.id === view.activeVersionId);
  let viewOptions = $derived(views.map((view) => ({ value: view.id, label: view.name, description: `v${activeVersion(view)?.number ?? 1}` })));
  let leftView = $derived(views.find((view) => view.id === joinLeftViewId));
  let rightView = $derived(views.find((view) => view.id === joinRightViewId));
  let leftColumns = $derived(activeVersion(leftView)?.columns ?? []);
  let rightColumns = $derived(activeVersion(rightView)?.columns ?? []);
  let leftKeyOptions = $derived(leftColumns.map((column) => ({ value: column, label: column })));
  let rightKeyOptions = $derived(rightColumns.map((column) => ({ value: column, label: column })));
  let keysValid = $derived(joinLeftKeys.length > 0 && joinLeftKeys.length === joinRightKeys.length);

  const relationshipLabels: Record<JoinRelationship, string> = {
    cartesian: 'Cartesian product', one_to_one: 'One to one', one_to_many: 'One to many',
    many_to_one: 'Many to one', many_to_many: 'Many to many'
  };
</script>

<details class="popover-host" {open} {ontoggle}>
  <summary class="trigger" class:active={open}>Joins</summary>
  <div class="popover">
    <header><strong>INNER JOIN</strong><span>project Views</span></header>
    <div class="members">
      <SelectDropdown label="Left View" options={viewOptions} value={joinLeftViewId} onchange={(id) => onSelectView('left', id)} placeholder="Choose a View" />
      <SelectDropdown label="Right View" options={viewOptions} value={joinRightViewId} onchange={(id) => onSelectView('right', id)} placeholder="Choose a View" />
    </div>

    {#if leftView && rightView}
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
          <header><strong title={leftView.name}>{leftView.name}</strong><span>{joinLeftColumns.length} selected</span></header>
          <div class="select-actions"><Button type="button" onclick={() => onSelectAll('left')}>All</Button><Button type="button" onclick={() => onSelectNone('left')}>None</Button></div>
          <div class="column-list">
            {#each leftColumns as column (column)}
              <Checkbox checked={joinLeftColumns.includes(column)} label={column} title={column} onchange={(checked) => onToggleColumn('left', column, checked)} />
            {/each}
          </div>
        </section>
        <section>
          <header><strong title={rightView.name}>{rightView.name}</strong><span>{joinRightColumns.length} selected</span></header>
          <div class="select-actions"><Button type="button" onclick={() => onSelectAll('right')}>All</Button><Button type="button" onclick={() => onSelectNone('right')}>None</Button></div>
          <div class="column-list">
            {#each rightColumns as column (column)}
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
    {:else}
      <p class="member-note">Choose two Views from this project.</p>
    {/if}
  </div>
</details>

<style>
  .popover-host { position: static; }
  summary { list-style: none; cursor: pointer; }
  summary::-webkit-details-marker { display: none; }
  .trigger { display: inline-flex; align-items: center; height: 30px; padding: 0 12px; border-radius: var(--radius-lg); border: 1px solid var(--control-border); background: var(--surface); font-size: 12.5px; font-weight: 500; color: var(--ink-2); }
  .trigger:hover { border-color: var(--faint); }
  .trigger.active { border-color: var(--action); background: var(--action-tint); color: var(--action-dark); }
  .popover { position: fixed; top: 125px; left: clamp(12px, calc((100vw - 620px) / 2), 245px); z-index: 10; width: min(620px, calc(100vw - 24px)); min-height: min(540px, calc(100vh - 137px)); max-height: min(680px, calc(100vh - 137px)); overflow-y: auto; padding: 12px; border-radius: var(--radius-xl); background: var(--surface); border: 1px solid var(--line-strong); box-shadow: var(--shadow-popover-wide); display: flex; flex-direction: column; gap: 10px; }
  header { display: flex; align-items: baseline; justify-content: space-between; gap: 8px; }
  header span { font: 10px var(--font-mono); color: var(--faint); white-space: nowrap; }
  .members { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 10px; padding: 8px; }
  .member-note { margin: 0; padding: 12px 8px; font-size: 10.5px; color: var(--faint); }
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
  @media (max-width: 760px) { .popover { width: calc(100% - 24px); } .members, .columns { grid-template-columns: 1fr; } .column-list { max-height: 140px; } dl { grid-template-columns: repeat(2, minmax(0, 1fr)); } }
</style>
