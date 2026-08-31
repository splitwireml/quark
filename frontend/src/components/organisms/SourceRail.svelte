<script lang="ts">
  import type { Snippet } from 'svelte';
  import Button from '../atoms/Button.svelte';
  import StatusDot from '../atoms/StatusDot.svelte';
  import Eyebrow from '../atoms/Eyebrow.svelte';
  import SourceTreeItem from '../molecules/SourceTreeItem.svelte';
  import type { NodeInfo } from '../../lib/types';

  type Props = {
    nodes: NodeInfo[];
    selectedNodeId: string;
    loadingNodes: boolean;
    railOpen: boolean;
    collapsed: boolean;
    sourceOpen: boolean;
    inert?: boolean;
    onSelectNode: (id: string) => void;
    onToggleSource: () => void;
    onCloseRail: () => void;
    disclosure: Snippet;
  };

  let { nodes, selectedNodeId, loadingNodes, railOpen, collapsed, sourceOpen, inert = false, onSelectNode, onToggleSource, onCloseRail, disclosure }: Props = $props();
</script>

<aside class:open={railOpen} class:collapsed class="rail" aria-label="Sources" {inert}>
  <div class="rail-head">
    <Button variant="primary" aria-expanded={sourceOpen} onclick={onToggleSource}>+ Add source</Button>
    {#if sourceOpen}
      <div class="disclosure">{@render disclosure()}</div>
    {/if}
  </div>
  <nav class="nodes" aria-label="Available sources">
    <div class="section-title"><Eyebrow>Sources</Eyebrow><span>{nodes.length}</span></div>
    {#if loadingNodes}
      <p class="state">Loading sources…</p>
    {:else if nodes.length === 0}
      <p class="state">No active sources yet.</p>
    {:else}
      {#each nodes as node (node.id)}
        <SourceTreeItem {node} active={node.id === selectedNodeId} onselect={() => onSelectNode(node.id)} />
      {/each}
    {/if}
  </nav>
  <footer><StatusDot tone="success" />Backend connected</footer>
</aside>
{#if railOpen}<button class="backdrop" aria-label="Close sources" onclick={onCloseRail}></button>{/if}

<style>
  .rail {
    width: var(--rail);
    flex: none;
    background: var(--surface-3);
    border-right: 1px solid var(--line);
    display: flex;
    flex-direction: column;
    overflow: hidden;
    transition: width 160ms ease, border-color 160ms ease;
  }
  .rail.collapsed { width: 0; border-color: transparent; }
  .rail-head { padding: 12px; border-bottom: 1px solid var(--line); }
  .disclosure { margin-top: 10px; }
  .nodes { flex: 1; min-height: 0; overflow-y: auto; padding: 10px 8px; display: flex; flex-direction: column; gap: 2px; }
  .section-title { display: flex; align-items: center; justify-content: space-between; padding: 0 6px 6px; }
  .section-title span { font-family: var(--font-mono); font-size: 10px; color: var(--faint); }
  .state { padding: 8px 6px; font-size: 12px; color: var(--muted); }
  footer { display: flex; align-items: center; gap: 7px; padding: 10px 14px; border-top: 1px solid var(--line); font-size: 11px; color: var(--muted); }
  .backdrop { display: none; }

  @media (max-width: 720px) {
    .rail {
      position: fixed;
      inset: 44px 0 0 0;
      z-index: 20;
      width: min(320px, 88vw);
      transform: translateX(-100%);
      transition: transform 180ms ease;
    }
    .rail.open { transform: translateX(0); box-shadow: var(--shadow-panel); }
    .backdrop {
      display: block;
      position: fixed;
      inset: 44px 0 0 0;
      z-index: 19;
      background: rgba(15, 22, 32, 0.3);
      border: none;
    }
  }
</style>
