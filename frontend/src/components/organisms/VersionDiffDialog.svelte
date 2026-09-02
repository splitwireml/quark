<script lang="ts">
  import Button from '../atoms/Button.svelte';
  import type { SerializableValue, VersionDiff } from '../../lib/types';

  type Props = {
    diff: VersionDiff;
    setDialog: (dialog: HTMLDialogElement | null) => void;
    onClose: () => void;
  };
  let { diff, setDialog, onClose }: Props = $props();
  let mode = $state<'summary' | 'sql'>('summary');

  function dialogRef(node: HTMLDialogElement) {
    setDialog(node);
    return { destroy: () => setDialog(null) };
  }
  function label(name: string): string {
    const words = name.replace(/([a-z])([A-Z])/g, '$1 $2').replace(/[_-]+/g, ' ');
    return words.charAt(0).toUpperCase() + words.slice(1);
  }
  function detail(value: SerializableValue): string {
    if (value === null) return 'None';
    if (typeof value === 'boolean') return value ? 'Yes' : 'No';
    if (Array.isArray(value)) return value.map(detail).join(', ') || 'None';
    if (typeof value === 'object') return Object.entries(value).map(([name, item]) => `${label(name)}: ${detail(item)}`).join(', ');
    return String(value);
  }
  function detailEntries(value?: Record<string, SerializableValue>) {
    return value ? Object.entries(value).filter(([name]) => name.toLowerCase() !== 'sql') : [];
  }
  function close(event: MouseEvent) {
    (event.currentTarget as HTMLElement).closest('dialog')?.close();
  }
</script>

<dialog use:dialogRef onclose={onClose} aria-labelledby="version-diff-title">
  <header>
    <div><h2 id="version-diff-title">Version diff</h2><p>{diff.parentId} → {diff.versionId}</p></div>
    <button type="button" class="close" onclick={close} aria-label="Close version diff">×</button>
  </header>
  <div class="body">
    <div class="modes" role="group" aria-label="Version diff display">
      <button type="button" aria-pressed={mode === 'summary'} onclick={() => mode = 'summary'}>Summary</button>
      <button type="button" aria-pressed={mode === 'sql'} onclick={() => mode = 'sql'}>SQL</button>
    </div>

    {#if mode === 'summary'}
      <section class="changes">
        <h3>Changes</h3>
        <ol>
          {#each diff.changes as change, index (`${change.kind}-${index}`)}
            {@const entries = detailEntries(change.details)}
            <li>
              <strong>{change.summary}</strong>
              {#if entries.length}
                <dl>{#each entries as [name, value] (name)}<div><dt>{label(name)}</dt><dd>{detail(value)}</dd></div>{/each}</dl>
              {/if}
            </li>
          {:else}
            <li class="empty">No recorded changes.</li>
          {/each}
        </ol>
      </section>
      <section>
        <h3>Version comparison</h3>
        <div class="snapshots">
          <article>
            <h4>Before <span>{diff.parentId}</span></h4>
            <dl class="snapshot-details">
              <div><dt>Column order</dt><dd>{diff.before.columns.join(' → ') || 'None'}</dd></div>
              <div><dt>Hidden columns</dt><dd>{diff.before.hiddenColumns.join(', ') || 'None'}</dd></div>
            </dl>
          </article>
          <article>
            <h4>After <span>{diff.versionId}</span></h4>
            <dl class="snapshot-details">
              <div><dt>Column order</dt><dd>{diff.after.columns.join(' → ') || 'None'}</dd></div>
              <div><dt>Hidden columns</dt><dd>{diff.after.hiddenColumns.join(', ') || 'None'}</dd></div>
            </dl>
          </article>
        </div>
      </section>
    {:else}
      <section>
        <h3>SQL comparison</h3>
        <div class="snapshots sql">
          <article><h4>Before <span>{diff.parentId}</span></h4><pre aria-label={`SQL before ${diff.parentId}`}>{diff.before.sql}</pre></article>
          <article><h4>After <span>{diff.versionId}</span></h4><pre aria-label={`SQL after ${diff.versionId}`}>{diff.after.sql}</pre></article>
        </div>
      </section>
    {/if}
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
  .modes { display: inline-flex; margin-bottom: 16px; }
  .modes button { height: 26px; padding: 0 11px; border: 1px solid var(--control-border); background: var(--surface); color: var(--muted); font-size: 11px; }
  .modes button:first-child { border-radius: var(--radius-md) 0 0 var(--radius-md); }
  .modes button:last-child { margin-left: -1px; border-radius: 0 var(--radius-md) var(--radius-md) 0; }
  .modes button[aria-pressed="true"] { position: relative; border-color: var(--action); background: var(--action-tint); color: var(--action-dark); }
  h3 { margin-bottom: 8px; font-size: 12.5px; }
  h4 { margin-bottom: 9px; font-size: 11.5px; }
  h4 span { margin-left: 4px; font: 10.5px var(--font-mono); color: var(--muted); }
  .changes { margin-bottom: 18px; }
  ol { display: grid; gap: 8px; margin: 0; padding: 0; list-style: none; }
  li { padding: 10px 11px; border: 1px solid var(--line); border-radius: var(--radius-lg); font-size: 12px; }
  li strong { color: var(--ink); }
  li.empty { color: var(--muted); }
  li dl { display: grid; gap: 5px; margin: 8px 0 0; }
  li dl div { display: grid; grid-template-columns: minmax(90px, 0.35fr) minmax(0, 1fr); gap: 10px; }
  li dt { color: var(--muted); }
  li dd { margin: 0; overflow-wrap: anywhere; color: var(--ink-2); }
  .snapshots { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 12px; }
  .snapshots article { min-width: 0; padding: 11px; border: 1px solid var(--line); border-radius: var(--radius-lg); }
  .snapshot-details { display: grid; gap: 10px; margin: 0; }
  .snapshot-details dt { margin-bottom: 3px; font-size: 10.5px; color: var(--muted); }
  .snapshot-details dd { margin: 0; overflow-wrap: anywhere; font: 10.5px/1.5 var(--font-mono); color: var(--ink-2); }
  pre { margin: 0; padding: 8px; overflow: auto; border-radius: var(--radius-md); background: var(--surface-inset); font: 10.5px/1.5 var(--font-mono); white-space: pre-wrap; word-break: break-word; }
  @media (max-width: 680px) {
    .snapshots { grid-template-columns: 1fr; }
    li dl div { grid-template-columns: 1fr; gap: 2px; }
  }
</style>
