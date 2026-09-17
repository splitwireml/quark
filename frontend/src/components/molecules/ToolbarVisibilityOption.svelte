<script lang="ts">
  import Icon from '../atoms/Icon.svelte';
  import PreferenceOption from './PreferenceOption.svelte';
  import type { ToolbarVisibility } from '../../lib/commands';
  let { value, title, detail, selected, onSelect }: {
    value: ToolbarVisibility; title: string; detail: string; selected: boolean; onSelect: () => void;
  } = $props();
</script>

<PreferenceOption name="toolbar-visibility" {value} {title} {detail} {selected} {onSelect}>
  <div class="preview" class:hover-mode={value === 'hover'} class:hide-mode={value === 'hide'} aria-hidden="true">
    <div class="mini-window">
      <div class="mini-title"><span></span><i></i><i></i></div>
      <div class="mini-toolbar"><Icon name="sigma" size={12} /><Icon name="join" size={12} /><Icon name="columns" size={12} /><Icon name="duplicate" size={12} /><span></span></div>
      <div class="mini-edge"></div>
      <div class="mini-table">{#each [0, 1, 2, 3, 4] as row (row)}<div>{#each [0, 1, 2] as col (col)}<span><i style:width="{30 + ((row + col) % 3) * 18}%"></i></span>{/each}</div>{/each}</div>
    </div>
  </div>
</PreferenceOption>

<style>
  .preview { height: 100%; padding: 20px 16px 0; }
  .mini-window { height: 144px; overflow: hidden; border: 1px solid var(--line-strong); border-radius: 6px 6px 0 0; background: var(--surface); box-shadow: 0 3px 10px rgb(25 35 50 / 5%); }
  .mini-title { height: 26px; display: flex; align-items: center; gap: 5px; padding: 0 9px; border-bottom: 1px solid var(--line); }
  .mini-title span { width: 38%; height: 4px; margin-right: auto; border-radius: 2px; background: var(--muted); opacity: .6; }
  .mini-title i { width: 8px; height: 8px; border: 1px solid var(--line-strong); border-radius: 2px; }
  .mini-toolbar { display: flex; align-items: center; gap: 11px; height: 27px; padding: 0 9px; overflow: hidden; background: var(--surface-2); color: var(--muted); border-bottom: 1px solid var(--line); transition: height 160ms ease, opacity 160ms ease; }
  .mini-toolbar span { margin-left: auto; height: 11px; width: 30%; border: 1px solid var(--line-strong); border-radius: 3px; }
  .mini-edge { display: none; height: 7px; background: var(--surface-2); border-bottom: 1px solid var(--line); }
  .hover-mode .mini-toolbar, .hide-mode .mini-toolbar { height: 0; opacity: 0; border: 0; }
  .hover-mode .mini-edge { display: block; }
  .hover-mode .mini-edge::after { content: ''; display: block; margin: auto; width: 24px; height: 2px; background: var(--action); border-radius: 1px; }
  :global(label:hover) .hover-mode .mini-toolbar, :global(label:focus-within) .hover-mode .mini-toolbar { height: 27px; opacity: 1; }
  .mini-table > div { display: grid; grid-template-columns: repeat(3, 1fr); height: 20px; border-bottom: 1px solid var(--line-soft); }
  .mini-table > div:first-child { background: var(--surface-2); }
  .mini-table span { display: flex; align-items: center; padding-left: 9px; border-right: 1px solid var(--line-soft); }
  .mini-table i { display: block; height: 3px; border-radius: 1px; background: var(--line-strong); }
  @media (prefers-reduced-motion: reduce) { .mini-toolbar { transition: none; } }
</style>
