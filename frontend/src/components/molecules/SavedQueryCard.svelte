<script lang="ts">
  import Button from '../atoms/Button.svelte';
  import type { SavedQuery } from '../../lib/types';

  type Props = { saved: SavedQuery; onrun: () => void; ondelete: () => void };
  let { saved, onrun, ondelete }: Props = $props();
</script>

<article class="card">
  <div>
    <h2>{saved.name}</h2>
    <pre>{saved.sql}</pre>
  </div>
  <footer>
    <Button onclick={onrun}>Run</Button>
    <Button onclick={ondelete} aria-label={`Delete saved query ${saved.name}`}>Delete</Button>
  </footer>
</article>

<style>
  .card {
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 14px;
    border: 1px solid var(--line);
    border-radius: var(--radius-xl);
    background: var(--surface);
  }
  h2 { margin: 0 0 6px; font-size: 13.5px; font-weight: 600; color: var(--ink); }
  pre {
    margin: 0;
    padding: 9px 10px;
    border-radius: var(--radius-lg);
    background: var(--surface-inset);
    border: 1px solid var(--line);
    font-family: var(--font-mono);
    font-size: 11px;
    line-height: 1.6;
    color: var(--ink-2);
    white-space: pre-wrap;
    word-break: break-word;
  }
  footer { display: flex; gap: 8px; }
</style>
