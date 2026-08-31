<script lang="ts">
  type Props = {
    value: string;
    glyph?: string;
    [key: string]: unknown;
  };

  let { value = $bindable(), glyph, ...rest }: Props = $props();
</script>

<span class="field">
  {#if glyph}<span class="glyph" aria-hidden="true">{glyph}</span>{/if}
  <input bind:value {...rest} />
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
  .field:focus-within { border-color: var(--ink); }
  .glyph { color: var(--placeholder); font-size: 11px; flex: none; }
  input {
    flex: 1;
    min-width: 0;
    border: none;
    background: transparent;
    font-size: 11.5px;
    color: var(--ink);
  }
  input::placeholder { color: var(--placeholder); }
</style>
