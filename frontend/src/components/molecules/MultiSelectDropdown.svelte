<script lang="ts">
  import { dismissable } from '../../lib/dismiss';

  type Option = { value: string; label: string };
  type Props = {
    label: string;
    options: Option[];
    selected: string[];
    onchange: (selected: string[]) => void;
    placeholder?: string;
    disabled?: boolean;
  };

  let { label, options, selected, onchange, placeholder = 'Choose keys', disabled = false }: Props = $props();
  let open = $state(false);
  let search = $state('');
  let selectedLabels = $derived(selected.map((value) => options.find((option) => option.value === value)?.label ?? value));
  let summary = $derived(selectedLabels.length === 0 ? placeholder : selectedLabels.length === 1 ? selectedLabels[0] : `${selectedLabels[0]} +${selectedLabels.length - 1}`);
  let matches = $derived.by(() => {
    const query = search.trim().toLowerCase();
    return query ? options.filter((option) => option.label.toLowerCase().includes(query)) : options;
  });

  function toggle(value: string) {
    onchange(selected.includes(value) ? selected.filter((item) => item !== value) : [...selected, value]);
  }
</script>

<div class="field" use:dismissable={() => open = false}>
  <span class="label">{label}</span>
  <button
    type="button" class="trigger" class:open
    aria-label={`${label}: ${summary}`} aria-haspopup="listbox" aria-expanded={open}
    title={selectedLabels.join(' + ') || undefined} {disabled}
    onclick={() => open = !open}
  >
    <span class="summary" class:placeholder={selected.length === 0}>{summary}</span>
    <span class="count">{selected.length || ''}</span>
    <span class="chevron" aria-hidden="true">⌄</span>
  </button>

  {#if open}
    <div class="panel">
      <label class="search">
        <span class="sr-only">Search {label}</span>
        <span aria-hidden="true">⌕</span>
        <input type="search" bind:value={search} placeholder="Search columns" />
      </label>
      <div class="options" role="listbox" aria-label={label} aria-multiselectable="true">
        {#each matches as option (option.value)}
          {@const on = selected.includes(option.value)}
          <button type="button" class="option" class:on role="option" aria-selected={on} onclick={() => toggle(option.value)}>
            <span class="box" aria-hidden="true">{on ? '✓' : ''}</span>
            <span class="option-label" title={option.label}>{option.label}</span>
            {#if on}<small>{selected.indexOf(option.value) + 1}</small>{/if}
          </button>
        {:else}
          <p class="empty">No matching columns</p>
        {/each}
      </div>
      <div class="actions">
        <span>{selected.length} selected</span>
        <button type="button" onclick={() => onchange([])} disabled={selected.length === 0}>Clear</button>
        <button type="button" class="done" onclick={() => open = false}>Done</button>
      </div>
    </div>
  {/if}
</div>

<style>
  .field { position: relative; display: flex; min-width: 0; flex-direction: column; gap: 5px; }
  .label { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: 11px; color: var(--muted); }
  .trigger {
    display: flex;
    align-items: center;
    gap: 7px;
    width: 100%;
    height: 32px;
    min-width: 0;
    padding: 0 9px;
    border: 1px solid var(--control-border);
    border-radius: var(--radius-lg);
    background: var(--surface);
    color: var(--ink-2);
    text-align: left;
  }
  .trigger:hover:not(:disabled) { border-color: var(--faint); }
  .trigger.open { border-color: var(--action); background: var(--action-tint); color: var(--action-dark); }
  .summary { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: 12px; font-weight: 500; }
  .summary.placeholder { color: var(--faint); font-weight: 400; }
  .count { min-width: 12px; text-align: right; font: 10px var(--font-mono); color: var(--faint); }
  .chevron { flex: none; color: var(--glyph); }

  .panel {
    position: absolute;
    top: calc(100% + 5px);
    left: 0;
    z-index: 20;
    width: max(100%, 220px);
    overflow: hidden;
    border: 1px solid var(--line-strong);
    border-radius: var(--radius-xl);
    background: var(--surface);
    box-shadow: var(--shadow-popover);
    animation: list-in 160ms cubic-bezier(0.32, 0.72, 0, 1);
  }
  .search { display: flex; align-items: center; gap: 6px; height: 32px; padding: 0 9px; border-bottom: 1px solid var(--line); color: var(--glyph); }
  .search input { flex: 1; min-width: 0; border: 0; background: transparent; font-size: 11.5px; color: var(--ink); outline: none; }
  .search input::placeholder { color: var(--faint); }
  .options { max-height: 184px; overflow-y: auto; padding: 4px; }
  .option { display: flex; align-items: center; gap: 8px; width: 100%; min-height: 30px; padding: 4px 7px; border: 0; border-radius: var(--radius-md); background: transparent; text-align: left; }
  .option:hover, .option:focus-visible { background: var(--surface-hover); }
  .option.on { background: var(--action-tint); color: var(--action-dark); }
  .box { display: inline-flex; align-items: center; justify-content: center; flex: none; width: 12px; height: 12px; border: 1px solid var(--glyph); border-radius: 2px; color: #FFFFFF; font-size: 8px; }
  .option.on .box { border-color: var(--action); background: var(--action); }
  .option-label { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font: 11px var(--font-mono); }
  .option small { flex: none; min-width: 16px; text-align: center; font: 9px var(--font-mono); color: var(--action-dark); }
  .empty { margin: 0; padding: 9px; font-size: 11px; color: var(--faint); }
  .actions { display: flex; align-items: center; gap: 4px; min-height: 34px; padding: 3px 5px 3px 9px; border-top: 1px solid var(--line); background: var(--surface-2); }
  .actions span { margin-right: auto; font: 10px var(--font-mono); color: var(--faint); }
  .actions button { height: 26px; padding: 0 7px; border: 0; border-radius: var(--radius-md); background: transparent; font-size: 11px; color: var(--muted); }
  .actions button:hover:not(:disabled) { background: var(--surface-hover); color: var(--ink); }
  .actions button:disabled { color: var(--disabled); }
  .actions .done { color: var(--action-dark); }

  @keyframes list-in { from { opacity: 0; transform: translateY(-4px); } to { opacity: 1; transform: none; } }
</style>
