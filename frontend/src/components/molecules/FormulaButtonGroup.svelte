<script lang="ts">
  import type { Snippet } from 'svelte';

  type Props = { label: string; children: Snippet };
  let { label, children }: Props = $props();
</script>

<fieldset class="formula-group">
  <legend>{label}</legend>
  <div>{@render children()}</div>
</fieldset>

<style>
  .formula-group { min-width: 0; margin: 0; padding: 8px; border: 1px solid var(--line); border-radius: var(--radius-md); }
  legend { padding: 0 4px; font-size: 10px; font-weight: 600; color: var(--muted); }
  div { display: flex; flex-wrap: wrap; gap: 6px; }
</style>
