<script lang="ts">
  type Props = { glyph: string; title: string; detail: string; selected: boolean; onselect: () => void };
  let { glyph, title, detail, selected, onselect }: Props = $props();
</script>

<button type="button" class:selected aria-pressed={selected} onclick={onselect}>
  <span class="glyph" aria-hidden="true">{glyph}</span>
  <span class="copy"><strong>{title}</strong><small>{detail}</small></span>
</button>

<style>
  button { display: flex; align-items: flex-start; gap: 10px; width: 100%; min-width: 0; padding: 10px 12px; border: 1px solid var(--line-strong); border-radius: var(--radius-xl); background: var(--surface); color: var(--ink); text-align: left; transition: border-color 140ms ease, background 140ms ease; }
  button:hover { border-color: var(--muted); }
  button.selected { border-color: var(--action); background: var(--action-tint); }
  .glyph { flex: none; display: grid; place-items: center; width: 26px; height: 26px; border-radius: var(--radius-md); background: var(--surface-inset); color: var(--muted); font: 500 12px var(--font-mono); transition: background 140ms ease, color 140ms ease; }
  .selected .glyph { background: var(--surface); color: var(--action-dark); }
  .copy { min-width: 0; display: grid; gap: 2px; }
  strong { font-size: 12.5px; font-weight: 600; }
  small { font-size: 11px; line-height: 1.35; color: var(--muted); }
  @media (prefers-reduced-motion: reduce) { button, .glyph { transition: none; } }
</style>
