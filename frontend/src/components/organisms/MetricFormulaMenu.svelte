<script lang="ts">
  import { tick, untrack } from 'svelte';
  import Button from '../atoms/Button.svelte';
  import Chip from '../atoms/Chip.svelte';
  import FormulaButtonGroup from '../molecules/FormulaButtonGroup.svelte';
  import FormulaHelpers from '../molecules/FormulaHelpers.svelte';
  import { quoteIdentifier } from '../../lib/mutation-sql';
  import type { ColumnInfo } from '../../lib/types';

  type Props = {
    columns: ColumnInfo[];
    expression: string;
    label: string;
    applying: boolean;
    error: string;
    setDialog: (element: HTMLDialogElement | null) => void;
    onClose: () => void;
    onCancelAttempt: (event: Event) => void;
    onApply: (expression: string, label: string) => void;
  };

  let { columns, expression, label, applying, error, setDialog, onClose, onCancelAttempt, onApply }: Props = $props();

  // This dialog is remounted per open, so the draft only needs the values it opened with.
  let formula = $state(untrack(() => expression));
  let caption = $state(untrack(() => label));
  let localError = $state('');
  let textarea = $state<HTMLTextAreaElement | null>(null);
  let dialog = $state<HTMLDialogElement | null>(null);

  function dialogRef(node: HTMLDialogElement) {
    dialog = node;
    setDialog(node);
    return { destroy: () => { dialog = null; setDialog(null); } };
  }

  /* The caret rules: a click with nothing selected wraps the whole formula, which is what makes
     "column, then sum" read the same way round as "sum, then column". */
  function bounds(): [number, number] {
    let start = textarea?.selectionStart ?? formula.length;
    let end = textarea?.selectionEnd ?? start;
    if (start === end && formula.trim()) { start = 0; end = formula.length; }
    return [start, end];
  }

  async function place(value: string, cursor: number, start: number, end: number) {
    formula = `${formula.slice(0, start)}${value}${formula.slice(end)}`;
    await tick();
    textarea?.focus();
    textarea?.setSelectionRange(cursor, cursor);
  }

  async function insertText(value: string) {
    const start = textarea?.selectionStart ?? formula.length;
    const end = textarea?.selectionEnd ?? start;
    await place(value, start + value.length, start, end);
  }

  async function wrap(prefix: string, suffix: string) {
    const [start, end] = bounds();
    const value = `${prefix}${formula.slice(start, end)}${suffix}`;
    await place(value, start + value.length, start, end);
  }

  async function template(prefix: string, middle: string, suffix: string) {
    const [start, end] = bounds();
    const selected = formula.slice(start, end);
    const value = `${prefix}${selected}${middle}${suffix}`;
    await place(value, start + prefix.length + selected.length + middle.length, start, end);
  }

  async function replaceExpression(value: string) {
    formula = value;
    await tick();
    textarea?.focus();
    textarea?.select();
  }

  function insertColumn(column: ColumnInfo) {
    void insertText(quoteIdentifier(column.name));
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

  function onlyRows() {
    const condition = ' FILTER (WHERE )';
    const at = formula.length + condition.length - 1;
    void place(condition, at, formula.length, formula.length);
  }

  function submit() {
    const trimmed = formula.trim();
    if (!trimmed) { localError = 'Enter a formula that returns one value.'; return; }
    if (trimmed.includes('?')) { localError = 'A formula cannot contain ? placeholders.'; return; }
    localError = '';
    onApply(trimmed, caption.trim());
  }
</script>

<dialog use:dialogRef aria-labelledby="metric-formula-title" onclose={onClose} oncancel={onCancelAttempt}>
  <form class="formula-menu" inert={applying} aria-busy={applying} onsubmit={(event) => { event.preventDefault(); submit(); }}>
    <header>
      <div>
        <h2 id="metric-formula-title">Metric formula</h2>
        <p>One aggregate over every row the current filters leave. For example <code>sum("price") / count(*)</code>.</p>
      </div>
      <button type="button" class="close" onclick={() => dialog?.close()} disabled={applying} aria-label="Close">×</button>
    </header>

    <label class="field">Card label
      <input value={caption} oninput={(event) => caption = event.currentTarget.value} maxlength="200" autocomplete="off" placeholder="Defaults to the formula" />
    </label>

    <label class="field">Formula
      <textarea bind:this={textarea} bind:value={formula} rows="3" spellcheck="false" placeholder={'sum("price") / count(*)'} ondragover={(event) => event.preventDefault()} ondrop={dropColumn}></textarea>
    </label>

    <fieldset>
      <legend>Columns — click to insert into the formula</legend>
      <div class="columns">
        {#each columns as column (column.name)}
          <Chip>
            <button type="button" class="column-pill" draggable="true" onclick={() => insertColumn(column)} ondragstart={(event) => dragColumn(event, column)} title={column.type}>{column.name}</button>
          </Chip>
        {/each}
      </div>
    </fieldset>

    <FormulaHelpers onInsert={insertText} onWrap={wrap} onTemplate={template} onReplace={replaceExpression}>
      {#snippet lead()}
        <FormulaButtonGroup label="Aggregate — wraps the formula">
          <Button type="button" onclick={() => wrap('sum(', ')')}>sum</Button><Button type="button" onclick={() => wrap('avg(', ')')}>avg</Button><Button type="button" onclick={() => wrap('median(', ')')}>median</Button><Button type="button" onclick={() => wrap('stddev_samp(', ')')}>stddev</Button>
          <Button type="button" onclick={() => wrap('min(', ')')}>min</Button><Button type="button" onclick={() => wrap('max(', ')')}>max</Button><Button type="button" onclick={() => wrap('count(', ')')}>count</Button><Button type="button" onclick={() => wrap('count(DISTINCT ', ')')}>distinct</Button>
          <Button type="button" onclick={() => replaceExpression('count(*)')}>count(*)</Button><Button type="button" onclick={onlyRows}>only rows where</Button>
        </FormulaButtonGroup>
      {/snippet}
    </FormulaHelpers>

    {#if localError || error}<p class="error" role="alert">{localError || error}</p>{/if}
    <footer>
      <Button type="button" onclick={() => dialog?.close()} disabled={applying}>Cancel</Button>
      <Button type="submit" variant="primary" disabled={applying}>{applying ? 'Checking…' : 'Use formula'}</Button>
    </footer>
  </form>
</dialog>

<style>
  dialog { padding: 0; border: 0; border-radius: var(--radius-2xl); background: var(--surface); box-shadow: var(--shadow-panel); }
  dialog::backdrop { background: rgba(15, 22, 32, 0.4); }
  .formula-menu { width: min(760px, 94vw); max-height: min(820px, 92vh); overflow-y: auto; padding: 20px; display: flex; flex-direction: column; gap: 12px; }
  header { display: flex; align-items: flex-start; justify-content: space-between; gap: 12px; }
  h2 { margin: 0; font-size: 15px; font-weight: 600; }
  header p { margin: 3px 0 0; font-size: 11.5px; color: var(--muted); }
  .close { border: 0; background: transparent; color: var(--muted); font-size: 18px; }
  .field { display: flex; flex-direction: column; gap: 5px; font-size: 11px; color: var(--muted); }
  input, textarea { width: 100%; border: 1px solid var(--control-border); border-radius: var(--radius-md); background: var(--surface); color: var(--ink); }
  input { height: 30px; padding: 0 9px; }
  textarea { padding: 8px 9px; resize: vertical; font: 12px/1.5 var(--font-mono); }
  .columns { display: flex; flex-wrap: wrap; gap: 6px; }
  fieldset { min-width: 0; margin: 0; padding: 0; border: 0; }
  legend { margin-bottom: 6px; font-size: 11px; color: var(--muted); }
  .column-pill { max-width: 180px; padding: 0; overflow: hidden; border: 0; background: transparent; font: inherit; color: inherit; text-overflow: ellipsis; white-space: nowrap; }
  code { font-family: var(--font-mono); }
  .error { margin: 0; padding: 8px 10px; border: 1px solid color-mix(in srgb, var(--error) 45%, var(--line)); border-radius: var(--radius-md); color: var(--error); font-size: 12px; }
  footer { display: flex; justify-content: flex-end; gap: 8px; padding-top: 2px; }
</style>
