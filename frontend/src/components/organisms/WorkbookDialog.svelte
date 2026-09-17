<script lang="ts">
  import Button from '../atoms/Button.svelte';
  import type { WorkbookPreview } from '../../lib/types';

  type Props = {
    preview: WorkbookPreview;
    selectedSheets: string[];
    confirming: boolean;
    setDialog: (el: HTMLDialogElement | null) => void;
    onClose: () => void;
    onCancelAttempt: (event: Event) => void;
    onBackdropClick: (event: MouseEvent) => void;
    onToggleSheet: (sheet: string, checked: boolean) => void;
    onCancel: () => void;
    onConfirm: () => void;
  };
  let { preview, selectedSheets, confirming, setDialog, onClose, onCancelAttempt, onBackdropClick, onToggleSheet, onCancel, onConfirm }: Props = $props();

  function dialogRef(node: HTMLDialogElement) {
    setDialog(node);
    return { destroy: () => setDialog(null) };
  }
</script>

<dialog use:dialogRef aria-labelledby="workbook-title" onclose={onClose} oncancel={onCancelAttempt} onclick={onBackdropClick}>
  <section class="workbook-dialog">
    <h2 id="workbook-title">Workbook detected</h2>
    <p>Choose worksheets. Each selected worksheet becomes a View.</p>
    <div class="sheets">
      {#each preview.sheets as sheet (sheet)}
        <label class="row">
          <input type="checkbox" checked={selectedSheets.includes(sheet)} onchange={(event) => onToggleSheet(sheet, (event.currentTarget as HTMLInputElement).checked)} disabled={confirming} />
          <span class="box" aria-hidden="true">{#if selectedSheets.includes(sheet)}✓{/if}</span>
          {sheet}
        </label>
      {/each}
    </div>
    <p class="count">{selectedSheets.length} of {preview.sheets.length} selected</p>
    <footer>
      <Button onclick={onCancel} disabled={confirming}>Cancel</Button>
      <Button variant="primary" onclick={onConfirm} disabled={confirming || selectedSheets.length === 0}>{confirming ? 'Adding…' : 'Continue'}</Button>
    </footer>
  </section>
</dialog>

<style>
  dialog { padding: 0; border: none; border-radius: var(--radius-2xl); box-shadow: var(--shadow-panel); background: var(--surface); }
  dialog::backdrop { background: rgba(15, 22, 32, 0.4); }
  .workbook-dialog { width: min(420px, 90vw); padding: 20px; display: flex; flex-direction: column; gap: 12px; }
  h2 { margin: 0; font-size: 15px; font-weight: 600; }
  p { margin: 0; font-size: 12.5px; color: var(--muted); }
  .sheets { display: flex; flex-direction: column; gap: 1px; max-height: 220px; overflow-y: auto; border: 1px solid var(--line); border-radius: var(--radius-md); padding: 4px; }
  .row { display: flex; align-items: center; gap: 8px; height: 28px; padding: 0 6px; border-radius: var(--radius-sm); font-size: 12.5px; cursor: pointer; }
  .row:hover { background: var(--surface-hover); }
  .row input { position: absolute; width: 1px; height: 1px; opacity: 0; }
  .box { display: inline-flex; align-items: center; justify-content: center; flex: none; width: 12px; height: 12px; border-radius: 2px; border: 1px solid var(--glyph); color: #fff; font-size: 8px; }
  input:checked + .box { background: var(--action); border-color: var(--action); }
  .count { font-size: 11px; color: var(--faint); }
  footer { display: flex; justify-content: flex-end; gap: 8px; }
</style>
