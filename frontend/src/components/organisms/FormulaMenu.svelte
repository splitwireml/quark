<script lang="ts">
  import { tick, untrack } from 'svelte';
  import Button from '../atoms/Button.svelte';
  import Chip from '../atoms/Chip.svelte';
  import FormulaButtonGroup from '../molecules/FormulaButtonGroup.svelte';
  import { buildIfExpression, buildSwitchExpression, quoteIdentifier, quoteLiteral } from '../../lib/mutation-sql';
  import type { ColumnInfo } from '../../lib/types';

  type Mode = 'blank' | 'text' | 'formula';
  type IfBuilder = { kind: 'if'; active: 'condition' | 'thenValue' | 'elseValue'; condition: string; thenValue: string; elseValue: string };
  type SwitchCase = { id: number; match: string; thenValue: string };
  type SwitchActive = 'value' | 'elseValue' | `match-${number}` | `then-${number}`;
  type SwitchBuilder = { kind: 'switch'; active: SwitchActive; value: string; cases: SwitchCase[]; elseValue: string };
  type ConditionalBuilder = IfBuilder | SwitchBuilder;
  type Props = {
    columns: ColumnInfo[];
    boundaryLabel: string;
    targetColumn?: ColumnInfo | null;
    applying: boolean;
    error: string;
    setDialog: (element: HTMLDialogElement | null) => void;
    onClose: () => void;
    onCancelAttempt: (event: Event) => void;
    onApply: (name: string, expression: string) => void;
  };

  let { columns, boundaryLabel, targetColumn = null, applying, error, setDialog, onClose, onCancelAttempt, onApply }: Props = $props();
  // ponytail: this dialog is remounted per operation, so its editable draft only needs the opening target.
  const initialTarget = untrack(() => targetColumn);
  let mode = $state<Mode>(initialTarget ? 'formula' : 'blank');
  let name = $state(initialTarget?.name ?? '');
  let repeatingText = $state('');
  let formula = $state(initialTarget ? quoteIdentifier(initialTarget.name) : '');
  let conditional = $state<ConditionalBuilder | null>(null);
  let localError = $state('');
  let textarea = $state<HTMLTextAreaElement | null>(null);
  let dialog = $state<HTMLDialogElement | null>(null);
  let firstColumnName = $state<string | null>(initialTarget?.name ?? null);
  let firstColumn = $derived(columns.find((column) => column.name === firstColumnName));
  let structuredSql = $derived.by(() => {
    if (!conditional) return '';
    return conditional.kind === 'if'
      ? buildIfExpression(conditional.condition, conditional.thenValue, conditional.elseValue)
      : buildSwitchExpression(conditional.value, conditional.cases, conditional.elseValue);
  });

  function dialogRef(node: HTMLDialogElement) {
    dialog = node;
    setDialog(node);
    return { destroy: () => { dialog = null; setDialog(null); } };
  }

  function activeOperand(): string {
    if (!conditional) return '';
    if (conditional.kind === 'if') {
      if (conditional.active === 'condition') return conditional.condition;
      if (conditional.active === 'thenValue') return conditional.thenValue;
      return conditional.elseValue;
    }
    if (conditional.active === 'value') return conditional.value;
    if (conditional.active === 'elseValue') return conditional.elseValue;
    const [part, id] = conditional.active.split('-');
    const branch = conditional.cases.find((item) => item.id === Number(id));
    return part === 'match' ? branch?.match ?? '' : branch?.thenValue ?? '';
  }

  function setActiveOperand(value: string) {
    if (!conditional) return;
    if (conditional.kind === 'if') {
      if (conditional.active === 'condition') conditional.condition = value;
      else if (conditional.active === 'thenValue') conditional.thenValue = value;
      else conditional.elseValue = value;
    } else if (conditional.active === 'value') conditional.value = value;
    else if (conditional.active === 'elseValue') conditional.elseValue = value;
    else {
      const [part, id] = conditional.active.split('-');
      const branch = conditional.cases.find((item) => item.id === Number(id));
      if (branch) { if (part === 'match') branch.match = value; else branch.thenValue = value; }
    }
  }

  function setIfOperand(key: IfBuilder['active'], value: string) { if (conditional?.kind === 'if') { conditional.active = key; conditional[key] = value; } }
  function setSwitchOperand(key: SwitchActive, value: string) { if (conditional?.kind === 'switch') { conditional.active = key; setActiveOperand(value); } }
  function activateIfOperand(key: IfBuilder['active']) { if (conditional?.kind === 'if') conditional.active = key; }
  function activateSwitchOperand(key: SwitchActive) { if (conditional?.kind === 'switch') conditional.active = key; }

  async function focusActiveOperand() {
    await tick();
    dialog?.querySelector<HTMLInputElement>(`[data-operand="${conditional?.active}"]`)?.focus();
  }

  async function insertText(value: string) {
    if (conditional) {
      setActiveOperand(`${activeOperand()}${value}`);
      await focusActiveOperand();
      return;
    }
    const start = textarea?.selectionStart ?? formula.length;
    const end = textarea?.selectionEnd ?? start;
    formula = `${formula.slice(0, start)}${value}${formula.slice(end)}`;
    await tick();
    textarea?.focus();
    textarea?.setSelectionRange(start + value.length, start + value.length);
  }

  async function replaceExpression(value: string) {
    if (conditional) {
      setActiveOperand(value);
      await focusActiveOperand();
      return;
    }
    formula = value;
    await tick();
    textarea?.focus();
    textarea?.select();
  }

  function insertColumn(column: ColumnInfo) {
    firstColumnName ??= column.name;
    if (conditional) void replaceExpression(quoteIdentifier(column.name));
    else void insertText(quoteIdentifier(column.name));
  }

  async function wrap(prefix: string, suffix: string) {
    if (conditional) {
      setActiveOperand(`${prefix}${activeOperand()}${suffix}`);
      await focusActiveOperand();
      return;
    }
    let start = textarea?.selectionStart ?? 0;
    let end = textarea?.selectionEnd ?? formula.length;
    if (start === end && formula.trim()) { start = 0; end = formula.length; }
    const selected = formula.slice(start, end);
    const value = `${prefix}${selected}${suffix}`;
    formula = `${formula.slice(0, start)}${value}${formula.slice(end)}`;
    await tick();
    textarea?.focus();
    textarea?.setSelectionRange(start + value.length, start + value.length);
  }

  async function template(prefix: string, middle: string, suffix: string) {
    if (conditional) {
      setActiveOperand(`${prefix}${activeOperand()}${middle}${suffix}`);
      await focusActiveOperand();
      return;
    }
    let start = textarea?.selectionStart ?? 0;
    let end = textarea?.selectionEnd ?? formula.length;
    if (start === end && formula.trim()) { start = 0; end = formula.length; }
    const selected = formula.slice(start, end);
    const value = `${prefix}${selected}${middle}${suffix}`;
    formula = `${formula.slice(0, start)}${value}${formula.slice(end)}`;
    const cursor = start + prefix.length + selected.length + middle.length;
    await tick();
    textarea?.focus();
    textarea?.setSelectionRange(cursor, cursor);
  }

  let nextSwitchCaseId = 2;
  function startConditional(kind: 'if' | 'switch') {
    if (conditional) return;
    const seed = formula.trim();
    conditional = kind === 'if'
      ? { kind: 'if', active: 'condition', condition: seed, thenValue: '', elseValue: '' }
      : { kind: 'switch', active: 'value', value: seed, cases: [{ id: 1, match: '', thenValue: '' }], elseValue: '' };
    void focusActiveOperand();
  }

  function addSwitchCase() {
    if (conditional?.kind !== 'switch') return;
    const id = nextSwitchCaseId++;
    conditional.cases = [...conditional.cases, { id, match: '', thenValue: '' }];
    conditional.active = `match-${id}`;
    void focusActiveOperand();
  }

  function removeSwitchCase(id: number) {
    if (conditional?.kind !== 'switch' || conditional.cases.length === 1) return;
    conditional.cases = conditional.cases.filter((item) => item.id !== id);
    conditional.active = 'value';
  }

  function useFreeform() {
    if (structuredSql) formula = structuredSql;
    conditional = null;
    void tick().then(() => textarea?.focus());
  }

  function dragColumn(event: DragEvent, column: ColumnInfo) {
    event.dataTransfer?.setData('text/plain', quoteIdentifier(column.name));
    event.dataTransfer?.setData('application/x-quark-column', column.name);
  }

  function dropColumn(event: DragEvent) {
    event.preventDefault();
    const columnName = event.dataTransfer?.getData('application/x-quark-column');
    const column = columns.find((item) => item.name === columnName);
    if (column) insertColumn(column);
    else {
      const value = event.dataTransfer?.getData('text/plain');
      if (value) void insertText(value);
    }
  }

  function submit() {
    const trimmedName = name.trim();
    if (!trimmedName) { localError = 'Enter a column name.'; return; }
    if (columns.some((column) => column.name !== targetColumn?.name && column.name.toLocaleLowerCase() === trimmedName.toLocaleLowerCase())) { localError = 'Column names must be unique.'; return; }
    const expression = mode === 'blank' ? 'NULL' : mode === 'text' ? quoteLiteral(repeatingText) : conditional ? structuredSql : formula.trim();
    if (!expression) { localError = conditional ? `Fill every ${conditional.kind === 'if' ? 'IF' : 'Switch'} operand.` : 'Enter a formula.'; return; }
    localError = '';
    onApply(trimmedName, expression);
  }
