<script lang="ts">
  import SavedQueryCard from '../molecules/SavedQueryCard.svelte';
  import type { SavedQuery } from '../../lib/types';

  type Props = {
    savedQueries: SavedQuery[];
    storageError: string;
    onRun: (saved: SavedQuery) => void;
    onDelete: (id: string) => void;
  };
  let { savedQueries, storageError, onRun, onDelete }: Props = $props();
</script>

<section class="pane" aria-label="Saved queries">
  {#if storageError}<p class="error" role="alert">{storageError}</p>{/if}
  {#each savedQueries as saved (saved.id)}
    <SavedQueryCard {saved} onrun={() => onRun(saved)} ondelete={() => onDelete(saved.id)} />
  {:else}
    <div class="empty"><strong>No saved queries</strong><p>Save a builder query or SQL statement to keep it in this browser.</p></div>
  {/each}
</section>

<style>
  .pane { padding: 20px; display: flex; flex-direction: column; gap: 12px; overflow-y: auto; flex: 1; min-height: 0; }
  .error { margin: 0; font-size: 12px; color: var(--error); }
  .empty { padding: 40px 0; text-align: center; color: var(--muted); }
  .empty strong { display: block; margin-bottom: 6px; color: var(--ink); font-size: 13.5px; }
  .empty p { margin: 0; font-size: 12.5px; }
</style>
