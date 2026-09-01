<script lang="ts">
  import { tick } from 'svelte';
  import Button from '../atoms/Button.svelte';
  import Checkbox from '../atoms/Checkbox.svelte';

  type Option = { value: string; label: string; description?: string };
  type Props = {
    label: string;
    options: Option[];
    selected: string[];
    onchange: (selected: string[]) => void;
    placeholder?: string;
    disabled?: boolean;
  };

  let { label, options, selected, onchange, placeholder = 'Select…', disabled = false }: Props = $props();
  let open = $state(false);
  let activeIndex = $state(0);
  let root: HTMLDivElement;
  let triggerHost: HTMLDivElement;
  let listbox = $state<HTMLDivElement>();
  let selectedLabels = $derived(selected.map((value) => options.find((option) => option.value === value)?.label ?? value));
  let summary = $derived(selectedLabels.length === 0 ? placeholder : selectedLabels.join(' + '));

  function optionElements() {
    return listbox?.querySelectorAll<HTMLElement>('[role="option"]') ?? [];
  }

  function focusOption(index: number) {
    const elements = optionElements();
    if (!elements.length) return;
    activeIndex = (index + elements.length) % elements.length;
    elements[activeIndex].focus();
  }

  async function show() {
    if (disabled) return;
    open = true;
    const firstSelected = options.findIndex((option) => option.value === selected[0]);
    activeIndex = firstSelected < 0 ? 0 : firstSelected;
    await tick();
    focusOption(activeIndex);
  }

  async function close(restoreFocus = false) {
    open = false;
    if (restoreFocus) {
      await tick();
      triggerHost.querySelector('button')?.focus();
    }
  }

  function toggle(value: string, checked = !selected.includes(value)) {
    onchange(checked ? [...selected, value] : selected.filter((item) => item !== value));
  }

  function onTriggerKeydown(event: KeyboardEvent) {
    if (event.key === 'Escape' && open) {
      event.preventDefault();
      close(true);
    } else if (['Enter', ' ', 'ArrowDown', 'ArrowUp'].includes(event.key)) {
      event.preventDefault();
      show();
    }
  }

  function onOptionKeydown(event: KeyboardEvent, option: Option, index: number) {
    if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
      event.preventDefault();
      focusOption(index + (event.key === 'ArrowDown' ? 1 : -1));
    } else if (event.key === 'Enter' || event.key === ' ') {
      event.preventDefault();
      toggle(option.value);
    }
  }

  function onWindowClick(event: MouseEvent) {
    if (open && !root.contains(event.target as Node)) close();
  }

  function onWindowKeydown(event: KeyboardEvent) {
    if (open && event.key === 'Escape') {
      event.preventDefault();
      close(true);
    }
  }
</script>

<svelte:window onclick={onWindowClick} onkeydown={onWindowKeydown} />

<div class="field" bind:this={root}>
  <span class="label">{label}</span>
  <div class="trigger-host" bind:this={triggerHost}>
    <Button
      type="button"
      class="trigger"
      style="width: 100%; min-width: 0; min-height: 32px; position: relative; overflow: hidden;"
      aria-label={`${label}: ${summary}`}
      aria-haspopup="listbox"
      aria-expanded={open}
      title={selectedLabels.join(' + ') || undefined}
      {disabled}
      onclick={() => open ? close() : show()}
      onkeydown={onTriggerKeydown}
    >
      <span class="summary" class:placeholder={selected.length === 0}>{summary}</span>
      <span class="chevron" aria-hidden="true">⌄</span>
    </Button>
  </div>

  {#if open}
    <div class="popover">
      <div class="list" bind:this={listbox} role="listbox" aria-label={label} aria-multiselectable="true">
        {#each options as option, index (option.value)}
          <div
            class="option"
            class:selected={selected.includes(option.value)}
            role="option"
            aria-selected={selected.includes(option.value)}
            tabindex="-1"
            onclick={() => toggle(option.value)}
            onkeydown={(event) => onOptionKeydown(event, option, index)}
            onfocus={() => activeIndex = index}
          >
            <Checkbox
              checked={selected.includes(option.value)}
              label={option.label}
              onchange={(checked) => toggle(option.value, checked)}
              onclick={(event: MouseEvent) => event.stopPropagation()}
            />
            {#if option.description}<small>{option.description}</small>{/if}
          </div>
        {:else}
          <span class="empty">No options</span>
        {/each}
      </div>
      <div class="actions">
        <span>{selected.length} selected</span>
        <Button variant="ghost" type="button" disabled={selected.length === 0} onclick={() => onchange([])}>Clear</Button>
      </div>
    </div>
  {/if}
</div>

<style>
  .field { position: relative; display: flex; min-width: 0; flex-direction: column; gap: 5px; }
  .label { font-size: 11px; color: var(--muted); }
  .trigger-host { display: flex; }
  .summary { min-width: 0; max-width: calc(100% - 32px); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .placeholder { color: var(--placeholder); }
  .chevron { position: absolute; right: 10px; color: var(--faint); }
  .popover {
    position: absolute;
    top: calc(100% + 5px);
    left: 0;
    z-index: 20;
    width: max(100%, 240px);
    overflow: hidden;
    border: 1px solid var(--line-strong);
    border-radius: var(--radius-xl);
    background: var(--surface);
    box-shadow: var(--shadow-popover);
  }
  .list { max-height: 240px; overflow-y: auto; padding: 4px; }
  .option {
    min-height: 32px;
    padding: 5px 8px;
    border-radius: var(--radius-md);
    color: var(--ink);
    cursor: pointer;
  }
  .option:hover, .option:focus-visible { background: var(--surface-hover); }
  .option.selected { background: var(--action-tint); }
  .option:focus-visible { outline: 2px solid var(--action); outline-offset: -2px; }
  .option small { display: block; margin: 2px 0 0 20px; color: var(--muted-2); font-size: 10.5px; }
  .empty { display: block; padding: 8px; font-size: 11.5px; color: var(--muted); }
  .actions {
    display: flex;
    min-height: 34px;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    padding: 3px 6px 3px 10px;
    border-top: 1px solid var(--line-soft);
    background: var(--surface-2);
  }
  .actions > span { font: 10px var(--font-mono); color: var(--faint); }
</style>