</script>

<dialog use:dialogRef aria-labelledby="formula-title" onclose={onClose} oncancel={onCancelAttempt}>
  <form class="formula-menu" inert={applying} aria-busy={applying} onsubmit={(event) => { event.preventDefault(); submit(); }}>
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
      {#if conditional?.kind === 'if'}
        <fieldset class="conditional-builder">
          <legend>IF / THEN / ELSE — click an operand, then use columns or operators below</legend>
          <label class:active={conditional.active === 'condition'}>IF
            <input data-operand="condition" value={conditional.condition} onfocus={() => activateIfOperand('condition')} oninput={(event) => setIfOperand('condition', event.currentTarget.value)} />
          </label>
          <label class:active={conditional.active === 'thenValue'}>THEN
            <input data-operand="thenValue" value={conditional.thenValue} onfocus={() => activateIfOperand('thenValue')} oninput={(event) => setIfOperand('thenValue', event.currentTarget.value)} />
          </label>
          <label class:active={conditional.active === 'elseValue'}>ELSE
            <input data-operand="elseValue" value={conditional.elseValue} onfocus={() => activateIfOperand('elseValue')} oninput={(event) => setIfOperand('elseValue', event.currentTarget.value)} />
          </label>
          <code>{structuredSql || 'Complete all three operands'}</code>
        </fieldset>
      {:else if conditional?.kind === 'switch'}
        <fieldset class="conditional-builder switch-builder">
          <legend>Switch — add cases, click an operand, then use columns or operators below</legend>
          <label class:active={conditional.active === 'value'}>VALUE
            <input data-operand="value" value={conditional.value} onfocus={() => activateSwitchOperand('value')} oninput={(event) => setSwitchOperand('value', event.currentTarget.value)} />
          </label>
          {#each conditional.cases as branch, index (branch.id)}
            <label class:active={conditional.active === `match-${branch.id}`}>WHEN {index + 1}
              <input data-operand={`match-${branch.id}`} value={branch.match} onfocus={() => activateSwitchOperand(`match-${branch.id}`)} oninput={(event) => setSwitchOperand(`match-${branch.id}`, event.currentTarget.value)} />
            </label>
            <label class:active={conditional.active === `then-${branch.id}`}>THEN {index + 1}
              <input data-operand={`then-${branch.id}`} value={branch.thenValue} onfocus={() => activateSwitchOperand(`then-${branch.id}`)} oninput={(event) => setSwitchOperand(`then-${branch.id}`, event.currentTarget.value)} />
            </label>
            <button type="button" class="remove-case" disabled={conditional.cases.length === 1} onclick={() => removeSwitchCase(branch.id)} aria-label={`Remove switch case ${index + 1}`}>×</button>
          {/each}
          <label class:active={conditional.active === 'elseValue'}>DEFAULT
            <input data-operand="elseValue" value={conditional.elseValue} onfocus={() => activateSwitchOperand('elseValue')} oninput={(event) => setSwitchOperand('elseValue', event.currentTarget.value)} />
          </label>
          <button type="button" class="add-case" onclick={addSwitchCase}>+ Case</button>
          <code>{structuredSql || 'Complete the value, every case, and the default'}</code>
        </fieldset>
      {:else}
        <label class="field">Formula
          <textarea bind:this={textarea} bind:value={formula} rows="5" spellcheck="false" placeholder='For example: "price" * 1.05' ondragover={(event) => event.preventDefault()} ondrop={dropColumn}></textarea>
        </label>
      {/if}

      <fieldset>
        <legend>Columns — click to replace the selected structured operand, or insert into freeform</legend>
        <div class="columns">
          {#each columns as column (column.name)}
            <Chip tone={firstColumn?.name === column.name ? 'accent' : 'default'}>
              <button type="button" class="column-pill" draggable="true" onclick={() => insertColumn(column)} ondragstart={(event) => dragColumn(event, column)} title={column.type}>{column.name}</button>
            </Chip>
          {/each}
        </div>
      </fieldset>

      <div class="helper-groups">
        <FormulaButtonGroup label="Arithmetic">
          <Button type="button" onclick={() => insertText(' + ')}>+</Button><Button type="button" onclick={() => insertText(' - ')}>−</Button><Button type="button" onclick={() => insertText(' * ')}>×</Button><Button type="button" onclick={() => insertText(' / ')}>÷</Button>
          <Button type="button" onclick={() => wrap('(', ')')}>( )</Button><Button type="button" onclick={() => wrap('(', ') / 100')}>%</Button>
        </FormulaButtonGroup>
        <FormulaButtonGroup label="Compare — all data types">
          <Button type="button" onclick={() => insertText(' = ')}>==</Button><Button type="button" onclick={() => insertText(' <> ')}>!=</Button><Button type="button" onclick={() => insertText(' > ')}>&gt;</Button><Button type="button" onclick={() => insertText(' >= ')}>≥</Button><Button type="button" onclick={() => insertText(' < ')}>&lt;</Button><Button type="button" onclick={() => insertText(' <= ')}>≤</Button>
          <Button type="button" onclick={() => insertText(' IS NULL')}>IS NULL</Button><Button type="button" onclick={() => insertText(' IS NOT NULL')}>IS NOT NULL</Button>
        </FormulaButtonGroup>
        <FormulaButtonGroup label="Logic">
          <Button type="button" onclick={() => insertText(' AND ')}>AND</Button><Button type="button" onclick={() => insertText(' OR ')}>OR</Button><Button type="button" onclick={() => wrap('NOT (', ')')}>NOT</Button>
          <Button type="button" onclick={() => replaceExpression('TRUE')}>TRUE</Button><Button type="button" onclick={() => replaceExpression('FALSE')}>FALSE</Button>
        </FormulaButtonGroup>
        <FormulaButtonGroup label="Conditional — no nesting">
          <Button type="button" disabled={!!conditional} onclick={() => startConditional('if')}>IF</Button><Button type="button" disabled={!!conditional} onclick={() => startConditional('switch')}>Switch</Button>
          {#if conditional}<Button type="button" onclick={useFreeform}>Freeform</Button>{/if}
        </FormulaButtonGroup>
        <FormulaButtonGroup label="Number functions">
          <Button type="button" onclick={() => wrap('abs(', ')')}>abs</Button><Button type="button" onclick={() => wrap('round(', ')')}>round</Button><Button type="button" onclick={() => wrap('sqrt(', ')')}>sqrt</Button><Button type="button" onclick={() => wrap('power(', ', 2)')}>power</Button>
        </FormulaButtonGroup>
        <FormulaButtonGroup label="Text functions">
          <Button type="button" onclick={() => wrap('upper(', ')')}>upper</Button><Button type="button" onclick={() => wrap('lower(', ')')}>lower</Button><Button type="button" onclick={() => wrap('length(', ')')}>length</Button><Button type="button" onclick={() => wrap('try_cast(', ' AS DOUBLE)')}>to number</Button>
          <Button type="button" onclick={() => wrap('left(', ', 1)')}>left</Button><Button type="button" onclick={() => wrap('right(', ', 1)')}>right</Button><Button type="button" onclick={() => template('regexp_extract(', ", '", "')")}>regexp_extract</Button>
          <Button type="button" onclick={() => insertText(' || ')}>concat</Button><Button type="button" onclick={() => insertText(quoteLiteral(' '))}>space</Button><Button type="button" onclick={() => insertText(quoteLiteral(', '))}>,</Button><Button type="button" onclick={() => insertText(quoteLiteral('-'))}>-</Button>
        </FormulaButtonGroup>
        <FormulaButtonGroup label="Date functions">
          <Button type="button" onclick={() => wrap('year(', ')')}>year</Button><Button type="button" onclick={() => wrap('month(', ')')}>month</Button><Button type="button" onclick={() => wrap('day(', ')')}>day</Button><Button type="button" onclick={() => template("date_diff('day', ", ', ', ')')}>date_diff</Button>
        </FormulaButtonGroup>
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
  dialog { padding: 0; border: 0; border-radius: var(--radius-2xl); background: var(--surface); box-shadow: var(--shadow-panel); }
  dialog::backdrop { background: rgba(15, 22, 32, 0.4); }
  .formula-menu { width: min(760px, 94vw); max-height: min(820px, 92vh); overflow-y: auto; padding: 20px; display: flex; flex-direction: column; gap: 12px; }
  header { display: flex; align-items: flex-start; justify-content: space-between; gap: 12px; }
  h2 { margin: 0; font-size: 15px; font-weight: 600; }
  header p, .note { margin: 3px 0 0; font-size: 11.5px; color: var(--muted); }
  .close { border: 0; background: transparent; color: var(--muted); font-size: 18px; }
  .field { display: flex; flex-direction: column; gap: 5px; font-size: 11px; color: var(--muted); }
  input, textarea { width: 100%; border: 1px solid var(--control-border); border-radius: var(--radius-md); background: var(--surface); color: var(--ink); }
  input { height: 30px; padding: 0 9px; }
  textarea { padding: 8px 9px; resize: vertical; font: 12px/1.5 var(--font-mono); }
  .modes, .columns { display: flex; flex-wrap: wrap; gap: 6px; }
  fieldset { min-width: 0; margin: 0; padding: 0; border: 0; }
  legend { margin-bottom: 6px; font-size: 11px; color: var(--muted); }
  .column-pill { max-width: 180px; padding: 0; overflow: hidden; border: 0; background: transparent; font: inherit; color: inherit; text-overflow: ellipsis; white-space: nowrap; }
  .helper-groups { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 10px; }
  .conditional-builder { display: grid; grid-template-columns: repeat(3, minmax(0, 1fr)); gap: 8px; padding: 10px; border: 1px solid var(--action-tint-border); border-radius: var(--radius-lg); background: var(--action-tint); }
  .conditional-builder legend { grid-column: 1 / -1; }
  .conditional-builder label { display: grid; gap: 4px; padding: 6px; border: 1px solid transparent; border-radius: var(--radius-md); font-size: 10px; color: var(--muted); }
  .conditional-builder label.active { border-color: var(--action); background: var(--surface); color: var(--action-dark); }
  .conditional-builder .remove-case, .conditional-builder .add-case { align-self: end; height: 30px; border: 1px solid var(--control-border); border-radius: var(--radius-md); background: var(--surface); color: var(--muted); }
  .conditional-builder .remove-case:disabled { opacity: 0.35; }
  .conditional-builder code { grid-column: 1 / -1; overflow-wrap: anywhere; color: var(--ink); }
  code { font-family: var(--font-mono); }
  .error { margin: 0; padding: 8px 10px; border: 1px solid color-mix(in srgb, var(--error) 45%, var(--line)); border-radius: var(--radius-md); color: var(--error); font-size: 12px; }
  footer { display: flex; justify-content: flex-end; gap: 8px; padding-top: 2px; }
  @media (max-width: 720px) { .helper-groups { grid-template-columns: 1fr; } .conditional-builder { grid-template-columns: 1fr; } }
</style>
