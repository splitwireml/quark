<script lang="ts">
  import Button from '../atoms/Button.svelte';
  import type { DatasetVersionHistory, Version, View } from '../../lib/types';

  type Props = {
    history: DatasetVersionHistory | undefined;
    storageError: string;
    onRestore: (version: Version) => void;
    onDiff: (version: Version) => void;
    onOpenView: (view: View) => void;
  };
  let { history, storageError, onRestore, onDiff, onOpenView }: Props = $props();
</script>

<section class="pane" aria-label="Versions and Views">
  {#if storageError}<p class="error" role="alert">{storageError}</p>{/if}
  {#if history}
    <section aria-labelledby="versions-title">
      <h2 id="versions-title">Versions</h2>
      <div class="list">
        {#each [...history.versions].reverse() as version (version.id)}
          <article class:active={version.id === history.activeVersionId}>
            <div><strong>Version {version.number}</strong><time datetime={version.timestamp}>{new Date(version.timestamp).toLocaleString()}</time></div>
            <p>{version.changes.length ? version.changes.map((change) => change.summary).join(' · ') : 'Source'}</p>
            <footer>
              <Button onclick={() => onRestore(version)}>Restore</Button>
              {#if version.number > 1}<Button onclick={() => onDiff(version)}>View diff</Button>{/if}
            </footer>
          </article>
        {/each}
      </div>
    </section>
    <section aria-labelledby="views-title">
      <h2 id="views-title">Views</h2>
      <div class="list">
        {#each [...history.views].reverse() as view (view.id)}
          <article>
            <div><strong>{view.name}</strong><time datetime={view.timestamp}>{new Date(view.timestamp).toLocaleString()}</time></div>
            <pre>{view.sql}</pre>
            <footer><Button onclick={() => onOpenView(view)}>Open View</Button></footer>
          </article>
        {:else}
          <p class="empty">No Views saved for this dataset.</p>
        {/each}
      </div>
    </section>
  {:else}
    <p class="empty">Open a dataset to create its source version.</p>
  {/if}
</section>

<style>
  .pane { flex: 1; min-height: 0; overflow-y: auto; display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); align-items: start; gap: 20px; padding: 20px; }
  section section { min-width: 0; }
  h2 { margin: 0 0 10px; font-size: 13px; color: var(--ink); }
  .list { display: flex; flex-direction: column; gap: 10px; }
  article { min-width: 0; padding: 12px; border: 1px solid var(--line); border-radius: var(--radius-xl); background: var(--surface); }
  article.active { border-color: var(--action-tint-border); background: var(--action-tint); }
  article > div { display: flex; justify-content: space-between; gap: 10px; }
  strong { font-size: 12.5px; }
  time { flex: none; font: 10px var(--font-mono); color: var(--faint); }
  p { margin: 7px 0 0; font-size: 11.5px; color: var(--muted); }
  pre { margin: 8px 0 0; padding: 8px; overflow: hidden; border: 1px solid var(--line); border-radius: var(--radius-md); background: var(--surface-inset); font: 10.5px/1.5 var(--font-mono); color: var(--ink-2); white-space: pre-wrap; word-break: break-word; }
  footer { display: flex; gap: 7px; margin-top: 10px; }
  .empty { margin: 0; padding: 18px 0; color: var(--muted); }
  .error { grid-column: 1 / -1; margin: 0; color: var(--error); }
  @media (max-width: 760px) { .pane { grid-template-columns: 1fr; } }
</style>
