<script lang="ts">
  import type { Snippet } from 'svelte';

  type Props = {
    variant?: 'primary' | 'secondary' | 'ghost';
    active?: boolean;
    children: Snippet;
    [key: string]: unknown;
  };

  let { variant = 'secondary', active = false, children, ...rest }: Props = $props();
</script>

<button class="btn {variant}" class:active {...rest}>
  {@render children()}
</button>

<style>
  .btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
    height: 30px;
    padding: 0 12px;
    border-radius: var(--radius-lg);
    border: 1px solid var(--control-border);
    background: var(--surface);
    font-size: 12.5px;
    font-weight: 500;
    color: var(--ink-2);
    white-space: nowrap;
    transition: background 120ms ease, border-color 120ms ease, color 120ms ease;
  }
  .btn:hover:not(:disabled) { border-color: var(--faint); }
  .btn:disabled { opacity: 0.5; }

  .btn.primary {
    border-color: var(--ink-fill);
    background: var(--ink-fill);
    color: #FFFFFF;
  }
  .btn.primary:hover:not(:disabled) { background: #2B3444; border-color: #2B3444; }

  .btn.ghost {
    border-color: transparent;
    background: transparent;
    color: var(--muted);
  }
  .btn.ghost:hover:not(:disabled) { background: var(--surface-hover); color: var(--ink); }

  .btn.active {
    border-color: var(--action);
    background: var(--action-tint);
    color: var(--action-dark);
  }
</style>
