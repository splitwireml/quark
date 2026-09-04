<script lang="ts">
  import StatusDot from '../atoms/StatusDot.svelte';
  import type { SourceSummary } from '../../lib/types';

  type Props = { node: SourceSummary; active: boolean; loading: boolean; loaded: boolean; size?: 'compact' | 'comfortable'; onselect: () => void };
  let { node, active, loading, loaded, size = 'compact', onselect }: Props = $props();
</script>

<button type="button" class="row {size}" class:active aria-current={active ? 'page' : undefined} aria-expanded={loaded} disabled={loading} onclick={onselect}>
  <StatusDot tone={loading ? 'warning' : loaded ? 'success' : 'muted'} />
  <span class="name" title={node.name}>{node.name}</span>
  <small>{loading ? 'loading' : loaded ? 'loaded' : ''}</small>
  {#if size === 'comfortable'}<span class="chevron" aria-hidden="true">›</span>{/if}
</button>

<style>
  .row {
    display: flex;
    align-items: center;
    gap: 8px;
    height: 28px;
    padding: 0 8px;
    border-radius: var(--radius-md);
    background: transparent;
    border: none;
    text-align: left;
    width: 100%;
  }
  .row:hover { background: var(--surface-hover); }
  .row.active { background: var(--surface-hover); }
  .row:disabled { cursor: wait; }
  .name {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--ink);
  }
  small { font-family: var(--font-mono); font-size: 9px; color: var(--faint); flex: none; }

  .row.comfortable {
    height: 40px;
    gap: 10px;
    padding: 0 12px;
    border: 1px solid var(--line);
    background: var(--surface);
    transition: border-color 120ms ease, background 120ms ease;
  }
  .row.comfortable:hover:not(:disabled) { border-color: var(--control-border); background: var(--surface-hover); }
  .row.comfortable .name { font-size: 13px; }
  .row.comfortable small { font-size: 10px; }
  .chevron { flex: none; font-size: 15px; line-height: 1; color: var(--glyph); }
  .row.comfortable:hover:not(:disabled) .chevron { color: var(--ink-2); }
</style>
