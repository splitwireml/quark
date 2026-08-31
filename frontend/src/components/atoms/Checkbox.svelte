<script lang="ts">
  type Props = {
    checked: boolean;
    label?: string;
    onchange: (checked: boolean) => void;
    [key: string]: unknown;
  };

  let { checked, label, onchange, ...rest }: Props = $props();
</script>

<label class="checkbox" {...rest}>
  <input
    type="checkbox"
    {checked}
    onchange={(event) => onchange((event.currentTarget as HTMLInputElement).checked)}
  />
  <span class="box" aria-hidden="true">{#if checked}✓{/if}</span>
  {#if label}<span class="label">{label}</span>{/if}
</label>

<style>
  .checkbox { display: inline-flex; align-items: center; gap: 8px; cursor: pointer; }
  input { position: absolute; width: 1px; height: 1px; opacity: 0; }
  .box {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 12px; height: 12px;
    flex: none;
    border-radius: 2px;
    border: 1px solid var(--placeholder);
    background: var(--surface);
    color: #FFFFFF;
    font-size: 8px;
  }
  input:checked + .box { border-color: var(--action); background: var(--action); }
  input:focus-visible + .box { outline: 2px solid var(--action); outline-offset: 1px; }
  .label { font-size: 11.5px; color: var(--ink); }
</style>
