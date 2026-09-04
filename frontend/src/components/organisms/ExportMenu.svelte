<script lang="ts">
  import Button from '../atoms/Button.svelte';
  import Checkbox from '../atoms/Checkbox.svelte';
  import Icon from '../atoms/Icon.svelte';
  import type { ExportFormat, ExportOption } from '../../lib/types';

  type Props = {
    open: boolean;
    current: ExportOption | null;
    options: ExportOption[];
    selectedKeys: string[];
    loading: boolean;
    exporting: boolean;
    error: string;
    setFormat: (format: ExportFormat) => void;
    onToggle: (key: string, checked: boolean) => void;
    onExport: () => void;
    onClose: () => void;
  };

  let {
    open, current, options, selectedKeys, loading, exporting, error,
    setFormat, onToggle, onExport, onClose
  }: Props = $props();

  // The panel holds one of two panes; choosing Excel deepens this surface
  // instead of replacing it, so the frame morphs and the pane cross-fades.
  let pane = $state<'root' | 'sheets'>('root');
  let paneHeight = $state(0);
  let measured = $state(false);
  let panel = $state<HTMLDivElement | null>(null);

  let sources = $derived([...new Set(options.map((option) => option.source))]);
  let sheetCount = $derived(selectedKeys.length);
  let canExport = $derived(!!current && !loading && !exporting && sheetCount > 0);

  $effect(() => {
    if (open) return;
    pane = 'root';
    measured = false;
  });

  // Height is animated, so the first measurement must land before transitions start.
  $effect(() => {
    if (!paneHeight || measured) return;
    const frame = requestAnimationFrame(() => measured = true);
    return () => cancelAnimationFrame(frame);
  });

  function openSheets() {
    setFormat('xlsx');
    pane = 'sheets';
  }

  function backToRoot() {
    setFormat('csv');
    pane = 'root';
  }

  function exportCsv() {
    setFormat('csv');
    onExport();
  }

  function onKeydown(event: KeyboardEvent) {
    if (event.key !== 'Escape' || exporting) return;
    event.stopPropagation();
    if (pane === 'sheets') { backToRoot(); return; }
    onClose();
  }

  function onPointerDown(event: PointerEvent) {
    const target = event.target as Node | null;
    if (exporting || !panel || !target || panel.contains(target)) return;
    // The trigger toggles itself; let its own handler decide.
    if ((target as HTMLElement).closest?.('[data-export-trigger]')) return;
    onClose();
  }
</script>

<svelte:window onpointerdown={open ? onPointerDown : undefined} onkeydowncapture={open ? onKeydown : undefined} />

