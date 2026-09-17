<script lang="ts">
  import { onMount } from 'svelte';
  import Button from '../atoms/Button.svelte';
  let { term, busy, notice, onTerm, onFind, onClose }: { term: string; busy: boolean; notice: string; onTerm: (value: string) => void; onFind: (direction: 'next' | 'previous') => void; onClose: () => void } = $props();
  let input: HTMLInputElement;
  onMount(() => input.focus());
</script>
<form class="finder" aria-label="Find cell values" onsubmit={(event) => { event.preventDefault(); onFind('next'); }}>
  <label>Find values <input bind:this={input} onkeydown={(event) => { if (event.key === 'Escape') { event.preventDefault(); event.stopPropagation(); onClose(); } else if (event.key === 'Enter' && event.shiftKey) { event.preventDefault(); onFind('previous'); } }} aria-label="Find cell values" value={term} placeholder="Search all rows…" oninput={(event) => onTerm(event.currentTarget.value)} /></label>
  <Button type="button" disabled={busy || !term} onclick={() => onFind('previous')}>Previous</Button>
  <Button type="submit" disabled={busy || !term}>Next</Button>
  <span role="status">{busy ? 'Searching…' : notice || 'All rows · visible columns · Enter for next'}</span>
  <Button type="button" variant="ghost" onclick={onClose}>Close</Button>
</form>
<style>
  .finder { display: flex; align-items: center; gap: 8px; padding: 10px 20px; border-bottom: 1px solid var(--line); background: var(--surface); }
  label { display: flex; align-items: center; gap: 10px; font-size: 13px; }
  input { height: 30px; width: 220px; border: 1px solid var(--control-border); border-radius: var(--radius-md); background: var(--surface); padding: 0 8px; color: var(--ink); }
  span { flex: 1; font-size: 12px; color: var(--muted); }
</style>
