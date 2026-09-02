<script lang="ts">
  import Button from '../atoms/Button.svelte';

  type Props = {
    setEditorHost: (el: HTMLDivElement | null) => void;
    hasError: boolean;
    sqlError: string;
    onClose: () => void;
    onRun: () => void;
    canRun: boolean;
    running: boolean;
  };
  let { setEditorHost, hasError, sqlError, onClose, onRun, canRun, running }: Props = $props();

  function hostRef(node: HTMLDivElement) {
    setEditorHost(node);
    return { destroy: () => setEditorHost(null) };
  }
</script>

<aside class="panel" aria-labelledby="sql-editor-title">
  <header>
    <div><strong id="sql-editor-title">SQL view</strong><span>DuckDB SQL</span></div>
    <button class="close" onclick={onClose} aria-label="Close SQL editor" title="Close SQL editor">×</button>
  </header>
  <div use:hostRef class="editor" class:has-error={hasError}></div>
  {#if sqlError}<p class="error" role="alert">{sqlError}</p>{/if}
  <footer>
    <Button variant="primary" title="Run SQL and save View (Shift+Enter)" onclick={onRun} disabled={!canRun}>{running ? 'Running…' : 'Run & save View'}</Button>
  </footer>
</aside>

<style>
  .panel {
    position: absolute;
    right: 20px;
    bottom: 20px;
    z-index: 12;
    width: min(640px, calc(100% - 40px));
    max-height: min(480px, calc(100% - 40px));
    display: flex;
    flex-direction: column;
    border-radius: var(--radius-2xl);
    background: var(--surface);
    border: 1px solid var(--line-strong);
    box-shadow: var(--shadow-panel);
    overflow: hidden;
  }
  header {
    flex: none;
    display: flex;
    align-items: center;
    justify-content: space-between;
    height: 38px;
    padding: 0 12px;
    background: var(--surface-2);
    border-bottom: 1px solid var(--line);
  }
  header strong { font-size: 12.5px; }
  header span { margin-left: 8px; font-size: 11px; color: var(--faint); }
  .close { width: 22px; height: 22px; border-radius: var(--radius-sm); border: none; background: transparent; color: var(--muted); font-size: 14px; }
  .close:hover { background: var(--surface-hover); }
  .editor { flex: 1; min-height: 160px; overflow: auto; font-family: var(--font-mono); font-size: 12.5px; }
  .editor.has-error { outline: 1px solid var(--error); outline-offset: -1px; }
  .error { margin: 0; padding: 8px 12px; font-size: 11.5px; color: var(--error); background: color-mix(in srgb, var(--error) 8%, var(--surface)); }
  footer { flex: none; display: flex; justify-content: flex-end; gap: 8px; padding: 10px 12px; border-top: 1px solid var(--line); }
</style>
