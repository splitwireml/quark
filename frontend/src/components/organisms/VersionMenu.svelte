<script lang="ts">
  import IconButton from '../atoms/IconButton.svelte';
  import { dismissable } from '../../lib/dismiss';
  import { versionLabel } from '../../lib/versioning';
  import type { Version, ViewHistory } from '../../lib/types';

  type Props = {
    open: boolean;
    history: ViewHistory | undefined;
    storageError: string;
    onRestore: (version: Version) => void;
    onDiff: (version: Version, trigger: HTMLButtonElement) => void;
    onClose: () => void;
  };

  let { open, history, storageError, onRestore, onDiff, onClose }: Props = $props();

  let versions = $derived(history ? [...history.versions].reverse() : []);

  function choose(version: Version) {
    if (version.id !== history?.activeVersionId) onRestore(version);
    onClose();
  }
</script>

{#if open}
  <div class="panel" use:dismissable={onClose} role="group" aria-label="Versions">
    {#if storageError}<p class="error" role="alert">{storageError}</p>{/if}
    <ul class="list">
      {#each versions as version (version.id)}
        {@const active = version.id === history?.activeVersionId}
        <li class:active>
          <button type="button" class="entry" onclick={() => choose(version)} aria-current={active ? 'true' : undefined}>
            <span class="line">
              <span class="label">{versionLabel(version)}</span>
              <time datetime={version.timestamp}>{new Date(version.timestamp).toLocaleString()}</time>
            </span>
            <span class="summary">{version.changes.length ? version.changes.map((change) => change.summary).join(' · ') : 'Created'}</span>
          </button>
          {#if version.parentId}
            <IconButton
              type="button" icon="compare" label={`Compare ${versionLabel(version)} with its parent`}
              onclick={(event: MouseEvent) => onDiff(version, event.currentTarget as HTMLButtonElement)}
            />
          {/if}
        </li>
      {/each}
    </ul>
  </div>
{/if}

<style>
  .panel {
    position: absolute;
    top: calc(100% + 6px);
    left: 0;
    z-index: 12;
    width: 320px;
    padding: 5px;
    border: 1px solid var(--line-strong);
    border-radius: var(--radius-xl);
    background: var(--surface);
    box-shadow: var(--shadow-popover);
    transform-origin: top left;
    animation: version-menu-in 200ms cubic-bezier(0.32, 0.72, 0, 1);
  }
  @keyframes version-menu-in {
    from { opacity: 0; transform: translateY(-6px) scale(0.96); }
    to { opacity: 1; transform: none; }
  }

  .list { max-height: 320px; overflow-y: auto; margin: 0; padding: 0; list-style: none; display: flex; flex-direction: column; gap: 1px; }
  li { display: flex; align-items: center; gap: 4px; padding-right: 4px; border-radius: var(--radius-lg); }
  li:hover { background: var(--surface-hover); }
  li.active { background: var(--action-tint); }

  .entry {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 7px 8px;
    border: 0;
    border-radius: var(--radius-lg);
    background: transparent;
    text-align: left;
  }
  .line { display: flex; align-items: baseline; justify-content: space-between; gap: 8px; }
  .label { font: 500 11.5px var(--font-mono); color: var(--ink); }
  li.active .label { color: var(--action-dark); }
  time { flex: none; font: 10px var(--font-mono); color: var(--faint); }
  .summary { font-size: 11.5px; color: var(--muted); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .error { margin: 4px 6px 8px; font-size: 11.5px; color: var(--error); }
</style>
