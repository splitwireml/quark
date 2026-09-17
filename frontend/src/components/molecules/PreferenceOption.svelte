<script lang="ts">
  import type { Snippet } from 'svelte';
  let { name, value, title, detail, selected, onSelect, children, previewHeight = 148 }: {
    name: string; value: string; title: string; detail: string; selected: boolean;
    onSelect: () => void; children: Snippet; previewHeight?: number;
  } = $props();
</script>

<label class:selected>
  <div class="preview" aria-hidden="true" style:height="{previewHeight}px">{@render children()}</div>
  <div class="option-copy"><div class="option-title"><strong>{title}</strong><input type="radio" {name} {value} checked={selected} onchange={onSelect} /></div><small>{detail}</small></div>
</label>

<style>
  label { display: flex; flex-direction: column; min-width: 0; cursor: pointer; border: 1px solid var(--line-strong); border-radius: var(--radius-xl); background: var(--surface); overflow: hidden; transition: border-color 140ms ease; }
  label:hover { border-color: var(--muted); }
  label.selected { border-color: var(--action); }
  label:focus-within { outline: 2px solid var(--action); outline-offset: 3px; }
  .preview { overflow: hidden; background: var(--surface-2); border-bottom: 1px solid var(--line); }
  .selected .preview { background: var(--action-tint); }
  .option-copy { padding: 17px 16px 20px; }
  .option-title { display: flex; align-items: center; justify-content: space-between; gap: 8px; margin-bottom: 8px; }
  strong { font-size: 13px; font-weight: 500; color: var(--ink); }
  input { accent-color: var(--action); margin: 0; width: 14px; height: 14px; flex: none; }
  small { display: block; font-size: 12px; line-height: 1.6; color: var(--muted); }
  @media (prefers-reduced-motion: reduce) { label { transition: none; } }
</style>
