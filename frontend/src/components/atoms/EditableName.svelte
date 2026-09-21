<script lang="ts">
  import { tick } from 'svelte';
  import type { HTMLButtonAttributes } from 'svelte/elements';

  let { value, label, onSave, onEdit, maxlength = 120, onkeydown, ...rest }: HTMLButtonAttributes & {
    value: string;
    label: string;
    onSave: (value: string) => void;
    onEdit?: () => void;
    maxlength?: number;
  } = $props();
  let editing = $state(false);
  let draft = $state('');
  let button = $state<HTMLButtonElement>();

  function edit() {
    onEdit?.();
    draft = value;
    editing = true;
  }

  async function finish(save: boolean, focus = false) {
    if (!editing) return;
    editing = false;
    if (save && draft.trim() && draft.trim() !== value) onSave(draft.trim());
    if (focus) { await tick(); button?.focus({ preventScroll: true }); }
  }

  function selectInput(node: HTMLInputElement) {
    node.focus({ preventScroll: true });
    node.select();
  }
</script>

{#if editing}
  <input class="name-input" aria-label={label} bind:value={draft} {maxlength} {@attach selectInput}
    onblur={() => finish(true)}
    onkeydown={(event) => {
      if (event.key === 'Enter' || event.key === 'Escape') {
        event.preventDefault();
        event.stopPropagation();
        void finish(event.key === 'Enter', true);
      }
    }} />
{:else}
  <button bind:this={button} type="button" {...rest} title="Double-click or press F2 to rename"
    ondblclick={edit}
    onkeydown={(event) => {
      if (event.key === 'F2') { event.preventDefault(); edit(); }
      else onkeydown?.(event);
    }}>{value}</button>
{/if}

<style>
  .name-input { min-width: 0; width: 100%; height: 28px; padding: 0 6px; border: 1px solid var(--action); border-radius: var(--radius-md); background: var(--surface); color: var(--ink); font: inherit; outline: none; }
</style>
