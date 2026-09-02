<script lang="ts">
  import Button from '../atoms/Button.svelte';
  import { versionLabel } from '../../lib/versioning';
  import type { Version, ViewHistory } from '../../lib/types';

  type Props = {
    history: ViewHistory | undefined;
    storageError: string;
    onRestore: (version: Version) => void;
    onDiff: (version: Version, trigger: HTMLButtonElement) => void;
  };
  let { history, storageError, onRestore, onDiff }: Props = $props();
</script>

<section class="pane" aria-label="Versions">
  {#if storageError}<p class="error" role="alert">{storageError}</p>{/if}
  {#if history}
    <section aria-labelledby="versions-title">
      <h2 id="versions-title">{history.name} versions</h2>
      <div class="list">
        {#each [...history.versions].reverse() as version (version.id)}
          <article class:active={version.id === history.activeVersionId} aria-current={version.id === history.activeVersionId ? 'true' : undefined}>
            <div><strong>{versionLabel(version)}</strong><time datetime={version.timestamp}>{new Date(version.timestamp).toLocaleString()}</time></div>
            <p>{version.changes.length ? version.changes.map((change) => change.summary).join(' · ') : 'Created'}</p>
            <footer>
              <Button onclick={() => onRestore(version)}>Restore</Button>
              {#if version.parentId}<Button onclick={(event: MouseEvent) => onDiff(version, event.currentTarget as HTMLButtonElement)}>View diff</Button>{/if}
            </footer>
          </article>
        {/each}
      </div>
    </section>
  {:else}
    <p class="empty">Choose a View to see its versions.</p>
  {/if}
</section>

<style>
  .pane { flex: 1; min-height: 0; overflow-y: auto; padding: 20px; }
  section section { width: min(680px, 100%); min-width: 0; }
  h2 { margin: 0 0 10px; font-size: 13px; color: var(--ink); }
  .list { display: flex; flex-direction: column; gap: 10px; }
  article { min-width: 0; padding: 12px; border: 1px solid var(--line); border-radius: var(--radius-xl); background: var(--surface); }
  article.active { border-color: var(--action-tint-border); background: var(--action-tint); }
  article > div { display: flex; justify-content: space-between; gap: 10px; }
  strong { font-size: 12.5px; }
  time { flex: none; font: 10px var(--font-mono); color: var(--faint); }
  p { margin: 7px 0 0; font-size: 11.5px; color: var(--muted); }
  footer { display: flex; gap: 7px; margin-top: 10px; }
  .empty { margin: 0; padding: 18px 0; color: var(--muted); }
  .error { margin: 0 0 12px; color: var(--error); }
</style>
