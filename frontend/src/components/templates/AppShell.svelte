<script lang="ts">
  import type { Snippet } from 'svelte';

  type Props = {
    titlebar: Snippet;
    rail: Snippet;
    main: Snippet;
    liveSummary: string;
  };
  let { titlebar, rail, main, liveSummary }: Props = $props();
</script>

<div class="app-shell">
  {@render titlebar()}
  <div class="shell">
    {@render rail()}
    {@render main()}
  </div>
  <p class="sr-only" aria-live="polite">{liveSummary}</p>
</div>

<style>
  .app-shell {
    height: 100vh;
    display: flex;
    flex-direction: column;
    background: var(--canvas);
  }
  .shell {
    flex: 1;
    min-height: 0;
    display: flex;
    padding: 20px;
    gap: 0;
  }

  @media (max-width: 720px) {
    .shell { padding: 0; }
    .shell > :global(main) { border: none; border-radius: 0; box-shadow: none; }
  }
  .shell > :global(main) {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    background: var(--surface);
    border: 1px solid var(--line-strong);
    border-radius: var(--radius-card);
    box-shadow: var(--shadow-popover);
    position: relative;
  }
</style>
