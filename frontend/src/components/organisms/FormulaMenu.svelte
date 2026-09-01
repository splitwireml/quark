<script lang="ts">
  import { tick } from 'svelte';
  import Button from '../atoms/Button.svelte';
  import Chip from '../atoms/Chip.svelte';
  import { quoteIdentifier, quoteLiteral } from '../../lib/mutation-sql';
  import type { ColumnInfo } from '../../lib/types';

  type Mode = 'blank' | 'text' | 'formula';
  type FormulaKind = 'numeric' | 'text' | 'date' | 'logical' | 'other';
  type Props = {
    columns: ColumnInfo[];
    boundaryLabel: string;
    applying: boolean;
    error: string;
    setDialog: (element: HTMLDialogElement | null) => void;
    onClose: () => void;
    onCancelAttempt: (event: Event) => void;
    onApply: (name: string, expression: string) => void;
  };

  let { columns, boundaryLabel, applying, error, setDialog, onClose, onCancelAttempt, onApply }: Props = $props();
  let mode = $state<Mode>('blank');
  let name = $state('');
  let repeatingText = $state('');
  let formula = $state('');
  let localError = $state('');
  let textarea = $state<HTMLTextAreaElement | null>(null);
  let firstColumnName = $state<string | null>(null);
  let firstColumn = $derived(columns.find((column) => column.name === firstColumnName));
  let firstKind = $derived(firstColumn ? columnKind(firstColumn) : null);

  function dialogRef(node: HTMLDialogElement) {
    setDialog(node);
    return { destroy: () => setDialog(null) };
  }

  function columnKind(column: ColumnInfo): FormulaKind {
    if (column.numeric) return 'numeric';
    if (column.profile_kind === 'date' || /DATE|TIME/i.test(column.type)) return 'date';
    if (/BOOL/i.test(column.type)) return 'logical';
    if (/CHAR|TEXT|STRING|VARCHAR/i.test(column.type)) return 'text';
    return 'other';
  }

  function incompatible(column: ColumnInfo): boolean {
    return firstKind !== null && columnKind(column) !== firstKind;
  }

  async function insertText(value: string) {
    const start = textarea?.selectionStart ?? formula.length;
    const end = textarea?.selectionEnd ?? start;
    formula = `${formula.slice(0, start)}${value}${formula.slice(end)}`;
    await tick();
    textarea?.focus();
    textarea?.setSelectionRange(start + value.length, start + value.length);
  }

  function insertColumn(column: ColumnInfo) {
    if (incompatible(column)) return;
    firstColumnName ??= column.name;
    void insertText(quoteIdentifier(column.name));
  }

  async function wrap(prefix: string, suffix: string) {
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

  function dragColumn(event: DragEvent, column: ColumnInfo) {
    if (incompatible(column)) { event.preventDefault(); return; }
    event.dataTransfer?.setData('text/plain', quoteIdentifier(column.name));
    event.dataTransfer?.setData('application/x-quark-column', column.name);
  }

  function dropColumn(event: DragEvent) {
    event.preventDefault();
    const value = event.dataTransfer?.getData('text/plain');
    const columnName = event.dataTransfer?.getData('application/x-quark-column');
    if (columnName && !firstColumnName) firstColumnName = columnName;
    if (value) void insertText(value);
  }

  function submit() {
    const trimmedName = name.trim();
    if (!trimmedName) { localError = 'Enter a new column name.'; return; }
    if (columns.some((column) => column.name.toLocaleLowerCase() === trimmedName.toLocaleLowerCase())) { localError = 'Column names must be unique.'; return; }
    if (mode === 'formula' && !formula.trim()) { localError = 'Enter a formula.'; return; }
    localError = '';
    onApply(trimmedName, mode === 'blank' ? 'NULL' : mode === 'text' ? quoteLiteral(repeatingText) : formula.trim());
  }
</script>

<dialog use:dialogRef aria-labelledby="formula-title" onclose={onClose} oncancel={onCancelAttempt}>
  <form class="formula-menu" inert={applying} aria-busy={applying} onsubmit={(event) => { event.preventDefault(); submit(); }}>
    <header>
      <div><h2 id="formula-title">Insert column</h2><p>{boundaryLabel}</p></div>
      <button type="button" class="close" onclick={() => (document.activeElement?.closest('dialog') as HTMLDialogElement)?.close()} disabled={applying} aria-label="Close">×</button>
    </header>

    <label class="field">New column name
      <input value={name} oninput={(event) => name = event.currentTarget.value} maxlength="128" autocomplete="off" />
    </label>

    <div class="modes" role="group" aria-label="Column value type">
      <Button type="button" active={mode === 'blank'} aria-pressed={mode === 'blank'} onclick={() => mode = 'blank'}>Blank</Button>
      <Button type="button" active={mode === 'text'} aria-pressed={mode === 'text'} onclick={() => mode = 'text'}>Repeating text</Button>
      <Button type="button" active={mode === 'formula'} aria-pressed={mode === 'formula'} onclick={() => mode = 'formula'}>Formula</Button>
    </div>

    {#if mode === 'blank'}
      <p class="note">Every row will contain <code>NULL</code>.</p>
    {:else if mode === 'text'}
      <label class="field">Text repeated for every row
        <input value={repeatingText} oninput={(event) => repeatingText = event.currentTarget.value} />
      </label>
    {:else}
      <label class="field">Formula
        <textarea bind:this={textarea} bind:value={formula} rows="5" spellcheck="false" placeholder='For example: "price" * 1.05' ondragover={(event) => event.preventDefault()} ondrop={dropColumn}></textarea>
      </label>

      <fieldset>
        <legend>Columns — click or drag into the formula</legend>
        <div class="columns">
          {#each columns as column (column.name)}
            {@const disabled = incompatible(column)}
            <Chip tone={firstColumn?.name === column.name ? 'accent' : 'default'} aria-disabled={disabled}>
              <button type="button" class="column-pill" disabled={disabled} draggable={!disabled} onclick={() => insertColumn(column)} ondragstart={(event) => dragColumn(event, column)} title={disabled ? `Choose another ${firstKind} column` : column.type}>{column.name}</button>
            </Chip>
          {/each}
        </div>
      </fieldset>

      {#if firstColumn}
        <div class="helpers" aria-label={`${firstKind} formula helpers`}>
          {#if firstKind === 'numeric'}
            <Button type="button" onclick={() => insertText(' + ')}>+</Button><Button type="button" onclick={() => insertText(' - ')}>−</Button><Button type="button" onclick={() => insertText(' * ')}>×</Button><Button type="button" onclick={() => insertText(' / ')}>÷</Button>
            <Button type="button" onclick={() => wrap('(', ')')}>( )</Button><Button type="button" onclick={() => wrap('(', ') / 100')}>%</Button>
            <Button type="button" onclick={() => wrap('abs(', ')')}>abs</Button><Button type="button" onclick={() => wrap('round(', ')')}>round</Button><Button type="button" onclick={() => wrap('sqrt(', ')')}>sqrt</Button><Button type="button" onclick={() => wrap('power(', ', 2)')}>power</Button>
          {:else if firstKind === 'text'}
            <Button type="button" onclick={() => wrap('upper(', ')')}>upper</Button><Button type="button" onclick={() => wrap('lower(', ')')}>lower</Button>
            <Button type="button" onclick={() => wrap('left(', ', 1)')}>left</Button><Button type="button" onclick={() => wrap('right(', ', 1)')}>right</Button><Button type="button" onclick={() => template('regexp_extract(', ", '", "')")}>regexp_extract</Button>
            <Button type="button" onclick={() => insertText(' || ')}>||</Button><Button type="button" onclick={() => insertText(quoteLiteral(' '))}>space</Button><Button type="button" onclick={() => insertText(quoteLiteral(', '))}>,</Button><Button type="button" onclick={() => insertText(quoteLiteral('-'))}>-</Button>
          {:else if firstKind === 'date'}
            <Button type="button" onclick={() => wrap('year(', ')')}>year</Button><Button type="button" onclick={() => wrap('month(', ')')}>month</Button><Button type="button" onclick={() => wrap('day(', ')')}>day</Button><Button type="button" onclick={() => template("date_diff('day', ", ', ', ')')}>date_diff</Button>
          {/if}
          <Button type="button" onclick={() => insertText(' = ')}>=</Button><Button type="button" onclick={() => insertText(' <> ')}>≠</Button><Button type="button" onclick={() => insertText(' > ')}>&gt;</Button><Button type="button" onclick={() => insertText(' >= ')}>≥</Button><Button type="button" onclick={() => insertText(' < ')}>&lt;</Button><Button type="button" onclick={() => insertText(' <= ')}>≤</Button>
          <Button type="button" onclick={() => insertText(' AND ')}>AND</Button><Button type="button" onclick={() => insertText(' OR ')}>OR</Button><Button type="button" onclick={() => wrap('NOT (', ')')}>NOT</Button>
        </div>
      {:else}
        <p class="note">Add a column to reveal compatible formula helpers.</p>
      {/if}
    {/if}

    {#if localError || error}<p class="error" role="alert">{localError || error}</p>{/if}
    <footer>
      <Button type="button" onclick={() => (document.activeElement?.closest('dialog') as HTMLDialogElement)?.close()} disabled={applying}>Cancel</Button>
      <Button type="submit" variant="primary" disabled={applying}>{applying ? 'Checking…' : 'Insert column'}</Button>
    </footer>
  </form>
</dialog>

<style>
  dialog { padding: 0; border: 0; border-radius: var(--radius-2xl); background: var(--surface); box-shadow: var(--shadow-panel); }
  dialog::backdrop { background: rgba(15, 22, 32, 0.4); }
  .formula-menu { width: min(660px, 92vw); max-height: min(760px, 90vh); overflow-y: auto; padding: 20px; display: flex; flex-direction: column; gap: 12px; }
  header { display: flex; align-items: flex-start; justify-content: space-between; gap: 12px; }
  h2 { margin: 0; font-size: 15px; font-weight: 600; }
  header p, .note { margin: 3px 0 0; font-size: 11.5px; color: var(--muted); }
  .close { border: 0; background: transparent; color: var(--muted); font-size: 18px; }
  .field { display: flex; flex-direction: column; gap: 5px; font-size: 11px; color: var(--muted); }
  input, textarea { width: 100%; border: 1px solid var(--control-border); border-radius: var(--radius-md); background: var(--surface); color: var(--ink); }
  input { height: 30px; padding: 0 9px; }
  textarea { padding: 8px 9px; resize: vertical; font: 12px/1.5 var(--font-mono); }
  .modes, .helpers, .columns { display: flex; flex-wrap: wrap; gap: 6px; }
  fieldset { min-width: 0; margin: 0; padding: 0; border: 0; }
  legend { margin-bottom: 6px; font-size: 11px; color: var(--muted); }
  .column-pill { max-width: 180px; padding: 0; overflow: hidden; border: 0; background: transparent; font: inherit; color: inherit; text-overflow: ellipsis; white-space: nowrap; }
  .column-pill:disabled { opacity: 0.35; }
  .helpers { padding-top: 2px; }
  code { font-family: var(--font-mono); }
  .error { margin: 0; padding: 8px 10px; border: 1px solid color-mix(in srgb, var(--error) 45%, var(--line)); border-radius: var(--radius-md); color: var(--error); font-size: 12px; }
  footer { display: flex; justify-content: flex-end; gap: 8px; padding-top: 2px; }
</style>
