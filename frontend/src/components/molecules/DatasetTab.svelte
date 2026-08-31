<script lang="ts">
  import type { Snippet } from 'svelte';
  type Props = { active: boolean; disabled?: boolean; onselect: () => void; children: Snippet };
  let { active, disabled = false, onselect, children }: Props = $props();
</script>

<button class="tab" class:active aria-current={active ? 'page' : undefined} {disabled} onclick={onselect}>
  {@render children()}
</button>

<style>
  .tab {
    height: 28px;
    padding: 0 12px;
    border-radius: var(--radius-lg);
    border: 1px solid transparent;
    background: transparent;
    font-size: 12.5px;
    color: var(--muted);
    white-space: nowrap;
  }
  .tab:hover:not(:disabled) { color: var(--ink); }
  .tab.active { background: var(--surface); border-color: var(--line); color: var(--ink); font-weight: 500; }
  .tab:disabled { opacity: 0.5; }
</style>
