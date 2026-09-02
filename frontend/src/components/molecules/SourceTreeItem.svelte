<script lang="ts">
  import StatusDot from '../atoms/StatusDot.svelte';
  import type { SourceSummary } from '../../lib/types';

  type Props = { node: SourceSummary; active: boolean; loading: boolean; loaded: boolean; onselect: () => void };
  let { node, active, loading, loaded, onselect }: Props = $props();
</script>

<button type="button" class="row" class:active aria-current={active ? 'page' : undefined} aria-expanded={loaded} disabled={loading} onclick={onselect}>
  <StatusDot tone={loading ? 'warning' : loaded ? 'success' : 'muted'} />
  <span class="name" title={node.name}>{node.name}</span>
  <small>{loading ? 'loading' : loaded ? 'loaded' : ''}</small>
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
</style>
