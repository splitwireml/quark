<script lang="ts">
  import { tick } from 'svelte';
  import Button from '../atoms/Button.svelte';
  import Checkbox from '../atoms/Checkbox.svelte';
  import Chip from '../atoms/Chip.svelte';
  import Icon from '../atoms/Icon.svelte';
  import TextInput from '../atoms/TextInput.svelte';
  import MenuPopover from '../molecules/MenuPopover.svelte';
  import MultiSelectDropdown from '../molecules/MultiSelectDropdown.svelte';
  import type { AggregateCount, JoinRelationship, JoinWorkspaceResponse, SourceSummary, ViewHistory } from '../../lib/types';

  type Side = 'left' | 'right';
  type Step = 0 | 1 | 2;
  type Props = {
    open: boolean;
    ontoggle: (event: Event) => void;
    step: Step;
    direction: -1 | 1;
    sourceSide: Side;
    onSetSourceSide: (side: Side) => void;
    onStep: (step: Step) => Promise<boolean>;
    sources: SourceSummary[];
    views: ViewHistory[];
    joinLeftViewId: string;
    joinRightViewId: string;
    joinLeftSourceId: string;
    joinRightSourceId: string;
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
    count: (value: AggregateCount) => string;
    onRun: () => void;
    canRun: boolean;
    running: boolean;
    preparing: boolean;
  };

  let {
    open, ontoggle, step, direction, sourceSide, onSetSourceSide, onStep, sources, views,
    joinLeftViewId, joinRightViewId, joinLeftSourceId, joinRightSourceId, joinLeftKeys, joinRightKeys, onSetKeys,
    joinLeftColumns, joinRightColumns, onToggleColumn, onSelectAll, onSelectNone,
    joinPreview, previewLoading, previewError, onCheck, count, onRun, canRun, running, preparing
  }: Props = $props();

  const stepNames = ['Views', 'Match rows', 'Carry over'] as const;
  const activeVersion = (view: ViewHistory | undefined) => view?.versions.find((version) => version.id === view.activeVersionId);
  const versionNumber = (view: ViewHistory | undefined) => activeVersion(view)?.number ?? 1;
  const options = (columns: string[]) => columns.map((column) => ({ value: column, label: column }));

  let carrySearch = $state('');
  let stageFrame: HTMLDivElement;
  let leftView = $derived(views.find((view) => view.id === joinLeftViewId));
  let rightView = $derived(views.find((view) => view.id === joinRightViewId));
  let leftName = $derived(leftView?.name ?? sources.find((source) => source.id === joinLeftSourceId)?.name);
  let rightName = $derived(rightView?.name ?? sources.find((source) => source.id === joinRightSourceId)?.name);
  let leftColumns = $derived(activeVersion(leftView)?.columns ?? []);
  let rightColumns = $derived(activeVersion(rightView)?.columns ?? []);
  let keysValid = $derived(joinLeftKeys.length > 0 && joinLeftKeys.length === joinRightKeys.length);
  let carryQuery = $derived(carrySearch.trim().toLowerCase());
  let leftCarryColumns = $derived(carryQuery ? leftColumns.filter((column) => column.toLowerCase().includes(carryQuery)) : leftColumns);
  let rightCarryColumns = $derived(carryQuery ? rightColumns.filter((column) => column.toLowerCase().includes(carryQuery)) : rightColumns);

  const relationshipLabels: Record<JoinRelationship, string> = {
    cartesian: 'Cartesian product', one_to_one: 'One to one', one_to_many: 'One to many',
    many_to_one: 'Many to one', many_to_many: 'Many to many'
  };

  async function changeStep(nextStep: Step) {
    const from = stageFrame.getBoundingClientRect().height;
    stageFrame.style.height = `${from}px`;
    stageFrame.style.overflow = 'hidden';
    if (!await onStep(nextStep)) {
      stageFrame.style.removeProperty('height');
      stageFrame.style.removeProperty('overflow');
      return;
    }
    await tick();

    const to = stageFrame.firstElementChild?.scrollHeight ?? from;
    if (Math.abs(to - from) < 1 || matchMedia('(prefers-reduced-motion: reduce)').matches) {
      stageFrame.style.removeProperty('height');
      stageFrame.style.removeProperty('overflow');
      return;
    }

    stageFrame.style.height = `${to}px`;
  }

  function finishMorph(event: TransitionEvent) {
    if (event.propertyName !== 'height' || event.target !== stageFrame) return;
    stageFrame.style.removeProperty('height');
    stageFrame.style.removeProperty('overflow');
  }

  function advance() {
    if (step === 1) onCheck();
    void changeStep((step + 1) as Step);
  }
