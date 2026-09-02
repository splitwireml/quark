<script lang="ts">
  import Button from '../atoms/Button.svelte';
  import StatusDot from '../atoms/StatusDot.svelte';
  import type { ProjectInfo, ViewHistory } from '../../lib/types';

  type Props = {
    project: ProjectInfo;
    currentView: ViewHistory | undefined;
    railCollapsed: boolean;
    inert?: boolean;
    onProjects: () => void;
    onToggleRailCollapsed: () => void;
    onOpenRail: () => void;
  };
  let { project, currentView, railCollapsed, inert = false, onProjects, onToggleRailCollapsed, onOpenRail }: Props = $props();
</script>

<header class="topbar" {inert}>
  <button class="glyph-btn menu-button" aria-label="Open sources and Views" onclick={onOpenRail}>☰</button>
  <button class="glyph-btn rail-toggle" aria-label={railCollapsed ? 'Expand sources sidebar' : 'Collapse sources sidebar'} aria-expanded={!railCollapsed} onclick={onToggleRailCollapsed}>☰</button>
  <div class="brand"><span aria-hidden="true">Q</span><strong>Quark</strong></div>
  <nav class="breadcrumbs" aria-label="Current location">
    <Button variant="ghost" onclick={onProjects}>← Projects</Button>
    <span>/</span><b>{project.name}</b>
    {#if currentView}<span>/</span><b>{currentView.name}</b>{/if}
  </nav>
  <div class="connection"><StatusDot tone="success" />On this machine</div>
</header>

<style>
  .topbar { height: 44px; flex: none; display: flex; align-items: center; gap: 14px; padding: 0 16px; background: var(--surface-2); border-bottom: 1px solid var(--line); }
  .glyph-btn { display: none; width: 28px; height: 28px; align-items: center; justify-content: center; border-radius: var(--radius-md); border: 1px solid transparent; background: transparent; color: var(--muted); }
  .rail-toggle { display: inline-flex; }
  .glyph-btn:hover { background: var(--surface-hover); }
  .brand { display: flex; align-items: center; gap: 8px; font-family: var(--font-mono); font-weight: 600; color: var(--ink); }
  .brand span { display: inline-flex; align-items: center; justify-content: center; width: 20px; height: 20px; border-radius: var(--radius-sm); background: var(--ink-fill); color: #fff; font-size: 11px; }
  .breadcrumbs { display: flex; align-items: center; gap: 6px; min-width: 0; overflow: hidden; font-size: 12.5px; color: var(--faint); }
  .breadcrumbs :global(.btn) { height: 26px; padding: 0 7px; }
  .breadcrumbs b { color: var(--ink); font-weight: 500; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .connection { display: flex; align-items: center; gap: 7px; margin-left: auto; flex: none; font-size: 11px; color: var(--muted); }
  @media (max-width: 720px) { .menu-button { display: inline-flex; } .rail-toggle { display: none; } .breadcrumbs b, .breadcrumbs > span { display: none; } }
</style>