{#if open}
  <div
    class="panel"
    class:wide={pane === 'sheets'}
    class:measured
    bind:this={panel}
    role="group"
    aria-label="Export"
    style:height={paneHeight ? `${paneHeight}px` : undefined}
  >
    <div class="pane" bind:clientHeight={paneHeight}>
      {#if pane === 'root'}
        <div class="rows" data-pane="root">
          <button type="button" class="row" onclick={exportCsv} disabled={!current || exporting}>
            <Icon name="download" size={14} />
            <span class="text">
              <span class="title">{exporting ? 'Preparing CSV…' : 'CSV'}</span>
              <span class="sub">{current?.name ?? 'No result available'}</span>
            </span>
          </button>
          <button type="button" class="row" onclick={openSheets} disabled={!current || exporting} aria-expanded="false">
            <Icon name="grid" size={14} />
            <span class="text">
              <span class="title">Excel workbook</span>
              <span class="sub">Pick the Views to include</span>
            </span>
            <span class="chevron" aria-hidden="true"><Icon name="chevron" size={12} /></span>
          </button>
        </div>
      {:else}
        <div class="rows" data-pane="sheets">
          <button type="button" class="back" onclick={backToRoot} disabled={exporting}>
            <span class="chevron back-chevron" aria-hidden="true"><Icon name="chevron" size={12} /></span>
            Excel workbook
          </button>

          <div class="sheets" aria-busy={loading}>
            {#if current}
              <p class="group">This View</p>
              <Checkbox
                checked={selectedKeys.includes(current.key)} label={current.name} disabled={exporting}
                onchange={(checked) => onToggle(current.key, checked)}
              />
            {/if}
            {#if loading}
              <p class="state">Loading project Views…</p>
            {:else}
              {#each sources as source (source)}
                <p class="group">{source}</p>
                {#each options.filter((option) => option.source === source) as option (option.key)}
                  <Checkbox
                    checked={selectedKeys.includes(option.key)} label={option.name} disabled={exporting}
                    onchange={(checked) => onToggle(option.key, checked)}
                  />
                {/each}
              {/each}
              {#if options.length === 0}<p class="state">No other Views in this project.</p>{/if}
            {/if}
          </div>

          <div class="foot">
            <span class="count">{sheetCount} {sheetCount === 1 ? 'sheet' : 'sheets'}</span>
            <Button type="button" variant="primary" onclick={onExport} disabled={!canExport}>
              {exporting ? 'Exporting…' : 'Export'}
            </Button>
          </div>
        </div>
      {/if}

      {#if error}<p class="error" role="alert">{error}</p>{/if}
    </div>
  </div>
{/if}

<style>
  .panel {
    position: absolute;
    top: calc(100% + 6px);
    right: 0;
    z-index: 12;
    width: 244px;
    overflow: hidden;
    border: 1px solid var(--line-strong);
    border-radius: var(--radius-xl);
    background: var(--surface);
    box-shadow: var(--shadow-popover);
    transform-origin: top right;
    animation: panel-in 200ms cubic-bezier(0.32, 0.72, 0, 1);
  }
  /* Width and height only animate once the first pane has been measured,
     so the panel opens at its true size and morphs on the pane change. */
  .panel.measured { transition: height 260ms cubic-bezier(0.32, 0.72, 0, 1), width 260ms cubic-bezier(0.32, 0.72, 0, 1); }
  .panel.wide { width: 288px; }

  @keyframes panel-in {
    from { opacity: 0; transform: translateY(-6px) scale(0.96); }
    to { opacity: 1; transform: none; }
  }

  .pane { padding: 5px; }
  .rows { display: flex; flex-direction: column; gap: 2px; animation: pane-in 220ms cubic-bezier(0.32, 0.72, 0, 1); }
  .rows[data-pane='root'] { animation-name: pane-in-back; }

  @keyframes pane-in { from { opacity: 0; transform: translateX(10px); } to { opacity: 1; transform: none; } }
  @keyframes pane-in-back { from { opacity: 0; transform: translateX(-8px); } to { opacity: 1; transform: none; } }

  .row {
    display: flex;
    align-items: center;
    gap: 9px;
    width: 100%;
    padding: 7px 8px;
    border: 0;
    border-radius: var(--radius-lg);
    background: transparent;
    color: var(--glyph);
    text-align: left;
    transition: background 120ms ease, color 120ms ease;
  }
  .row:hover:not(:disabled) { background: var(--surface-hover); color: var(--action); }
  .row:disabled { opacity: 0.5; }
  .text { display: flex; flex-direction: column; gap: 1px; min-width: 0; }
  .title { font-size: 12.5px; font-weight: 500; color: var(--ink); }
  .sub { font: 10.5px var(--font-mono); color: var(--faint); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .chevron { margin-left: auto; display: flex; color: var(--glyph); transition: transform 160ms cubic-bezier(0.32, 0.72, 0, 1); }
  .row:hover:not(:disabled) .chevron { transform: translateX(2px); }
  .back-chevron { margin-left: 0; transform: rotate(180deg); }
  .back:hover:not(:disabled) .back-chevron { transform: rotate(180deg) translateX(2px); }

  .back {
    display: flex;
    align-items: center;
    gap: 7px;
    width: 100%;
    padding: 6px 8px;
    border: 0;
    border-radius: var(--radius-lg);
    background: transparent;
    color: var(--muted);
    font-size: 11.5px;
    font-weight: 500;
    text-align: left;
  }
  .back:hover:not(:disabled) { background: var(--surface-hover); color: var(--ink); }

  .sheets {
    display: flex;
    flex-direction: column;
    gap: 6px;
    max-height: 264px;
    overflow-y: auto;
    padding: 6px 8px 8px;
    margin-top: 2px;
    border-top: 1px solid var(--line-soft);
  }
  .group { margin: 4px 0 0; font: 10px var(--font-mono); letter-spacing: 0.06em; text-transform: uppercase; color: var(--faint); }
  .group:first-child { margin-top: 0; }
  .state { margin: 2px 0; font-size: 11.5px; color: var(--muted); }

  .foot { display: flex; align-items: center; justify-content: space-between; gap: 8px; padding: 8px 8px 4px; }
  .count { font: 10.5px var(--font-mono); color: var(--faint); }

  .error { margin: 6px 4px 2px; font-size: 11.5px; color: var(--error); }
</style>
