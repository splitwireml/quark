<script lang="ts">
  import type { Snippet } from 'svelte';

  type Props = {
    value: string;
    glyph?: string;
    size?: 'sm' | 'md';
    mono?: boolean;
    trailing?: Snippet;
    [key: string]: unknown;
  };

  let { value = $bindable(), glyph, size = 'sm', mono = false, trailing, ...rest }: Props = $props();
</script>

<span class="field {size}" class:mono>
  {#if glyph}<span class="glyph" aria-hidden="true">{glyph}</span>{/if}
  <input bind:value {...rest} />
  {#if trailing}<span class="trailing">{@render trailing()}</span>{/if}
</span>

<style>
  .field {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    height: 26px;
    padding: 0 8px;
    border-radius: var(--radius-md);
    border: 1px solid var(--control-border);
    background: var(--surface);
  }
  .field.md { height: 30px; padding: 0 10px; border-radius: var(--radius-lg); }
  .field:focus-within { border-color: var(--ink); }
  .glyph { color: var(--glyph); font-size: 11px; flex: none; }
  .md .glyph { font-size: 12.5px; }
  .trailing { display: inline-flex; align-items: center; gap: 2px; flex: none; margin-right: -4px; }
  input {
    flex: 1;
    min-width: 0;
    border: none;
    background: transparent;
    font-size: 11.5px;
    color: var(--ink);
  }
  .md input { font-size: 12.5px; }
  .mono input { font-family: var(--font-mono); }
  input::placeholder { color: var(--faint); }
</style>
