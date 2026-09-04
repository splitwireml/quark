<script lang="ts">
  import type { Snippet } from 'svelte';
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
    exportOpen: boolean;
    exportMenu?: Snippet;
    inert?: boolean;
  };
  let {
    title, versionLabel, canPreviousVersion, canNextVersion, onPreviousVersion, onNextVersion,
    showMeta, rows, ms, showRefresh, onRefresh, onExport, loadingData, canExport, exporting,
    pendingCount, onStopRecording, exportOpen, exportMenu, inert = false
  }: Props = $props();
</script>

<header class="head" {inert}>
  <div class="ident">
    <h1>{title}</h1>
    {#if versionLabel}<span class="sep" aria-hidden="true">|</span><span class="version">{versionLabel}</span>{/if}
    {#if showMeta}<span class="sep" aria-hidden="true">|</span><span class="meta">{rows} rows</span><span class="sep" aria-hidden="true">|</span><span class="meta">{ms} ms</span>{/if}
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
        <div class="export-anchor">
          <IconButton
            type="button" icon="download" active={exportOpen}
            label={exporting ? 'Exporting…' : 'Export data'}
            data-export-trigger aria-expanded={exportOpen} aria-haspopup="true"
            onclick={(event: MouseEvent) => onExport(event.currentTarget as HTMLButtonElement)}
            disabled={!canExport || exporting}
          />
          {#if exportMenu}{@render exportMenu()}{/if}
        </div>
        <IconButton type="button" glyph="↻" label="Refresh data" onclick={onRefresh} disabled={loadingData} />
      {/if}
    </div>
  {/if}
</header>

<style>
  .head { flex: none; display: flex; align-items: center; justify-content: space-between; gap: 12px; padding: 14px 20px 6px; }
  .ident { display: flex; align-items: baseline; gap: 8px; min-width: 0; }
  h1 { margin: 0; font-size: 15px; font-weight: 600; letter-spacing: -0.01em; color: var(--ink); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .sep { color: var(--faint); font-size: 11px; }
  .version { font: 11px var(--font-mono); color: var(--ink-2); white-space: nowrap; }
  .meta { font: 11px var(--font-mono); color: var(--muted); white-space: nowrap; }
  .actions, .version-actions { display: flex; align-items: center; gap: 6px; }
  .export-anchor { position: relative; display: flex; }
  .version-actions { gap: 2px; }
</style>
