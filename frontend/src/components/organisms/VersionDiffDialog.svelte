<script lang="ts">
  import Button from '../atoms/Button.svelte';
  import type { SerializableValue, VersionDiff } from '../../lib/types';

  type Props = {
    diff: VersionDiff;
    setDialog: (dialog: HTMLDialogElement | null) => void;
    onClose: () => void;
  };
  let { diff, setDialog, onClose }: Props = $props();

  function dialogRef(node: HTMLDialogElement) {
    setDialog(node);
    return { destroy: () => setDialog(null) };
  }
  function detail(value: SerializableValue): string {
    return typeof value === 'string' ? value : JSON.stringify(value);
  }
  function close(event: MouseEvent) {
    (event.currentTarget as HTMLElement).closest('dialog')?.close();
  }
</script>

<dialog use:dialogRef onclose={onClose} aria-labelledby="version-diff-title">
  <header>
    <div><h2 id="version-diff-title">Version diff</h2><p>{diff.parentId} → {diff.versionId}</p></div>
    <button class="close" onclick={close} aria-label="Close version diff">×</button>
  </header>
  <div class="body">
    <section>
      <h3>Changes</h3>
      <ol>
        {#each diff.changes as change, index (`${change.kind}-${index}`)}
          <li><strong>{change.summary}</strong>{#if change.details}<dl>{#each Object.entries(change.details) as [name, value] (name)}<div><dt>{name}</dt><dd>{detail(value)}</dd></div>{/each}</dl>{/if}</li>
        {/each}
      </ol>
    </section>
    <section class="snapshots">
      <div><h3>{diff.parentId}</h3><h4>Column order</h4><p>{diff.before.columns.join(' → ')}</p><h4>Hidden columns</h4><p>{diff.before.hiddenColumns.join(', ') || 'None'}</p><h4>SQL</h4><pre>{diff.before.sql}</pre></div>
      <div><h3>{diff.versionId}</h3><h4>Column order</h4><p>{diff.after.columns.join(' → ')}</p><h4>Hidden columns</h4><p>{diff.after.hiddenColumns.join(', ') || 'None'}</p><h4>SQL</h4><pre>{diff.after.sql}</pre></div>
    </section>
  </div>
  <footer><Button onclick={close}>Close</Button></footer>
</dialog>

<style>
  dialog { width: min(860px, calc(100vw - 32px)); max-height: calc(100vh - 32px); padding: 0; border: 1px solid var(--line-strong); border-radius: var(--radius-2xl); background: var(--surface); color: var(--ink); box-shadow: var(--shadow-panel); }
  dialog::backdrop { background: rgba(20, 24, 31, 0.45); }
  header, footer { display: flex; align-items: center; justify-content: space-between; gap: 10px; padding: 12px 14px; background: var(--surface-2); }
  header { border-bottom: 1px solid var(--line); }
  footer { justify-content: flex-end; border-top: 1px solid var(--line); }
  h2, h3, h4, p { margin: 0; }
  h2 { font-size: 14px; }
  header p { margin-top: 3px; font: 10.5px var(--font-mono); color: var(--muted); }
  .close { width: 26px; height: 26px; border: none; border-radius: var(--radius-md); background: transparent; color: var(--muted); font-size: 17px; }
  .body { overflow-y: auto; max-height: calc(100vh - 150px); padding: 14px; }
  h3 { margin-bottom: 8px; font-size: 12.5px; }
  h4 { margin: 9px 0 3px; font-size: 10.5px; color: var(--muted); }
  ol { margin: 0 0 16px; padding-left: 22px; }
  li { margin: 6px 0; font-size: 11.5px; }
  dl { display: flex; flex-wrap: wrap; gap: 5px 12px; margin: 5px 0 0; }
  dl div { display: flex; gap: 4px; }
  dt { color: var(--muted); }
  dd { margin: 0; font-family: var(--font-mono); }
  .snapshots { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 12px; }
  .snapshots > div { min-width: 0; padding: 11px; border: 1px solid var(--line); border-radius: var(--radius-lg); }
  .snapshots p { overflow-wrap: anywhere; font: 10.5px/1.5 var(--font-mono); color: var(--ink-2); }
  pre { margin: 0; padding: 8px; overflow: auto; border-radius: var(--radius-md); background: var(--surface-inset); font: 10.5px/1.5 var(--font-mono); white-space: pre-wrap; word-break: break-word; }
  @media (max-width: 680px) { .snapshots { grid-template-columns: 1fr; } }
</style>
