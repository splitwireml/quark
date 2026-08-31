<script lang="ts">
  import { tick } from 'svelte';
  import Button from '../atoms/Button.svelte';

  type Option = { value: string; label: string; description?: string };
  type Props = {
    label: string;
    options: Option[];
    value: string;
    onchange: (value: string) => void;
    placeholder?: string;
    disabled?: boolean;
  };

  let { label, options, value, onchange, placeholder = 'Select…', disabled = false }: Props = $props();
  let open = $state(false);
  let activeIndex = $state(0);
  let root: HTMLDivElement;
  let triggerHost: HTMLDivElement;
  let listbox = $state<HTMLDivElement>();
  let selected = $derived(options.find((option) => option.value === value));

  function optionElements() {
    return listbox?.querySelectorAll<HTMLButtonElement>('[role="option"]') ?? [];
  }

  function focusOption(index: number) {
    const elements = optionElements();
    if (!elements.length) return;
    activeIndex = (index + elements.length) % elements.length;
    elements[activeIndex].focus();
  }

  async function show(preferredIndex = options.findIndex((option) => option.value === value)) {
    if (disabled) return;
    open = true;
    activeIndex = preferredIndex < 0 ? 0 : preferredIndex;
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

  function choose(option: Option) {
    onchange(option.value);
    close(true);
  }

  function onTriggerKeydown(event: KeyboardEvent) {
    if (event.key === 'Escape' && open) {
      event.preventDefault();
      close(true);
    } else if (['Enter', ' ', 'ArrowDown', 'ArrowUp'].includes(event.key)) {
      event.preventDefault();
      show(event.key === 'ArrowUp' ? options.length - 1 : undefined);
    }
  }

  function onOptionKeydown(event: KeyboardEvent, option: Option, index: number) {
    if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
      event.preventDefault();
      focusOption(index + (event.key === 'ArrowDown' ? 1 : -1));
    } else if (event.key === 'Enter') {
      event.preventDefault();
      choose(option);
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
      style="width: 100%; min-height: 32px; position: relative;"
      aria-label={`${label}: ${selected?.label ?? placeholder}`}
      aria-haspopup="listbox"
      aria-expanded={open}
      {disabled}
      onclick={() => open ? close() : show()}
      onkeydown={onTriggerKeydown}
    >
      <span class:placeholder={!selected}>{selected?.label ?? placeholder}</span>
      <span class="chevron" aria-hidden="true">⌄</span>
    </Button>
  </div>

  {#if open}
    <div class="popover" bind:this={listbox} role="listbox" aria-label={label}>
      {#each options as option, index (option.value)}
        <button
          type="button"
          class="option"
          class:selected={option.value === value}
          role="option"
          aria-selected={option.value === value}
          tabindex="-1"
          onclick={() => choose(option)}
          onkeydown={(event) => onOptionKeydown(event, option, index)}
          onfocus={() => activeIndex = index}
        >
          <span>{option.label}</span>
          {#if option.description}<small>{option.description}</small>{/if}
        </button>
      {:else}
        <span class="empty">No options</span>
      {/each}
    </div>
  {/if}
</div>

<style>
  .field { position: relative; display: flex; min-width: 0; flex-direction: column; gap: 5px; }
  .label { font-size: 11px; color: var(--muted); }
  .trigger-host { display: flex; }
  .placeholder { color: var(--placeholder); }
  .chevron { position: absolute; right: 10px; color: var(--faint); }
  .popover {
    position: absolute;
    top: calc(100% + 5px);
    left: 0;
    z-index: 20;
    width: max(100%, 220px);
    max-height: 240px;
    overflow-y: auto;
    padding: 4px;
    border: 1px solid var(--line-strong);
    border-radius: var(--radius-xl);
    background: var(--surface);
    box-shadow: var(--shadow-popover);
  }
  .option {
    display: flex;
    width: 100%;
    min-height: 32px;
    flex-direction: column;
    justify-content: center;
    gap: 2px;
    padding: 5px 8px;
    border: 0;
    border-radius: var(--radius-md);
    background: transparent;
    color: var(--ink);
    text-align: left;
  }
  .option:hover, .option:focus-visible { background: var(--surface-hover); }
  .option.selected { background: var(--action-tint); color: var(--action-dark); }
  .option span { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .option small { color: var(--muted-2); font-size: 10.5px; }
  .option:focus-visible { outline: 2px solid var(--action); outline-offset: -2px; }
  .empty { display: block; padding: 8px; font-size: 11.5px; color: var(--muted); }
</style>
