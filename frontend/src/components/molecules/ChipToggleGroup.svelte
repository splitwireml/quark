<script lang="ts">
  type Option = { value: string; label: string; tip?: string };
  type Props = {
    options: readonly Option[];
    selected: string | null;
    label: string;
    onSelect: (value: string) => void;
  };
  let { options, selected, label, onSelect }: Props = $props();
</script>

<div class="chips" role="group" aria-label={label}>
  {#each options as option (option.value)}
    <button
      type="button"
      class="chip-toggle"
      class:on={selected === option.value}
      aria-pressed={selected === option.value}
      data-tip={option.tip}
      data-tip-position="top"
      onclick={() => onSelect(option.value)}
    >{option.label}</button>
  {/each}
</div>

<style>
  .chips { display: flex; flex-wrap: wrap; gap: 5px; }
  .chip-toggle {
    height: 26px;
    padding: 0 9px;
    border-radius: var(--radius-sm);
    border: 1px solid var(--control-border);
    background: var(--surface);
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--muted);
  }
  .chip-toggle.on { border-color: var(--ink-fill); background: var(--ink-fill); color: var(--on-fill); }
  .chip-toggle:hover:not(.on) { border-color: var(--faint); color: var(--ink); }
</style>
