<script lang="ts">
  import Button from '../atoms/Button.svelte';
  import type { Snippet } from 'svelte';

  type Props = {
    error: string;
    mutating: boolean;
    onUpload: (event: Event) => void;
    onRetry: () => void;
    attachForm: Snippet;
  };
  let { error, mutating, onUpload, onRetry, attachForm }: Props = $props();
</script>

<section class="welcome">
  <div class="icon">Q</div>
  <h1>Explore local data</h1>
  <p>Open a local file or a read-only DuckDB database. Quark keeps the work on this machine and loads only the page you are viewing.</p>
  <ol class="steps">
    <li><b>1. Add a source</b><span>CSV, TSV, Parquet, JSON, JSONL/NDJSON, XLSX, DuckDB, or DB</span></li>
    <li><b>2. Choose a dataset</b><span>Tables and views appear after the source opens</span></li>
    <li><b>3. Inspect the data</b><span>Filter, profile, hide columns, or dedupe by selected keys</span></li>
  </ol>
  {#if error}
    <div class="error" role="alert">
      <div><strong>Could not load Quark</strong><p>{error}</p></div>
      <Button onclick={onRetry}>Retry</Button>
    </div>
  {/if}
  <label class="primary">
    Choose a file
    <input type="file" accept=".csv,.tsv,.parquet,.json,.ndjson,.jsonl,.xlsx,.duckdb,.db" onchange={onUpload} disabled={mutating} />
  </label>
  <details class="attach">
    <summary>Attach a local DuckDB database</summary>
    {@render attachForm()}
  </details>
</section>

<style>
  .welcome { max-width: 560px; margin: 64px auto; padding: 0 24px; display: flex; flex-direction: column; gap: 14px; }
  .icon {
    width: 40px; height: 40px;
    display: flex; align-items: center; justify-content: center;
    border-radius: var(--radius-lg);
    background: var(--ink-fill);
    color: #FFFFFF;
    font-family: var(--font-mono);
    font-weight: 600;
  }
  h1 { margin: 0; font-size: 22px; font-weight: 600; letter-spacing: -0.01em; }
  p { margin: 0; font-size: 14px; line-height: 1.55; color: var(--muted); }
  .steps { margin: 8px 0; padding: 0; list-style: none; display: flex; flex-direction: column; gap: 10px; }
  .steps li { display: flex; flex-direction: column; gap: 2px; }
  .steps b { font-size: 13px; color: var(--ink); }
  .steps span { font-size: 12px; color: var(--muted-2); }
  .error {
    display: flex; align-items: center; justify-content: space-between; gap: 12px;
    padding: 12px;
    border-radius: var(--radius-lg);
    border: 1px solid var(--error);
    background: color-mix(in srgb, var(--error) 8%, var(--surface));
  }
  .error strong { color: var(--error); font-size: 12.5px; }
  .error p { margin: 2px 0 0; font-size: 12px; }
  .primary {
    display: inline-flex; align-items: center; justify-content: center;
    height: 34px;
    border-radius: var(--radius-lg);
    background: var(--ink-fill);
    color: #FFFFFF;
    font-size: 13px;
    font-weight: 500;
    cursor: pointer;
    width: fit-content;
    padding: 0 16px;
  }
  .primary input { position: absolute; width: 1px; height: 1px; opacity: 0; }
  .attach summary { font-size: 12.5px; color: var(--action); cursor: pointer; }
</style>
