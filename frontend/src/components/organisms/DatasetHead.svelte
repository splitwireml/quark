<script lang="ts">
  import IconButton from '../atoms/IconButton.svelte';

  type Props = {
    title: string;
    versionLabel: string;
    canPreviousVersion: boolean;
    canNextVersion: boolean;
    onPreviousVersion: () => void;
    onNextVersion: () => void;
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
  let {
    title, versionLabel, canPreviousVersion, canNextVersion, onPreviousVersion, onNextVersion,
    showMeta, rows, ms, showRefresh, onRefresh, onExport, loadingData, canExport, exporting,
    pendingCount, onStopRecording, inert = false
  }: Props = $props();
</script>

<header class="head" {inert}>
  <div>
    <h1>{title}</h1>
    {#if versionLabel}<p class="version">{versionLabel}</p>{/if}
    {#if showMeta}<p class="meta"><span>{rows} rows</span><span>{ms} ms</span></p>{/if}
  </div>
  {#if versionLabel || showRefresh || pendingCount}
    <div class="actions">
      {#if versionLabel}
        <div class="version-actions" role="group" aria-label="Version navigation">
          <IconButton type="button" glyph="←" label="Previous version" onclick={onPreviousVersion} disabled={!canPreviousVersion} />
          <IconButton type="button" glyph="→" label="Next version" onclick={onNextVersion} disabled={!canNextVersion} />
        </div>
      {/if}
      {#if pendingCount}<IconButton type="button" active glyph="■" label={`Stop recording (${pendingCount} pending changes)`} onclick={onStopRecording} disabled={loadingData} />{/if}
      {#if showRefresh}
        <button class="export" onclick={(event) => onExport(event.currentTarget as HTMLButtonElement)} disabled={!canExport || exporting} aria-label="Export data" title="Export data">{exporting ? 'Exporting…' : 'Export'}</button>
        <IconButton type="button" glyph="↻" label="Refresh data" onclick={onRefresh} disabled={loadingData} />
      {/if}
    </div>
  {/if}
</header>

<style>
  .head { flex: none; display: flex; align-items: center; justify-content: space-between; gap: 12px; padding: 16px 20px 4px; }
  h1 { margin: 0; font-size: 16px; font-weight: 600; letter-spacing: -0.01em; color: var(--ink); }
  .version { margin: 3px 0 0; font: 11px var(--font-mono); color: var(--ink-2); }
  .meta { margin: 4px 0 0; display: flex; gap: 10px; font-family: var(--font-mono); font-size: 11px; color: var(--muted); }
  .actions, .version-actions { display: flex; align-items: center; gap: 6px; }
  .version-actions { gap: 2px; }
  .export { height: 26px; padding: 0 10px; border-radius: var(--radius-md); border: 1px solid var(--control-border); background: var(--surface); color: var(--muted); font-size: 11.5px; }
  .export:hover:not(:disabled) { border-color: var(--faint); color: var(--ink); }
  .export:disabled { opacity: 0.5; }
</style>
