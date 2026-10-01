<script lang="ts">
  import type { Snippet } from 'svelte';

  type Props = { label: string; children: Snippet; keypad?: boolean; available?: boolean; wide?: boolean };
  let { label, children, keypad = false, available = false, wide = false }: Props = $props();
</script>

<fieldset class="formula-group" class:keypad class:available class:wide>
  <legend>{label}</legend>
  <div>{@render children()}</div>
</fieldset>

<style>
  .formula-group { min-width: 0; margin: 0; padding: 8px; border: 1px solid var(--line); border-radius: var(--radius-md); }
  legend { padding: 0 4px; font-size: 10px; font-weight: 600; color: var(--muted); }
  div { display: flex; flex-wrap: wrap; gap: 6px; }
  .formula-group.keypad { border-radius: 4px; }
  .formula-group.keypad.available { border-color: var(--action-tint-border); background: var(--action-tint); }
  .formula-group.keypad.available legend { color: var(--action-dark); }
  .formula-group.keypad.wide { grid-column: span 2; }
  .formula-group.keypad div { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); }
  .formula-group.keypad.wide div { grid-template-columns: repeat(4, minmax(0, 1fr)); }
  .formula-group.keypad :global(.btn) { width: 100%; min-width: 0; height: 36px; padding: 0 5px; border-radius: 4px; font-size: 11.5px; }
  .formula-group.keypad :global(.btn.active:disabled) { opacity: 1; }
  @media (max-width: 420px) {
    .formula-group.keypad.wide { grid-column: auto; }
    .formula-group.keypad.wide div { grid-template-columns: repeat(2, minmax(0, 1fr)); }
  }
</style>
