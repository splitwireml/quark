<script lang="ts">
  import BusyBar from '../atoms/BusyBar.svelte';
  import Button from '../atoms/Button.svelte';

  type Props = {
    mutating: boolean;
    uploading?: boolean;
    attachPath: string;
    onUpload: (event: Event) => void;
    onAttach: (event: SubmitEvent) => void;
    setAttachPath: (value: string) => void;
    idPrefix?: string;
    showUpload?: boolean;
  };
  let { mutating, uploading = false, attachPath, onUpload, onAttach, setAttachPath, idPrefix = 'database-path', showUpload = true }: Props = $props();
  // The trigger becomes the upload's progress, so remember which file it took.
  let picked = $state('');
  let busy = $derived(uploading && !!picked);
  function choose(event: Event) {
    picked = (event.currentTarget as HTMLInputElement).files?.[0]?.name ?? '';
    onUpload(event);
  }
</script>

<div class="disclosure">
  {#if showUpload}
    <label class="upload" class:disabled={mutating && !busy} aria-busy={busy}>
      <span class="text" aria-live="polite">{busy ? `Uploading ${picked}…` : 'Upload file'}</span>
      <input type="file" accept=".csv,.tsv,.parquet,.json,.ndjson,.jsonl,.xlsx,.duckdb,.db" onchange={choose} disabled={mutating} />
      {#if busy}<BusyBar />{/if}
    </label>
  {/if}
  <form onsubmit={onAttach}>
    <label for={idPrefix}>Attach database path</label>
    <div class="row">
      <input id={idPrefix} value={attachPath} oninput={(event) => setAttachPath((event.currentTarget as HTMLInputElement).value)} placeholder="/data/example.duckdb" disabled={mutating} />
      <Button type="submit" disabled={mutating || !attachPath.trim()}>Attach</Button>
    </div>
  </form>
</div>

<style>
  .disclosure { display: flex; flex-direction: column; gap: 10px; }
  .upload {
    position: relative;
    overflow: hidden;
    display: flex; align-items: center; justify-content: center;
    height: 30px;
    border-radius: var(--radius-lg);
    border: 1px solid var(--control-border);
    background: var(--surface);
    font-size: 12.5px;
    color: var(--ink-2);
    cursor: pointer;
  }
  .upload:hover { border-color: var(--faint); }
  .upload.disabled { opacity: 0.5; pointer-events: none; }
  .upload[aria-busy='true'] { pointer-events: none; color: var(--muted); }
  .upload .text { min-width: 0; padding: 0 10px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .upload input { position: absolute; width: 1px; height: 1px; opacity: 0; }
  form { display: flex; flex-direction: column; gap: 5px; }
  form label { font-size: 11px; color: var(--muted); }
  .row { display: flex; gap: 6px; }
  .row input {
    flex: 1; min-width: 0;
    height: 30px;
    padding: 0 9px;
    border-radius: var(--radius-lg);
    border: 1px solid var(--control-border);
    background: var(--surface);
    font-family: var(--font-mono);
    font-size: 11.5px;
    color: var(--ink);
  }
</style>
