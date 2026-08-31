<script lang="ts">
  type Props = {
    page: number;
    pageInput: string;
    totalPages: number;
    loadingData: boolean;
    onprev: () => void;
    onnext: () => void;
    onjump: (event: SubmitEvent) => void;
    setPageInput: (value: string) => void;
  };
  let { page, pageInput, totalPages, loadingData, onprev, onnext, onjump, setPageInput }: Props = $props();
</script>

<div class="pager">
  <button onclick={onprev} disabled={page <= 1 || loadingData} aria-label="Previous page">←</button>
  <form onsubmit={onjump}>
    <label for="page-number" class="sr-only">Page</label>
    <input id="page-number" type="number" min="1" max={Math.max(totalPages, 1)} value={pageInput} oninput={(event) => setPageInput((event.currentTarget as HTMLInputElement).value)} />
    <span>/ {totalPages.toLocaleString()}</span>
  </form>
  <button onclick={onnext} disabled={page >= totalPages || loadingData} aria-label="Next page">→</button>
</div>

<style>
  .pager {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    height: 24px;
    padding: 0 6px;
    border-radius: var(--radius-sm);
    border: 1px solid var(--control-border);
    background: var(--surface);
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--muted);
  }
  .pager button { color: var(--muted); padding: 0 2px; }
  .pager button:disabled { opacity: 0.4; }
  form { display: inline-flex; align-items: center; gap: 4px; }
  input { width: 34px; border: none; background: transparent; text-align: right; color: var(--ink); font: inherit; }
  span { color: var(--faint); }
</style>
