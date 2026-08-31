<script lang="ts">
  import type { Snippet } from 'svelte';

  type Props = {
    title: string;
    subtitle: string;
    typeLabel: string;
    onClose: () => void;
    setPanel: (el: HTMLElement | null) => void;
    body: Snippet;
    footer: Snippet;
  };
  let { title, subtitle, typeLabel, onClose, setPanel, body, footer }: Props = $props();

  function panelRef(node: HTMLElement) {
    setPanel(node);
    return { destroy: () => setPanel(null) };
  }
</script>

<button class="backdrop" type="button" tabindex="-1" aria-label="Close inspector" onclick={onClose}></button>
<div use:panelRef class="inspector" role="dialog" aria-modal="true" aria-labelledby="inspector-title">
  <header>
    <div>
      <p>{subtitle}</p>
      <h2 id="inspector-title">{title}</h2>
      <small>{typeLabel}</small>
    </div>
    <button class="close" onclick={onClose} aria-label="Close inspector" title="Close inspector">×</button>
  </header>
  <div class="body">{@render body()}</div>
  <footer>{@render footer()}</footer>
</div>

<style>
  .backdrop {
    position: fixed; inset: 0; z-index: 15;
    background: rgba(15, 22, 32, 0.18);
    border: none;
  }
  .inspector {
    position: absolute;
    top: 0; right: 0; bottom: 0;
    z-index: 16;
    width: var(--inspector);
    max-width: 100%;
    display: flex;
    flex-direction: column;
    background: var(--surface);
    border-left: 1px solid var(--line-strong);
    box-shadow: var(--shadow-panel);
  }
  header {
    flex: none;
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 8px;
    padding: 14px 16px;
    border-bottom: 1px solid var(--line);
  }
  header p { margin: 0 0 3px; font-size: 10.5px; text-transform: uppercase; letter-spacing: 0.06em; color: var(--faint); }
  header h2 { margin: 0; font-size: 14px; font-weight: 600; font-family: var(--font-mono); color: var(--ink); }
  header small { font-family: var(--font-mono); font-size: 10.5px; color: var(--faint); }
  .close { width: 22px; height: 22px; flex: none; border-radius: var(--radius-sm); border: none; background: transparent; color: var(--muted); font-size: 14px; }
  .close:hover { background: var(--surface-hover); }
  .body { flex: 1; min-height: 0; overflow-y: auto; padding: 14px 16px; }
  footer { flex: none; display: flex; gap: 8px; justify-content: flex-end; padding: 12px 16px; border-top: 1px solid var(--line); }
</style>
