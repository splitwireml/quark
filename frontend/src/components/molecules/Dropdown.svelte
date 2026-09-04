<script lang="ts">
  import { dismissable } from '../../lib/dismiss';

  type Option = { value: string; label: string; hint?: string };
  type Props = {
    label: string;
    value: string;
    options: Option[];
    onchange: (value: string) => void;
    placeholder?: string;
    disabled?: boolean;
  };

  let { label, value, options, onchange, placeholder = 'Select…', disabled = false }: Props = $props();

  let open = $state(false);
  let selected = $derived(options.find((option) => option.value === value));

  function choose(option: Option) {
    onchange(option.value);
    open = false;
  }
</script>

<div class="dropdown">
  <button
    type="button" class="trigger" class:open
    aria-label={`${label}: ${selected?.label ?? placeholder}`}
    aria-haspopup="listbox" aria-expanded={open} {disabled}
    onclick={() => open = !open}
  >
    <span class="value" class:placeholder={!selected}>{selected?.label ?? placeholder}</span>
    <span class="chevron" aria-hidden="true">⌄</span>
  </button>

  {#if open}
    <div class="panel" role="listbox" aria-label={label} use:dismissable={() => open = false}>
      {#each options as option (option.value)}
        <button
          type="button" class="option" class:on={option.value === value}
          role="option" aria-selected={option.value === value}
          onclick={() => choose(option)}
        >
          <span class="option-label">{option.label}</span>
          {#if option.hint}<small>{option.hint}</small>{/if}
        </button>
      {:else}
        <p class="empty">No options</p>
      {/each}
    </div>
  {/if}
</div>

<style>
  /* The list opens in place, under the trigger and inside the menu's own
     scroll, so it can never be clipped by the panel it lives in. */
  .dropdown { display: flex; flex-direction: column; min-width: 0; }
  .trigger {
    display: flex;
    align-items: center;
    gap: 6px;
    width: 100%;
    min-width: 0;
    height: 30px;
    padding: 0 9px;
    border-radius: var(--radius-lg);
    border: 1px solid var(--control-border);
    background: var(--surface);
    font-size: 12.5px;
    font-weight: 500;
    color: var(--ink-2);
    transition: border-color 120ms ease, background 120ms ease, color 120ms ease;
  }
  .trigger:hover:not(:disabled) { border-color: var(--faint); }
  .trigger:disabled { opacity: 0.5; }
  .trigger.open { border-color: var(--action); background: var(--action-tint); color: var(--action-dark); }
  .value { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; text-align: left; }
  .placeholder { color: var(--faint); }
  .chevron { flex: none; color: var(--faint); }
  .trigger.open .chevron { color: var(--action); }

  .panel {
    width: 100%;
    margin-top: 5px;
    max-height: 176px;
    overflow-y: auto;
    padding: 4px;
    border: 1px solid var(--line);
    border-radius: var(--radius-lg);
    background: var(--surface-inset);
    transform-origin: top center;
    animation: dropdown-in 160ms cubic-bezier(0.32, 0.72, 0, 1);
  }
  .option {
    display: flex;
    align-items: baseline;
    gap: 6px;
    width: 100%;
    min-height: 28px;
    padding: 4px 8px;
    border: 0;
    border-radius: var(--radius-md);
    background: transparent;
    font-size: 12px;
    color: var(--ink);
    text-align: left;
  }
  .option:hover, .option:focus-visible { background: var(--surface-hover); }
  .option.on { background: var(--action-tint); color: var(--action-dark); }
  .option-label { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .option small { flex: none; font: 10px var(--font-mono); color: var(--faint); }
  .empty { margin: 0; padding: 8px; font-size: 11px; color: var(--faint); }

  @keyframes dropdown-in {
    from { opacity: 0; transform: translateY(-4px) scale(0.98); }
    to { opacity: 1; transform: none; }
  }
  @media (prefers-reduced-motion: reduce) {
    .panel { animation: none; }
  }
</style>
