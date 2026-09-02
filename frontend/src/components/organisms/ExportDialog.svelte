<script lang="ts">
  import Button from '../atoms/Button.svelte';
  import Checkbox from '../atoms/Checkbox.svelte';
  import type { ExportFormat, ExportOption } from '../../lib/types';

  type Props = {
    format: ExportFormat;
    current: ExportOption | null;
    options: ExportOption[];
    selectedKeys: string[];
    loading: boolean;
    exporting: boolean;
    error: string;
    setDialog: (element: HTMLDialogElement | null) => void;
    setFormat: (format: ExportFormat) => void;
    onToggle: (key: string, checked: boolean) => void;
    onClose: () => void;
    onCancelAttempt: (event: Event) => void;
    onBackdropClick: (event: MouseEvent) => void;
    onCancel: () => void;
    onExport: () => void;
  };

  let {
    format, current, options, selectedKeys, loading, exporting, error, setDialog, setFormat,
    onToggle, onClose, onCancelAttempt, onBackdropClick, onCancel, onExport
  }: Props = $props();

  let sources = $derived([...new Set(options.map((option) => option.source))]);
  let selectionCount = $derived(format === 'csv' ? (current ? 1 : 0) : selectedKeys.length);
  let canExport = $derived(!!current && !loading && !exporting && selectionCount > 0);

  function dialogRef(node: HTMLDialogElement) {
    setDialog(node);
    return { destroy: () => setDialog(null) };
  }
</script>

<dialog use:dialogRef aria-labelledby="export-title" onclose={onClose} oncancel={onCancelAttempt} onclick={onBackdropClick}>
  <form class="export-dialog" onsubmit={(event) => { event.preventDefault(); onExport(); }}>
    <header>
      <div><h2 id="export-title">Export data</h2><p>Download the full result, not only this page.</p></div>
      <button type="button" class="close" onclick={onCancel} disabled={exporting} aria-label="Close export dialog">×</button>
    </header>

    <fieldset class="formats">
      <legend>Format</legend>
      <label><input type="radio" name="export-format" value="csv" checked={format === 'csv'} onchange={() => setFormat('csv')} />CSV</label>
      <label><input type="radio" name="export-format" value="xlsx" checked={format === 'xlsx'} onchange={() => setFormat('xlsx')} />Excel workbook</label>
    </fieldset>

    {#if format === 'csv'}
      <section class="current">
        <strong>Current View</strong>
        <span>{current?.name ?? 'No result available'}</span>
        <small>Includes current filters, sorting, joins, aggregates, and computed columns.</small>
      </section>
    {:else}
      <div class="sheets" aria-busy={loading}>
        {#if current}
          <section>
            <h3>Current View</h3>
            <Checkbox checked={selectedKeys.includes(current.key)} label={current.name} disabled={exporting} onchange={(checked) => onToggle(current.key, checked)} />
          </section>
        {/if}

        {#if loading}
          <p class="state">Loading project Views…</p>
        {:else}
          {#each sources as source (source)}
            <section>
              <h3>{source}</h3>
              {#each options.filter((option) => option.source === source) as option (option.key)}
                <Checkbox checked={selectedKeys.includes(option.key)} label={option.name} disabled={exporting} onchange={(checked) => onToggle(option.key, checked)} />
              {/each}
            </section>
          {/each}
          {#if options.length === 0}<p class="state">No additional Views are available.</p>{/if}
        {/if}
      </div>
    {/if}

    {#if error}<p class="error" role="alert">{error}</p>{/if}
    <footer>
      <span>{selectionCount} {selectionCount === 1 ? 'View' : 'Views'}</span>
      <Button type="button" onclick={onCancel} disabled={exporting}>Cancel</Button>
      <Button type="submit" variant="primary" disabled={!canExport}>{exporting ? 'Exporting…' : 'Export'}</Button>
    </footer>
  </form>
</dialog>

<style>
  dialog { padding: 0; border: 0; border-radius: var(--radius-2xl); background: var(--surface); box-shadow: var(--shadow-panel); }
  dialog::backdrop { background: rgba(15, 22, 32, 0.4); }
  .export-dialog { width: min(520px, 92vw); max-height: min(720px, 90vh); overflow: hidden; padding: 20px; display: flex; flex-direction: column; gap: 14px; }
  header { display: flex; align-items: flex-start; justify-content: space-between; gap: 12px; }
  h2 { margin: 0; font-size: 15px; font-weight: 600; }
  header p { margin: 3px 0 0; font-size: 11.5px; color: var(--muted); }
  .close { border: 0; background: transparent; color: var(--muted); font-size: 18px; }
  fieldset { margin: 0; padding: 0; border: 0; }
  legend, h3 { margin: 0 0 7px; font: 10px var(--font-mono); letter-spacing: 0.06em; text-transform: uppercase; color: var(--faint); }
  .formats { display: flex; flex-wrap: wrap; gap: 8px; }
  .formats legend { width: 100%; }
  .formats label { display: flex; align-items: center; gap: 6px; padding: 7px 9px; border: 1px solid var(--control-border); border-radius: var(--radius-md); font-size: 12px; cursor: pointer; }
  .current { display: flex; flex-direction: column; gap: 3px; padding: 12px; border: 1px solid var(--line); border-radius: var(--radius-md); }
  .current strong { font-size: 12px; }
  .current span { font-size: 12.5px; color: var(--ink); }
  .current small { color: var(--muted); line-height: 1.4; }
  .sheets { min-height: 100px; max-height: 380px; overflow-y: auto; display: flex; flex-direction: column; gap: 12px; padding: 10px; border: 1px solid var(--line); border-radius: var(--radius-md); }
  .sheets section { display: flex; flex-direction: column; gap: 7px; }
  .state { margin: 0; padding: 10px 2px; font-size: 12px; color: var(--muted); }
  .error { margin: 0; padding: 8px 10px; border: 1px solid color-mix(in srgb, var(--error) 45%, var(--line)); border-radius: var(--radius-md); color: var(--error); font-size: 12px; }
  footer { display: flex; align-items: center; justify-content: flex-end; gap: 8px; }
  footer span { margin-right: auto; font-size: 11px; color: var(--faint); }
</style>
