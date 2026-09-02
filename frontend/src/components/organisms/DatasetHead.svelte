<script lang="ts">
  type Props = {
    title: string;
    showMeta: boolean;
    rows: string;
    ms: string;
    showRefresh: boolean;
    onRefresh: () => void;
    onExport: (trigger: HTMLButtonElement) => void;
    loadingData: boolean;
    canExport: boolean;
    exporting: boolean;
    pendingCount: number;
    onStopRecording: () => void;
    inert?: boolean;
  };
  let { title, showMeta, rows, ms, showRefresh, onRefresh, onExport, loadingData, canExport, exporting, pendingCount, onStopRecording, inert = false }: Props = $props();
</script>

<header class="head" {inert}>
  <div>
    <h1>{title}</h1>
    {#if showMeta}<p class="meta"><span>{rows} rows</span><span>{ms} ms</span></p>{/if}
  </div>
  {#if showRefresh || pendingCount}
    <div class="actions">
      {#if pendingCount}<button class="recording" onclick={onStopRecording} disabled={loadingData}>Stop recording ({pendingCount})</button>{/if}
      {#if showRefresh}
        <button class="export" onclick={(event) => onExport(event.currentTarget as HTMLButtonElement)} disabled={!canExport || exporting} aria-label="Export data" title="Export data">{exporting ? 'Exporting…' : 'Export'}</button>
        <button class="refresh" onclick={onRefresh} disabled={loadingData} aria-label="Refresh data" title="Refresh data">↻</button>
      {/if}
    </div>
  {/if}
</header>

<style>
  .head { flex: none; display: flex; align-items: center; justify-content: space-between; gap: 12px; padding: 16px 20px 4px; }
  h1 { margin: 0; font-size: 16px; font-weight: 600; letter-spacing: -0.01em; color: var(--ink); }
  .meta { margin: 4px 0 0; display: flex; gap: 10px; font-family: var(--font-mono); font-size: 11px; color: var(--muted); }
  .actions { display: flex; align-items: center; gap: 6px; }
  .actions button { height: 26px; border-radius: var(--radius-md); border: 1px solid var(--control-border); background: var(--surface); color: var(--muted); }
  .actions button:hover:not(:disabled) { border-color: var(--faint); color: var(--ink); }
  .actions button:disabled { opacity: 0.5; }
  .export { padding: 0 10px; font-size: 11.5px; }
  .recording { padding: 0 10px; border-color: var(--action-tint-border) !important; background: var(--action-tint) !important; color: var(--action-dark) !important; font-size: 11.5px; }
  .refresh { width: 26px; }
</style>
