<script lang="ts">
  import type { Snippet } from 'svelte';
  import Icon from '../atoms/Icon.svelte';
  import IconButton from '../atoms/IconButton.svelte';

  type Props = {
    title: string;
    versionLabel: string;
    canPreviousVersion: boolean;
    canNextVersion: boolean;
    onPreviousVersion: () => void;
    onNextVersion: () => void;
    versionOpen: boolean;
    onToggleVersions: () => void;
    versionMenu?: Snippet;
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
    canUndo: boolean;
    onUndo: () => void;
    exportOpen: boolean;
    exportMenu?: Snippet;
    inert?: boolean;
  };
  let {
    title, versionLabel, canPreviousVersion, canNextVersion, onPreviousVersion, onNextVersion,
    versionOpen, onToggleVersions, versionMenu,
    showMeta, rows, ms, showRefresh, onRefresh, onExport, loadingData, canExport, exporting,
    pendingCount, onStopRecording, canUndo, onUndo, exportOpen, exportMenu, inert = false
  }: Props = $props();
</script>

<header class="head" {inert}>
  <div class="ident">
    <h1>{title}</h1>
    {#if versionLabel}
      <span class="sep" aria-hidden="true">|</span>
      <div class="version-anchor">
        <button
          type="button" class="version" class:open={versionOpen}
          data-menu-trigger data-tip="Browse this View's versions"
          aria-expanded={versionOpen} aria-haspopup="true"
          onclick={onToggleVersions}
        >
          {versionLabel}
          <span class="caret" aria-hidden="true"><Icon name="chevron" size={10} /></span>
        </button>
        {#if versionMenu}{@render versionMenu()}{/if}
      </div>
    {/if}
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
      {#if pendingCount}
        <IconButton type="button" icon="undo" label="Undo last change" onclick={onUndo} disabled={!canUndo || loadingData} />
        <IconButton type="button" active glyph="■" label={`Stop recording (${pendingCount} pending changes)`} onclick={onStopRecording} disabled={loadingData} />
      {/if}
      {#if showRefresh}
        <div class="export-anchor">
          <IconButton
            type="button" icon="download" active={exportOpen}
            label={exporting ? 'Exporting…' : 'Export data'}
            data-menu-trigger aria-expanded={exportOpen} aria-haspopup="true"
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
  .version-anchor { position: relative; display: flex; align-self: center; }
  .version {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    height: 21px;
    padding: 0 6px 0 8px;
    border: 1px solid var(--control-border);
    border-radius: 999px;
    background: var(--surface);
    font: 11px var(--font-mono);
    color: var(--ink-2);
    white-space: nowrap;
    transition: border-color 120ms ease, background 120ms ease, color 120ms ease;
  }
  .version:hover { border-color: var(--faint); color: var(--ink); }
  .version.open { border-color: var(--action); background: var(--action-tint); color: var(--action-dark); }
  .caret { display: flex; color: var(--glyph); transform: rotate(90deg); transition: transform 180ms cubic-bezier(0.32, 0.72, 0, 1); }
  .version.open .caret { transform: rotate(270deg); color: var(--action); }
  .meta { font: 11px var(--font-mono); color: var(--muted); white-space: nowrap; }
  .actions, .version-actions { display: flex; align-items: center; gap: 6px; }
  .export-anchor { position: relative; display: flex; }
  .version-actions { gap: 2px; }
</style>
