<script lang="ts">
  import StatusDot from '../atoms/StatusDot.svelte';
  import type { NodeInfo } from '../../lib/types';

  type Props = { node: NodeInfo; active: boolean; onselect: () => void };
  let { node, active, onselect }: Props = $props();
</script>

<button class="row" class:active aria-current={active ? 'page' : undefined} onclick={onselect}>
  <StatusDot tone="success" />
  <span class="name" title={node.name}>{node.name}</span>
  <small>{node.kind}</small>
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