</script>

<MenuPopover {open} {ontoggle} icon="join" label="Join" width={468} flush --menu-overflow="visible" --menu-body-overflow="visible">
  {#snippet header()}
    <strong>Join</strong><span>{step + 1} of 3 · {stepNames[step]}</span>
  {/snippet}

  <div class="stage-frame" bind:this={stageFrame} ontransitionend={finishMorph}>
    {#key step}
      <section class="stage" class:backward={direction < 0} aria-label={`Join step ${step + 1}: ${stepNames[step]}`}>
        {#if step === 0}
          <div class="stage-heading">
            <h3>Choose two Views</h3>
            <p>Select a side, then pick a View from Sources.</p>
          </div>
          <div class="source-pair" role="group" aria-label="Join Views">
            <button type="button" class="source-slot" class:active={sourceSide === 'left'} aria-pressed={sourceSide === 'left'} onclick={() => onSetSourceSide('left')}>
              <span>Left View</span>
              <strong title={leftName}>{leftName ?? 'Choose from Sources'}</strong>
              {#if leftView}<small>Version {versionNumber(leftView)}</small>{/if}
            </button>
            <span class="join-mark" aria-hidden="true"><Icon name="join" size={16} /></span>
            <button type="button" class="source-slot" class:active={sourceSide === 'right'} aria-pressed={sourceSide === 'right'} onclick={() => onSetSourceSide('right')}>
              <span>Right View</span>
              <strong title={rightName}>{rightName ?? 'Choose from Sources'}</strong>
              {#if rightView}<small>Version {versionNumber(rightView)}</small>{/if}
            </button>
          </div>
          <p class="source-note"><span aria-hidden="true"></span>Sources is ready for the {sourceSide} View.</p>
        {:else if step === 1}
          <div class="stage-heading">
            <h3>Match rows</h3>
            <p>Choose columns whose values identify the same row.</p>
          </div>
          <div class="key-pair">
            <MultiSelectDropdown label={`Left · ${leftView?.name ?? ''}`} options={options(leftColumns)} selected={joinLeftKeys} onchange={(columns) => onSetKeys('left', columns)} />
            <span class="equals" aria-hidden="true">=</span>
            <MultiSelectDropdown label={`Right · ${rightView?.name ?? ''}`} options={options(rightColumns)} selected={joinRightKeys} onchange={(columns) => onSetKeys('right', columns)} />
          </div>
          <p class="key-note" class:invalid={joinLeftKeys.length > 0 || joinRightKeys.length > 0 ? !keysValid : false}>
            {#if joinLeftKeys.length === 0 && joinRightKeys.length === 0}
              Choose at least one column on each side.
            {:else if keysValid}
              {joinLeftKeys.length} {joinLeftKeys.length === 1 ? 'pair' : 'pairs'} will be compared in selection order.
            {:else}
              Choose the same number of columns on both sides.
            {/if}
          </p>
        {:else}
          <div class="stage-heading carry-heading">
            <div>
              <h3>Carry over columns</h3>
              <p>Choose what the joined View keeps.</p>
            </div>
            <span>{joinLeftColumns.length + joinRightColumns.length} selected</span>
          </div>
          <label class="carry-search">
            <span class="sr-only">Search carry-over columns</span>
            <TextInput type="search" glyph="⌕" value={carrySearch} oninput={(event: Event) => carrySearch = (event.currentTarget as HTMLInputElement).value} placeholder="Search columns" />
          </label>
          <div class="carry-list">
            <div class="column-group">
              <div class="group-head">
                <strong title={leftView?.name}>{leftView?.name}</strong>
                <span>{joinLeftColumns.length}/{leftColumns.length}</span>
                <button type="button" onclick={() => onSelectAll('left')}>All</button>
                <button type="button" onclick={() => onSelectNone('left')}>None</button>
              </div>
              {#each leftCarryColumns as column (column)}
                <div class="column-row"><Checkbox checked={joinLeftColumns.includes(column)} label={column} title={column} onchange={(checked) => onToggleColumn('left', column, checked)} /></div>
              {/each}
            </div>
            <div class="column-group">
              <div class="group-head">
                <strong title={rightView?.name}>{rightView?.name}</strong>
                <span>{joinRightColumns.length}/{rightColumns.length}</span>
                <button type="button" onclick={() => onSelectAll('right')}>All</button>
                <button type="button" onclick={() => onSelectNone('right')}>None</button>
              </div>
              {#each rightCarryColumns as column (column)}
                <div class="column-row"><Checkbox checked={joinRightColumns.includes(column)} label={column} title={column} onchange={(checked) => onToggleColumn('right', column, checked)} /></div>
              {/each}
            </div>
            {#if leftCarryColumns.length === 0 && rightCarryColumns.length === 0}<p class="empty">No matching columns</p>{/if}
          </div>
          <div class="preview" aria-live="polite">
            {#if previewLoading}
              <span class="spinner"></span><span>Checking join…</span>
            {:else if joinPreview}
              <span><b>{count(joinPreview.output_rows)}</b> output rows</span>
              <span><b>{relationshipLabels[joinPreview.relationship]}</b> relationship</span>
              {#if joinPreview.cartesian_risk}<Chip tone="sort">Cartesian risk</Chip>{/if}
            {:else}
              <span>Join check pending</span>
            {/if}
          </div>
        {/if}
      </section>
    {/key}
  </div>

  {#snippet footer()}
    {#if previewError}
      <span class="footer-status error" role="alert">{previewError}</span>
    {:else}
      <span class="footer-status">{stepNames[step]}</span>
    {/if}
    <Button variant="ghost" type="button" aria-label="Previous join step" onclick={() => void changeStep((step - 1) as Step)} disabled={step === 0}>
      <span class="back-icon" aria-hidden="true"><Icon name="arrow-right" size={13} /></span> Back
    </Button>
    {#if step < 2}
      <Button variant="primary" type="button" onclick={advance} disabled={preparing || (step === 0 ? !(joinLeftViewId || joinLeftSourceId) || !(joinRightViewId || joinRightSourceId) : !keysValid)}>
        {preparing && step === 0 ? 'Loading…' : 'Next'} {#if !preparing}<Icon name="arrow-right" size={13} />{/if}
      </Button>
    {:else}
      {#if previewError}<Button type="button" onclick={onCheck} disabled={previewLoading || running}>Retry</Button>{/if}
      <Button variant="primary" type="button" onclick={onRun} disabled={running || previewLoading || !canRun}>{running ? 'Running…' : 'Run join'}</Button>
    {/if}
  {/snippet}
</MenuPopover>

<style>
  .stage-frame { transition: height 220ms cubic-bezier(0.16, 1, 0.3, 1); }
  .stage { padding: 16px; animation: stage-forward 190ms cubic-bezier(0.32, 0.72, 0, 1); }
  .stage.backward { animation-name: stage-back; }
  .stage-heading { margin-bottom: 18px; }
  .stage-heading h3 { margin: 0 0 4px; font-size: 14px; line-height: 1.25; color: var(--ink); }
  .stage-heading p { margin: 0; font-size: 11.5px; color: var(--muted); }

  .source-pair { display: grid; grid-template-columns: minmax(0, 1fr) 24px minmax(0, 1fr); align-items: center; }
  .source-slot { min-width: 0; min-height: 92px; padding: 13px; display: flex; flex-direction: column; justify-content: center; gap: 5px; border: 1px solid var(--control-border); border-radius: var(--radius-xl); background: var(--surface); text-align: left; transition: border-color 140ms ease, background 140ms ease, box-shadow 180ms ease; }
  .source-slot:hover { border-color: var(--faint); }
  .source-slot.active { border-color: var(--action); background: var(--action-tint); box-shadow: 0 10px 22px -18px rgba(17, 85, 245, 0.78); }
  .source-slot span { font-size: 10.5px; color: var(--muted); }
  .source-slot strong { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font: 500 12px var(--font-mono); color: var(--ink); }
  .source-slot small { font: 9.5px var(--font-mono); color: var(--faint); }
  .join-mark { display: flex; align-items: center; justify-content: center; color: var(--glyph); }
  .source-note { display: flex; align-items: center; gap: 7px; margin: 14px 0 0; font-size: 11px; color: var(--muted); }
  .source-note span { width: 6px; height: 6px; border-radius: 50%; background: var(--action); box-shadow: 0 4px 10px -5px rgba(17, 85, 245, 0.9); }

  .key-pair { display: grid; grid-template-columns: minmax(0, 1fr) 20px minmax(0, 1fr); align-items: end; }
  .equals { display: flex; align-items: center; justify-content: center; height: 32px; font: 500 12px var(--font-mono); color: var(--faint); }
  .key-note { margin: 12px 0 0; font-size: 11px; color: var(--muted); }
  .key-note.invalid, .error { color: var(--error); }

  .carry-heading { display: flex; align-items: flex-start; justify-content: space-between; gap: 12px; margin-bottom: 10px; }
  .carry-heading > span { flex: none; font: 10px var(--font-mono); color: var(--faint); }
  .carry-search { display: block; margin-bottom: 8px; }
  .carry-search :global(.field) { width: 100%; }
  .carry-list { height: 190px; display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); overflow: hidden; border-top: 1px solid var(--line); border-bottom: 1px solid var(--line); }
  .column-group { min-width: 0; overflow-y: auto; }
  .column-group + .column-group { border-left: 1px solid var(--line); }
  .group-head { position: sticky; top: 0; z-index: 1; display: flex; align-items: center; gap: 7px; min-height: 30px; padding: 0 7px; background: var(--surface-2); }
  .group-head strong { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font: 500 10.5px var(--font-mono); }
  .group-head span { font: 9.5px var(--font-mono); color: var(--faint); }
  .group-head button { padding: 2px; border: 0; background: transparent; font-size: 10.5px; color: var(--action-dark); }
  .column-row { min-height: 28px; padding: 5px 8px; }
  .column-row:hover { background: var(--surface-hover); }
  .column-row :global(.checkbox) { width: 100%; }
  .column-row :global(.label) { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-family: var(--font-mono); font-size: 10.5px; }
  .empty { grid-column: 1 / -1; margin: 0; padding: 12px 8px; font-size: 11px; color: var(--faint); }
  .preview { display: flex; align-items: center; gap: 14px; min-height: 30px; font-size: 10.5px; color: var(--muted); }
  .preview span { display: inline-flex; align-items: center; gap: 5px; }
  .preview b { font: 500 10.5px var(--font-mono); color: var(--ink); }

  .footer-status { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: 10.5px; color: var(--faint); }
  .back-icon { display: inline-flex; transform: rotate(180deg); }

  @keyframes stage-forward { from { opacity: 0; transform: translateX(16px); } to { opacity: 1; transform: none; } }
  @keyframes stage-back { from { opacity: 0; transform: translateX(-16px); } to { opacity: 1; transform: none; } }
  @media (prefers-reduced-motion: reduce) {
    .stage-frame { transition: none; }
    .stage { animation: none; }
  }
</style>
