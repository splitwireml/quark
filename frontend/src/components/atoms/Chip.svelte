<script lang="ts">
  import type { Snippet } from 'svelte';

  type Props = {
    tone?: 'default' | 'accent' | 'muted' | 'dark' | 'dashed' | 'filter' | 'sort';
    onRemove?: () => void;
    removeLabel?: string;
    children: Snippet;
    [key: string]: unknown;
  };

  let { tone = 'default', onRemove, removeLabel = 'Remove', children, ...rest }: Props = $props();
</script>

<span class="chip {tone}" {...rest}>
  {@render children()}
  {#if onRemove}
    <button type="button" class="remove" aria-label={removeLabel} onclick={onRemove}>×</button>
  {/if}
</span>

<style>
  .chip {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    height: 24px;
    padding: 0 8px;
    border-radius: var(--radius-sm);
    border: 1px solid var(--control-border);
    background: var(--surface);
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--ink-2);
    white-space: nowrap;
  }
  .chip.accent { border-color: var(--action-tint-border); background: var(--action-tint); color: var(--action-dark); }
  .chip.filter { border-color: var(--action-tint-border); background: var(--action-tint); color: var(--action-dark); }
  .chip.sort { border-color: var(--warning); background: var(--surface); color: var(--warning); }
  .chip.muted { border-color: var(--control-border); background: var(--surface-2); color: var(--ink-2); }
  .chip.dark { border-color: var(--ink-fill); background: var(--ink-fill); color: #FFFFFF; }
  .chip.dashed { border-style: dashed; border-color: var(--faint); color: var(--faint); background: transparent; }

  .remove {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 18px;
    height: 18px;
    border: 0;
    border-radius: 2px;
    background: transparent;
    color: var(--faint);
    font-size: 12px;
    line-height: 1;
    padding: 0;
    opacity: 0.55;
  }
  .remove:hover { color: currentColor; background: color-mix(in srgb, currentColor 8%, transparent); opacity: 1; }
  .remove:focus-visible { color: currentColor; opacity: 1; outline: 2px solid var(--action); outline-offset: 1px; }
</style>
