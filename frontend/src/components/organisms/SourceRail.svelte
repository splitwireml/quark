<script lang="ts">
  import type { Snippet } from 'svelte';
  import Button from '../atoms/Button.svelte';
  import StatusDot from '../atoms/StatusDot.svelte';
  import Eyebrow from '../atoms/Eyebrow.svelte';
  import SourceTreeItem from '../molecules/SourceTreeItem.svelte';
  import ViewTreeItem from '../molecules/ViewTreeItem.svelte';
  import { versionLabel } from '../../lib/versioning';
  import type { SourceSummary, ViewHistory } from '../../lib/types';

  type Props = {
    nodes: SourceSummary[];
    views: ViewHistory[];
    selectedViewId: string;
    selectedSourceId: string;
    loadedSourceIds: string[];
    loadingSourceId: string;
    loadingNodes: boolean;
    railOpen: boolean;
    collapsed: boolean;
    sourceOpen: boolean;
    highlightToken?: number;
    inert?: boolean;
    onSelectSource: (id: string) => void;
    onSelectView: (id: string) => void;
    onToggleSource: () => void;
    onCloseRail: () => void;
    disclosure: Snippet;
  };

  let { nodes, views, selectedViewId, selectedSourceId, loadedSourceIds, loadingSourceId, loadingNodes, railOpen, collapsed, sourceOpen, highlightToken = 0, inert = false, onSelectSource, onSelectView, onToggleSource, onCloseRail, disclosure }: Props = $props();
  let highlighting = $state(false);
  // a CSS animation only replays once the class is removed and re-added, so drop it for one frame first
  $effect(() => {
    if (!highlightToken) return;
    highlighting = false;
    let frame = requestAnimationFrame(() => { frame = requestAnimationFrame(() => highlighting = true); });
    const timer = setTimeout(() => highlighting = false, 1800);
    return () => { cancelAnimationFrame(frame); clearTimeout(timer); };
  });
  let derivedViews = $derived(views.filter((view) => view.kind === 'derived'));
  const activeVersionLabel = (view: ViewHistory) => versionLabel(view.versions.find((item) => item.id === view.activeVersionId) ?? view.versions[view.versions.length - 1]);
</script>

<aside class:open={railOpen} class:collapsed class="rail" aria-label="Sources and Views" {inert}>
  <div class="rail-head">
    <Button variant="primary" aria-expanded={sourceOpen} onclick={onToggleSource}>+ Add source</Button>
    {#if sourceOpen}<div class="disclosure">{@render disclosure()}</div>{/if}
  </div>
  <nav class="nodes" aria-label="Project Views">
    <div class="section-title"><Eyebrow>Sources</Eyebrow><span>{nodes.length}</span></div>
    {#if loadingNodes}
      <p class="state">Loading sources…</p>
    {:else if nodes.length === 0}
      <p class="state">No sources yet.</p>
    {:else}
      {#each nodes as node, index (node.id)}
        <section class="source-group" class:highlight={highlighting} style="--stagger: {index * 70}ms">
          <SourceTreeItem
            {node}
            active={node.id === selectedSourceId}
            loading={loadingSourceId === node.id || loadingSourceId === '*'}
            loaded={loadedSourceIds.includes(node.id)}
            onselect={() => onSelectSource(node.id)}
          />
          {#if loadedSourceIds.includes(node.id)}
            {#each views.filter((view) => view.kind === 'source' && view.sourceId === node.id) as view (view.id)}
              <ViewTreeItem name={view.name} versionLabel={activeVersionLabel(view)} active={view.id === selectedViewId} onselect={() => onSelectView(view.id)} />
            {:else}
              <p class="empty">No Views</p>
            {/each}
          {/if}
        </section>
      {/each}
    {/if}
    <div class="section-title derived-title"><Eyebrow>Derived Views</Eyebrow><span>{derivedViews.length}</span></div>
    {#each derivedViews as view (view.id)}
      <ViewTreeItem name={view.name} versionLabel={activeVersionLabel(view)} active={view.id === selectedViewId} onselect={() => onSelectView(view.id)} />
    {:else}
      <p class="state">No derived Views yet.</p>
    {/each}
  </nav>
  <footer><StatusDot tone="success" />Backend connected</footer>
</aside>
{#if railOpen}<button class="backdrop" aria-label="Close sources" onclick={onCloseRail}></button>{/if}

<style>
  .rail { width: var(--rail); flex: none; background: var(--surface-3); border-right: 1px solid var(--line); display: flex; flex-direction: column; overflow: hidden; transition: width 160ms ease, border-color 160ms ease; }
  .rail.collapsed { width: 0; border-color: transparent; }
  .rail-head { padding: 12px; border-bottom: 1px solid var(--line); }
  .disclosure { margin-top: 10px; }
  .nodes { flex: 1; min-height: 0; overflow-y: auto; padding: 10px 8px; display: flex; flex-direction: column; gap: 2px; }
  .section-title { display: flex; align-items: center; justify-content: space-between; padding: 0 6px 6px; }
  .section-title span { font-family: var(--font-mono); font-size: 10px; color: var(--faint); }
  .derived-title { margin-top: 14px; padding-top: 10px; border-top: 1px solid var(--line); }
  .source-group { min-width: 0; margin-bottom: 8px; padding-left: 8px; border-left: 1px solid var(--line); border-radius: 0 var(--radius-md) var(--radius-md) 0; }
  .source-group.highlight { animation: source-sweep 900ms ease-out var(--stagger, 0ms) both; }
  @keyframes source-sweep {
    0% { background: transparent; border-left-color: var(--line); }
    22% { background: var(--action-tint); border-left-color: var(--action); }
    100% { background: transparent; border-left-color: var(--line); }
  }
  .state, .empty { margin: 0; padding: 8px 6px; font-size: 12px; color: var(--muted); }
  .empty { padding: 4px 8px 6px; font-size: 10.5px; color: var(--faint); }
  footer { display: flex; align-items: center; gap: 7px; padding: 10px 14px; border-top: 1px solid var(--line); font-size: 11px; color: var(--muted); }
  .backdrop { display: none; }
  @media (prefers-reduced-motion: reduce) {
    .source-group.highlight { animation: none; }
  }
  @media (max-width: 720px) {
    .rail { position: fixed; inset: 44px 0 0 0; z-index: 20; width: min(320px, 88vw); transform: translateX(-100%); transition: transform 180ms ease; }
    .rail.open { transform: translateX(0); box-shadow: var(--shadow-panel); }
    .backdrop { display: block; position: fixed; inset: 44px 0 0 0; z-index: 19; background: rgba(15, 22, 32, 0.3); border: none; }
  }
</style>
