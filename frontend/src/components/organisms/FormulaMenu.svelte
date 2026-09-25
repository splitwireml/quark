<script lang="ts">
  import { tick, untrack } from 'svelte';
  import Button from '../atoms/Button.svelte';
  import FormulaButtonGroup from '../molecules/FormulaButtonGroup.svelte';
  import FormulaHelpers from '../molecules/FormulaHelpers.svelte';
  import { buildIfExpression, buildSwitchExpression, inferActiveFormulaKind, inferFormulaKind, quoteIdentifier, quoteLiteral, sqlValueFromInput } from '../../lib/mutation-sql';
  import type { ColumnInfo } from '../../lib/types';

  type Mode = 'blank' | 'text' | 'formula';
  type IfBuilder = { kind: 'if'; active: 'condition' | 'thenValue' | 'elseValue'; condition: string; thenValue: string; elseValue: string };
  type SwitchCase = { id: number; condition: string; thenValue: string };
  type SwitchActive = 'elseValue' | `condition-${number}` | `then-${number}`;
  type SwitchBuilder = { kind: 'switch'; active: SwitchActive; cases: SwitchCase[]; elseValue: string };
  type ConditionalBuilderState = IfBuilder | SwitchBuilder;
  type Props = {
    columns: ColumnInfo[];
    boundaryLabel: string;
    targetColumn?: ColumnInfo | null;
    initialExpression?: string;
    applying: boolean;
    error: string;
    setDialog: (element: HTMLDialogElement | null) => void;
    onClose: () => void;
    onCancelAttempt: (event: Event) => void;
    onApply: (name: string, expression: string) => void;
  };

  let { columns, boundaryLabel, targetColumn = null, initialExpression, applying, error, setDialog, onClose, onCancelAttempt, onApply }: Props = $props();
  // ponytail: this dialog is remounted per operation, so its editable draft only needs the opening target.
  const initialTarget = untrack(() => targetColumn);
  let mode = $state<Mode>(initialTarget ? 'formula' : 'blank');
  let name = $state(initialTarget?.name ?? '');
  let repeatingText = $state('');
  let formula = $state(untrack(() => initialExpression) ?? (initialTarget ? quoteIdentifier(initialTarget.name) : ''));
  let kind = $derived(inferFormulaKind(formula, columns));
  let activeFunction = $derived(formula.trim().match(/^([a-z_][a-z_0-9]*)\s*\(/i)?.[1]?.toLowerCase() ?? '');
  let columnSearch = $state('');
  let selectedSource = $state(initialTarget?.name ?? '');
  let matches = $derived.by(() => {
    const query = columnSearch.trim().toLocaleLowerCase();
    return query ? columns.filter((column) => column.name.toLocaleLowerCase().includes(query)) : columns;
  });
  let conditionalBuilder = $state<ConditionalBuilderState | null>(null);
  let canUseOperations = $derived(Boolean(formula.trim()) && !conditionalBuilder);
  let switchCase = $derived.by(() => {
    const builder = conditionalBuilder;
    if (builder?.kind !== 'switch') return null;
    return builder.cases.find((item) => item.id === Number(builder.active.split('-')[1])) ?? builder.cases[builder.cases.length - 1] ?? null;
  });
  let switchCondition = $derived(switchCase?.condition ?? '');
  let switchKind = $derived(inferActiveFormulaKind(switchCondition, columns));
  let switchReady = $derived(conditionalBuilder?.kind === 'switch' && conditionalBuilder.active.startsWith('condition-') && Boolean(switchCondition.trim()));
  let switchFunction = $derived(switchCondition.trim().match(/^([a-z_][a-z_0-9]*)\s*\(/i)?.[1]?.toLowerCase() ?? '');
  let localError = $state('');
  let textarea = $state<HTMLTextAreaElement | null>(null);
  let dialog = $state<HTMLDialogElement | null>(null);
  let structuredSql = $derived.by(() => {
    if (!conditionalBuilder) return '';
    return conditionalBuilder.kind === 'if'
      ? buildIfExpression(conditionalBuilder.condition, sqlValueFromInput(conditionalBuilder.thenValue), sqlValueFromInput(conditionalBuilder.elseValue))
      : buildSwitchExpression(conditionalBuilder.cases.map((item) => ({ condition: item.condition, thenValue: sqlValueFromInput(item.thenValue) })), sqlValueFromInput(conditionalBuilder.elseValue));
  });

  function dialogRef(node: HTMLDialogElement) {
    dialog = node;
    setDialog(node);
    return { destroy: () => { dialog = null; setDialog(null); } };
  }

  function setIfOperand(key: IfBuilder['active'], value: string) { if (conditionalBuilder?.kind === 'if') { conditionalBuilder.active = key; conditionalBuilder[key] = value; } }
  function setSwitchOperand(key: SwitchActive, value: string) {
    if (conditionalBuilder?.kind !== 'switch') return;
    conditionalBuilder.active = key;
    if (key === 'elseValue') conditionalBuilder.elseValue = value;
    else {
      const [part, id] = key.split('-');
      const branch = conditionalBuilder.cases.find((item) => item.id === Number(id));
      if (branch) { if (part === 'condition') branch.condition = value; else branch.thenValue = value; }
    }
  }
  function activateIfOperand(key: IfBuilder['active']) { if (conditionalBuilder?.kind === 'if') conditionalBuilder.active = key; }
  function activateSwitchOperand(key: SwitchActive) { if (conditionalBuilder?.kind === 'switch') conditionalBuilder.active = key; }
  function advanceSwitch(event: KeyboardEvent, key: SwitchActive) {
    if (event.key !== 'Enter' || event.isComposing || conditionalBuilder?.kind !== 'switch') return;
    event.preventDefault();
    conditionalBuilder.active = key;
    void focusActiveOperand();
  }

  async function focusActiveOperand() {
    await tick();
    const editor = dialog?.querySelector<HTMLInputElement | HTMLTextAreaElement>(`[data-operand="${conditionalBuilder?.active}"]`);
    editor?.focus({ preventScroll: true });
    editor?.scrollIntoView({ block: 'nearest' });
    if (editor instanceof HTMLTextAreaElement) editor.setSelectionRange(editor.value.length, editor.value.length);
  }

  function switchSelection(whole = false) {
    if (!switchCase) return null;
    const editor = dialog?.querySelector<HTMLTextAreaElement>(`[data-operand="condition-${switchCase.id}"]`);
    let start = editor?.selectionStart ?? switchCase.condition.length;
    let end = editor?.selectionEnd ?? start;
    if (whole && start === end && switchCase.condition.trim()) { start = 0; end = switchCase.condition.length; }
    return { branch: switchCase, editor, start, end };
  }

  function updateSwitchCondition(value: string, cursor: number, id: number) {
    if (conditionalBuilder?.kind !== 'switch') return;
    const branch = conditionalBuilder.cases.find((item) => item.id === id);
    if (!branch) return;
    branch.condition = value;
    conditionalBuilder.active = `condition-${id}`;
    void tick().then(() => {
      const editor = dialog?.querySelector<HTMLTextAreaElement>(`[data-operand="condition-${id}"]`);
      editor?.focus({ preventScroll: true });
      editor?.setSelectionRange(cursor, cursor);
    });
  }

  function insertSwitchText(value: string, replaceSingleColumn = false) {
    const selection = switchSelection();
    if (!selection) return;
    let { start, end } = selection;
    if (replaceSingleColumn && start === end && start === selection.branch.condition.length && /^"(?:[^"]|"")*"$/.test(selection.branch.condition.trim())) { start = 0; end = selection.branch.condition.length; }
    updateSwitchCondition(`${selection.branch.condition.slice(0, start)}${value}${selection.branch.condition.slice(end)}`, start + value.length, selection.branch.id);
  }

  function wrapSwitchText(prefix: string, suffix: string) {
    const selection = switchSelection(true);
    if (!selection) return;
    const value = `${prefix}${selection.branch.condition.slice(selection.start, selection.end)}${suffix}`;
    updateSwitchCondition(`${selection.branch.condition.slice(0, selection.start)}${value}${selection.branch.condition.slice(selection.end)}`, selection.start + value.length, selection.branch.id);
  }

  function templateSwitchText(prefix: string, middle: string, suffix: string) {
    const selection = switchSelection(true);
    if (!selection) return;
    const selected = selection.branch.condition.slice(selection.start, selection.end);
    updateSwitchCondition(`${selection.branch.condition.slice(0, selection.start)}${prefix}${selected}${middle}${suffix}${selection.branch.condition.slice(selection.end)}`, selection.start + prefix.length + selected.length + middle.length, selection.branch.id);
  }

  function replaceSwitchText(value: string) {
    if (switchCase) updateSwitchCondition(value, value.length, switchCase.id);
  }

  function insertColumn(column: ColumnInfo) {
    const value = quoteIdentifier(column.name);
    if (conditionalBuilder?.kind === 'switch') {
      insertSwitchText(value, true);
      selectedSource = column.name;
      return;
    }
    selectedSource = column.name;
    const start = textarea?.selectionStart ?? formula.length;
    const end = textarea?.selectionEnd ?? start;
    if (!conditionalBuilder && (awaitsOperand() || (start > 0 && start < formula.length) || end > start)) {
      insertText(value);
      return;
    }
    formula = value;
    conditionalBuilder = null;
    focusCaret(formula.length);
  }

  function awaitsOperand() { return /(?:\+|-|\*|\/|=|<>|>=|<=|>|<|\|\||\bAND|\bOR)\s*$/i.test(formula); }

  function focusCaret(cursor: number) {
    void tick().then(() => { textarea?.focus(); textarea?.setSelectionRange(cursor, cursor); });
  }

  function bounds(): [number, number] {
    let start = textarea?.selectionStart ?? formula.length;
    let end = textarea?.selectionEnd ?? start;
    if (start === end && formula.trim()) { start = 0; end = formula.length; }
    return [start, end];
  }

  function insertText(value: string) {
    const start = textarea?.selectionStart ?? formula.length;
    const end = textarea?.selectionEnd ?? start;
    formula = `${formula.slice(0, start)}${value}${formula.slice(end)}`;
    focusCaret(start + value.length);
  }

  function wrapTyped(prefix: string, suffix: string) {
    if (!formula.trim()) return;
    const [start, end] = bounds();
    const wrapped = `${prefix}${formula.slice(start, end)}${suffix}`;
    formula = `${formula.slice(0, start)}${wrapped}${formula.slice(end)}`;
    focusCaret(start + wrapped.length);
  }

  function templateTyped(prefix: string, middle: string, suffix: string) {
    if (!formula.trim()) return;
    const [start, end] = bounds();
    const selected = formula.slice(start, end);
    formula = `${formula.slice(0, start)}${prefix}${selected}${middle}${suffix}${formula.slice(end)}`;
    focusCaret(start + prefix.length + selected.length + middle.length);
  }

  function replaceExpression(value: string) {
    if (awaitsOperand()) { insertText(value); return; }
    formula = value;
    focusCaret(value.length);
  }

  function editFormula(value: string) {
    formula = value;
  }

  let nextSwitchCaseId = 2;
  function startConditional(builderKind: 'if' | 'switch') {
    if (conditionalBuilder) return;
    const seed = formula.trim();
    conditionalBuilder = builderKind === 'if'
      ? { kind: 'if', active: 'condition', condition: kind === 'boolean' ? seed : `${seed} = `, thenValue: '', elseValue: '' }
      : { kind: 'switch', active: 'condition-1', cases: [{ id: 1, condition: seed, thenValue: '' }], elseValue: 'NULL' };
    void tick().then(() => {
      dialog?.querySelector('.editor-pane')?.scrollTo(0, 0);
      void focusActiveOperand();
    });
  }

  function addSwitchCase() {
    if (conditionalBuilder?.kind !== 'switch') return;
    const id = nextSwitchCaseId++;
    conditionalBuilder.cases = [...conditionalBuilder.cases, { id, condition: selectedSource ? quoteIdentifier(selectedSource) : formula.trim(), thenValue: '' }];
    conditionalBuilder.active = `condition-${id}`;
    void focusActiveOperand();
  }

  function removeSwitchCase(id: number) {
    if (conditionalBuilder?.kind !== 'switch' || conditionalBuilder.cases.length === 1) return;
    conditionalBuilder.cases = conditionalBuilder.cases.filter((item) => item.id !== id);
    conditionalBuilder.active = `condition-${conditionalBuilder.cases[0].id}`;
    void focusActiveOperand();
  }

  function useFreeform() {
    if (structuredSql) formula = structuredSql;
    conditionalBuilder = null;
    void tick().then(() => textarea?.focus());
  }

  function submit() {
    const trimmedName = name.trim();
    if (!trimmedName) { localError = 'Enter a column name.'; return; }
    if (columns.some((column) => column.name !== targetColumn?.name && column.name.toLocaleLowerCase() === trimmedName.toLocaleLowerCase())) { localError = 'Column names must be unique.'; return; }
    if (conditionalBuilder?.kind === 'switch' && conditionalBuilder.cases.some((item) => ['text', 'number', 'date'].includes(inferFormulaKind(item.condition, columns)))) { localError = 'Each When condition needs a comparison that returns true or false.'; return; }
    const expression = mode === 'blank' ? 'NULL' : mode === 'text' ? quoteLiteral(repeatingText) : conditionalBuilder ? structuredSql : formula.trim();
    if (!expression) { localError = conditionalBuilder?.kind === 'switch' ? 'Enter a condition and a result for each case, plus an otherwise result.' : conditionalBuilder ? 'Fill every IF operand.' : 'Enter a formula.'; return; }
    localError = '';
    onApply(trimmedName, expression);
  }
</script>

<dialog use:dialogRef aria-labelledby="formula-title" onclose={onClose} oncancel={onCancelAttempt}>
  <form class="formula-menu" class:switching={conditionalBuilder?.kind === 'switch'} inert={applying} aria-busy={applying} onsubmit={(event) => { event.preventDefault(); submit(); }}>
    <header>
      <div><h2 id="formula-title">{targetColumn ? 'Modify column' : 'Insert column'}</h2><p>{boundaryLabel}</p></div>
      <button type="button" class="close" onclick={() => dialog?.close()} disabled={applying} aria-label="Close">×</button>
    </header>

    <label class="field">{targetColumn ? 'Column name' : 'New column name'}
      <input value={name} oninput={(event) => name = event.currentTarget.value} maxlength="128" autocomplete="off" />
    </label>

    {#if !targetColumn}
      <div class="modes" role="group" aria-label="Column value type">
        <Button type="button" active={mode === 'blank'} aria-pressed={mode === 'blank'} onclick={() => mode = 'blank'}>Blank</Button>
        <Button type="button" active={mode === 'text'} aria-pressed={mode === 'text'} onclick={() => mode = 'text'}>Repeating text</Button>
        <Button type="button" active={mode === 'formula'} aria-pressed={mode === 'formula'} onclick={() => mode = 'formula'}>Formula</Button>
      </div>
    {/if}

    {#if mode === 'blank'}
      <p class="note">Every row will contain <code>NULL</code>.</p>
    {:else if mode === 'text'}
      <label class="field">Text repeated for every row
        <input value={repeatingText} oninput={(event) => repeatingText = event.currentTarget.value} />
      </label>
    {:else}
      <div class="formula-workspace" class:switching={conditionalBuilder?.kind === 'switch'}>
        <aside class="source-pane" aria-label="Source columns">
          <div class="source-heading"><strong>Columns</strong><small>{matches.length} of {columns.length}</small></div>
          <label class="field">Find a column
            <input type="search" value={columnSearch} oninput={(event) => columnSearch = event.currentTarget.value} placeholder="Search columns" autocomplete="off" />
          </label>
          <div class="column-results" role="group" aria-label="Source columns">
            {#each matches as column (column.name)}
              <button type="button" class:selected={selectedSource === column.name} aria-pressed={selectedSource === column.name} onclick={() => insertColumn(column)} title={column.type}><span>{column.name}</span><small>{column.type}</small></button>
            {:else}
              <p>No matching columns</p>
            {/each}
          </div>
          <p class="source-hint">{conditionalBuilder?.kind === 'switch' ? 'Click a column to insert it into the selected When condition.' : 'Choose a column to start or fill the next operand.'}</p>
        </aside>
        <div class="editor-pane">

      {#if conditionalBuilder?.kind === 'if'}
        <fieldset class="conditional-builder">
          <legend>If · write a condition, then set the results; text is quoted for you</legend>
          <label class:active={conditionalBuilder.active === 'condition'}>IF
            <input data-operand="condition" value={conditionalBuilder.condition} placeholder="Write a condition" onfocus={() => activateIfOperand('condition')} oninput={(event) => setIfOperand('condition', event.currentTarget.value)} />
          </label>
          <label class:active={conditionalBuilder.active === 'thenValue'}>THEN
            <input data-operand="thenValue" value={conditionalBuilder.thenValue} onfocus={() => activateIfOperand('thenValue')} oninput={(event) => setIfOperand('thenValue', event.currentTarget.value)} />
          </label>
          <label class:active={conditionalBuilder.active === 'elseValue'}>ELSE
            <input data-operand="elseValue" value={conditionalBuilder.elseValue} onfocus={() => activateIfOperand('elseValue')} oninput={(event) => setIfOperand('elseValue', event.currentTarget.value)} />
          </label>
          <code>{structuredSql || 'Complete all three operands'}</code>
        </fieldset>
      {:else if conditionalBuilder?.kind === 'switch'}
        <section class="switch-builder" aria-labelledby="switch-title">
          <div class="switch-heading">
            <div><h3 id="switch-title">Switch by condition</h3><p>Each case runs when its condition is true. The first true case sets the result.</p></div>
            <Button type="button" variant="ghost" onclick={useFreeform}>{structuredSql ? 'Edit as SQL' : 'Cancel switch'}</Button>
          </div>
          <div class="switch-cases">
            {#each conditionalBuilder.cases as branch, index (branch.id)}
              {@const nextCase = conditionalBuilder.cases[index + 1]}
              <div class="switch-case">
                <div class="switch-case-heading"><strong>Case {index + 1}</strong><Button type="button" variant="ghost" disabled={conditionalBuilder.cases.length === 1} onclick={() => removeSwitchCase(branch.id)} aria-label={`Remove case ${index + 1}`}>Remove</Button></div>
                <div class="switch-case-fields">
                  <label class:active={conditionalBuilder.active === `condition-${branch.id}`}>When this is true
                    <textarea data-operand={`condition-${branch.id}`} value={branch.condition} rows="2" placeholder="e.g. &quot;amount&quot; &lt; 10 AND &quot;region&quot; = 'east'" onfocus={() => activateSwitchOperand(`condition-${branch.id}`)} oninput={(event) => setSwitchOperand(`condition-${branch.id}`, event.currentTarget.value)} spellcheck="false"></textarea>
                  </label>
                  {#if ['text', 'number', 'date'].includes(inferFormulaKind(branch.condition, columns))}<small class="condition-hint">Add a comparison, such as &lt; or =, to make this true or false.</small>{/if}
                  <label class:active={conditionalBuilder.active === `then-${branch.id}`}>Then return
                    <input data-operand={`then-${branch.id}`} value={branch.thenValue} placeholder="Result for matching rows" onfocus={() => activateSwitchOperand(`then-${branch.id}`)} oninput={(event) => setSwitchOperand(`then-${branch.id}`, event.currentTarget.value)} onkeydown={(event) => advanceSwitch(event, nextCase ? `condition-${nextCase.id}` : 'elseValue')} />
                  </label>
                </div>
              </div>
            {/each}
          </div>
          <Button type="button" onclick={addSwitchCase}>+ Add another case</Button>
          <label class="switch-default" class:active={conditionalBuilder.active === 'elseValue'}>Otherwise return
            <input data-operand="elseValue" value={conditionalBuilder.elseValue} placeholder="Result when nothing matches" onfocus={() => activateSwitchOperand('elseValue')} oninput={(event) => setSwitchOperand('elseValue', event.currentTarget.value)} />
          </label>
          <p class="switch-hint">Use columns and functions to build each condition. Put text in single quotes inside conditions; return text is quoted for you. <code>NULL</code> leaves unmatched rows empty.</p>
          {#if structuredSql}<details><summary>View SQL</summary><code>{structuredSql}</code></details>{/if}
        </section>
      {:else}
        <label class="field">Formula
          <textarea bind:this={textarea} value={formula} oninput={(event) => editFormula(event.currentTarget.value)} rows="3" spellcheck="false" placeholder='Select a column or write SQL'></textarea>
        </label>
      {/if}

      {#if conditionalBuilder?.kind === 'if'}<Button type="button" onclick={useFreeform}>Edit as SQL</Button>{/if}
      {#if !conditionalBuilder}<div class="operations">
        <p class="type-status" aria-live="polite">Result type: <strong>{conditionalBuilder ? 'building conditional' : kind === 'unknown' ? 'unknown' : kind}</strong></p>
        <FormulaHelpers onInsert={insertText} onWrap={wrapTyped} onTemplate={templateTyped} onReplace={replaceExpression} keypad {kind} ready={canUseOperations} editable={!conditionalBuilder} {activeFunction}>
          {#snippet conditional()}
            <FormulaButtonGroup label="Conditional" keypad available={canUseOperations}>
              <Button type="button" active={conditionalBuilder?.kind === 'if'} disabled={!canUseOperations} onclick={() => startConditional('if')}>IF</Button>
              <Button type="button" active={conditionalBuilder?.kind === 'switch'} disabled={!canUseOperations} onclick={() => startConditional('switch')}>Switch</Button>
            </FormulaButtonGroup>
          {/snippet}
        </FormulaHelpers>
      </div>
      {/if}
        </div>
        {#if conditionalBuilder?.kind === 'switch'}
          <aside class="switch-tools-pane" aria-label="Condition functions">
            <p class="type-status" aria-live="polite">Condition tools · <strong>{switchReady ? switchKind : 'select a When condition'}</strong></p>
            <FormulaHelpers onInsert={insertSwitchText} onWrap={wrapSwitchText} onTemplate={templateSwitchText} onReplace={replaceSwitchText} keypad kind={switchKind} ready={switchReady} editable={switchReady} activeFunction={switchFunction} />
          </aside>
        {/if}
      </div>
    {/if}

    {#if localError || error}<p class="error" role="alert">{localError || error}</p>{/if}
    <footer>
      <Button type="button" onclick={() => dialog?.close()} disabled={applying}>Cancel</Button>
      <Button type="submit" variant="primary" disabled={applying}>{applying ? 'Checking…' : targetColumn ? 'Apply change' : 'Insert column'}</Button>
    </footer>
  </form>
</dialog>

<style>
  dialog { padding: 0; border: 1px solid var(--line); border-radius: 8px; background: var(--surface); box-shadow: 0 14px 40px rgba(0, 0, 0, 0.04); }
  dialog::backdrop { background: rgba(17, 17, 17, 0.32); }
  .formula-menu { width: min(920px, 96vw); max-height: 92vh; overflow-y: auto; padding: 20px; display: flex; flex-direction: column; gap: 14px; }
  .formula-menu.switching { width: min(1220px, 96vw); }
  header { display: flex; align-items: flex-start; justify-content: space-between; gap: 12px; }
  h2 { margin: 0; font-size: 16px; font-weight: 600; letter-spacing: -0.02em; }
  header p, .note { margin: 3px 0 0; font-size: 11.5px; color: var(--muted); }
  .close { border: 0; background: transparent; color: var(--muted); font-size: 18px; }
  .field { display: flex; flex-direction: column; gap: 5px; font-size: 11px; color: var(--muted); }
  .formula-menu > .field { max-width: 440px; }
  input, textarea { width: 100%; border: 1px solid var(--line); border-radius: 4px; background: var(--surface); color: var(--ink); }
  input { height: 34px; padding: 0 9px; }
  textarea { padding: 8px 9px; resize: vertical; font: 12px/1.5 var(--font-mono); }
  .modes { display: flex; flex-wrap: wrap; gap: 6px; }
  fieldset { min-width: 0; margin: 0; padding: 0; border: 0; }
  legend { margin-bottom: 6px; font-size: 11px; color: var(--muted); }
  .formula-workspace { display: grid; grid-template-columns: minmax(220px, 270px) minmax(0, 1fr); height: min(540px, 60vh); min-height: 340px; border: 1px solid var(--line); border-radius: 6px; overflow: hidden; }
  .formula-workspace.switching { grid-template-columns: minmax(190px, 230px) minmax(330px, 1fr) minmax(280px, 370px); height: min(650px, 70vh); }
  .source-pane { display: flex; flex-direction: column; min-height: 0; padding: 16px; border-right: 1px solid var(--line); background: var(--surface-inset); }
  .source-heading { display: flex; align-items: baseline; justify-content: space-between; gap: 8px; margin-bottom: 12px; }
  .source-heading strong { font-size: 12px; font-weight: 600; color: var(--ink); }
  .source-heading small { font: 10px var(--font-mono); color: var(--muted); }
  .column-results { flex: 1; min-height: 0; overflow-y: auto; margin-top: 8px; }
  .column-results button { width: 100%; display: flex; align-items: center; gap: 8px; padding: 8px 10px; border: 0; background: transparent; color: var(--ink); text-align: left; }
  .column-results button:hover, .column-results button:focus-visible { background: var(--surface-hover); }
  .column-results button.selected { background: var(--surface); box-shadow: inset 2px 0 var(--action); }
  .column-results span { flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font: 11px var(--font-mono); }
  .column-results small { color: var(--muted); font: 10px var(--font-mono); }
  .column-results p { margin: 8px 10px; color: var(--muted); font-size: 11px; }
  .source-hint { margin: 10px 0 0; font-size: 10.5px; line-height: 1.4; color: var(--muted); }
  .editor-pane { min-width: 0; min-height: 0; overflow-y: auto; display: flex; flex-direction: column; gap: 8px; padding: 14px 18px; }
  .switch-tools-pane { min-width: 0; min-height: 0; overflow-y: auto; display: grid; align-content: start; gap: 10px; padding: 16px; border-left: 1px solid var(--line); background: var(--surface-inset); }
  .switch-tools-pane :global(.helper-groups.keypad) { grid-template-columns: repeat(2, minmax(0, 1fr)); }
  .editor-pane textarea { min-height: 88px; }
  .operations { display: grid; gap: 6px; }
  .type-status { margin: 0; font-size: 11px; color: var(--muted); }
  .type-status strong { color: var(--ink); font-weight: 600; }
  .conditional-builder { display: grid; grid-template-columns: repeat(3, minmax(0, 1fr)); gap: 8px; padding: 10px; border: 1px solid var(--line); border-radius: 4px; background: var(--surface-inset); }
  .conditional-builder legend { grid-column: 1 / -1; }
  .conditional-builder label { display: grid; gap: 4px; padding: 6px; border: 1px solid transparent; border-radius: var(--radius-md); font-size: 10px; color: var(--muted); }
  .conditional-builder label.active { border-color: var(--line-strong); background: var(--surface); color: var(--ink); }
  .conditional-builder code { grid-column: 1 / -1; overflow-wrap: anywhere; color: var(--ink); }
  .switch-builder { display: flex; flex-direction: column; align-items: flex-start; gap: 14px; }
  .switch-heading { width: 100%; display: flex; justify-content: space-between; align-items: flex-start; gap: 12px; }
  .switch-heading h3 { margin: 0; font-size: 15px; font-weight: 600; letter-spacing: -0.02em; }
  .switch-heading p, .switch-hint { margin: 3px 0 0; color: var(--muted); font-size: 11px; line-height: 1.45; }
  .switch-default { width: 100%; display: grid; gap: 6px; color: var(--ink); font-size: 11px; font-weight: 600; }
  .switch-default input, .switch-case input, .switch-case textarea { font: 12px/1.5 var(--font-mono); }
  .switch-cases { width: 100%; display: grid; gap: 8px; }
  .switch-case { padding: 8px 10px 12px; border: 1px solid var(--line); border-radius: var(--radius-md); animation: case-enter 150ms ease-out; }
  .switch-case-heading { display: flex; justify-content: space-between; align-items: center; gap: 8px; margin-bottom: 5px; }
  .switch-case-heading strong { font-size: 10px; color: var(--muted); font-weight: 600; text-transform: uppercase; letter-spacing: 0.06em; }
  .switch-case-fields { display: grid; gap: 10px; }
  .switch-case-fields label { min-width: 0; display: grid; gap: 5px; color: var(--ink-2); font-size: 11px; }
  .switch-case-fields textarea { min-height: 52px; resize: vertical; }
  .condition-hint { margin-top: -6px; color: var(--muted); font-size: 10px; }
  .switch-hint { margin-top: -7px; }
  .switch-builder details { max-width: 100%; color: var(--muted); font-size: 10px; }
  .switch-builder summary { cursor: pointer; }
  .switch-builder details code { display: block; margin-top: 6px; overflow-wrap: anywhere; color: var(--ink); }
  .switch-builder label.active input, .switch-builder label.active textarea { border-color: var(--line-strong); }
  @keyframes case-enter { from { opacity: 0; transform: translateY(4px); } }
  @media (prefers-reduced-motion: reduce) { .switch-case { animation: none; } }
  code { font-family: var(--font-mono); }
  .error { margin: 0; padding: 8px 10px; border: 1px solid color-mix(in srgb, var(--error) 45%, var(--line)); border-radius: var(--radius-md); color: var(--error); font-size: 12px; }
  footer { display: flex; justify-content: flex-end; gap: 8px; padding-top: 2px; }
  @media (max-width: 720px) {
    .formula-workspace { display: block; flex: none; height: auto; min-height: 0; overflow: visible; }
    .source-pane { height: 250px; border-right: 0; border-bottom: 1px solid var(--line); }
    .editor-pane { overflow: visible; }
    .conditional-builder { grid-template-columns: 1fr; }
  }
  @media (max-width: 1050px) {
    .formula-workspace.switching { display: block; height: auto; max-height: 70vh; min-height: 0; overflow-y: auto; }
    .formula-workspace.switching .source-pane { height: 200px; border-right: 0; border-bottom: 1px solid var(--line); }
    .switch-tools-pane { border-left: 0; border-top: 1px solid var(--line); }
  }
</style>
