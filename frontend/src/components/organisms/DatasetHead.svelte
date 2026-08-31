<script lang="ts">
  type Props = {
    title: string;
    showMeta: boolean;
    rows: string;
    ms: string;
    showRefresh: boolean;
    onRefresh: () => void;
    loadingData: boolean;
    inert?: boolean;
  };
  let { title, showMeta, rows, ms, showRefresh, onRefresh, loadingData, inert = false }: Props = $props();
</script>

<header class="head" {inert}>
  <div>
    <h1>{title}</h1>
    {#if showMeta}<p class="meta"><span>{rows} rows</span><span>{ms} ms</span></p>{/if}
  </div>
  {#if showRefresh}
    <button class="refresh" onclick={onRefresh} disabled={loadingData} aria-label="Refresh data" title="Refresh data">↻</button>
  {/if}
</header>

<style>
  .head { flex: none; display: flex; align-items: center; justify-content: space-between; gap: 12px; padding: 16px 20px 4px; }
  h1 { margin: 0; font-size: 16px; font-weight: 600; letter-spacing: -0.01em; color: var(--ink); }
  .meta { margin: 4px 0 0; display: flex; gap: 10px; font-family: var(--font-mono); font-size: 11px; color: var(--muted); }
  .refresh { width: 26px; height: 26px; border-radius: var(--radius-md); border: 1px solid var(--control-border); background: var(--surface); color: var(--muted); }
  .refresh:hover:not(:disabled) { border-color: var(--faint); color: var(--ink); }
  .refresh:disabled { opacity: 0.5; }
</style>
