<script lang="ts">
  import Button from '../atoms/Button.svelte';
  import Icon from '../atoms/Icon.svelte';
  import SourceTreeItem from '../molecules/SourceTreeItem.svelte';
  import Eyebrow from '../atoms/Eyebrow.svelte';
  import type { Snippet } from 'svelte';
  import type { SourceSummary } from '../../lib/types';

  type Props = {
    error: string;
    mutating: boolean;
    nodes: SourceSummary[];
    loadedSourceIds: string[];
    loadingSourceId: string;
    onSelectSource: (id: string) => void;
    onShowAllSources: () => void;
    onUpload: (event: Event) => void;
    onRetry: () => void;
    attachForm: Snippet;
  };
  let { error, mutating, nodes, loadedSourceIds, loadingSourceId, onSelectSource, onShowAllSources, onUpload, onRetry, attachForm }: Props = $props();
  let hasSources = $derived(nodes.length > 0);
  let attachOpen = $state(false);
  const VISIBLE_SOURCES = 3;
  let visibleNodes = $derived(nodes.slice(0, VISIBLE_SOURCES));
</script>

<section class="welcome">
  <header>
    <div class="icon">Q</div>
    <h1>{hasSources ? 'Open a project source' : 'Add project sources'}</h1>
    <p>
      {hasSources
        ? 'Choose a source to open its Views. Quark keeps the work on this machine and loads only the page you are viewing.'
        : 'Open a local file or a read-only DuckDB database. Quark keeps the work on this machine and loads only the page you are viewing.'}
    </p>
  </header>

  {#if error}
    <div class="error" role="alert">
      <div><strong>Could not load Quark</strong><p>{error}</p></div>
      <Button onclick={onRetry}>Retry</Button>
    </div>
  {/if}

  {#if hasSources}
    <div class="sources-group">
      <div class="group-title"><Eyebrow>Sources</Eyebrow><span>{nodes.length}</span></div>
      <ul class="sources">
        {#each visibleNodes as node (node.id)}
          <li>
            <SourceTreeItem
              {node}
              size="comfortable"
              active={false}
              loading={loadingSourceId === node.id || loadingSourceId === '*'}
              loaded={loadedSourceIds.includes(node.id)}
              onselect={() => onSelectSource(node.id)}
            />
          </li>
        {/each}
      </ul>
      {#if nodes.length > VISIBLE_SOURCES}
        <button type="button" class="more" onclick={onShowAllSources}>
          Show all {nodes.length} sources
          <span class="more-glyph" aria-hidden="true"><Icon name="chevron" size={13} /></span>
        </button>
      {/if}
    </div>
  {:else}
    <ol class="steps">
      <li><b>1. Add a source</b><span>CSV, TSV, Parquet, JSON, JSONL/NDJSON, XLSX, DuckDB, or DB</span></li>
      <li><b>2. Choose a View</b><span>Source Views appear after the source opens</span></li>
      <li><b>3. Inspect the View</b><span>Filter, profile, hide columns, or dedupe by selected keys</span></li>
    </ol>
  {/if}

  <div class="add-group">
    <div class="group-title"><Eyebrow>{hasSources ? 'Add another source' : 'Add a source'}</Eyebrow></div>
    <div class="actions">
      <label class="action" class:disabled={mutating}>
        <Icon name="file" />
        <span class="label">Data file</span>
        <small>CSV, Parquet, JSON, XLSX</small>
        <input type="file" accept=".csv,.tsv,.parquet,.json,.ndjson,.jsonl,.xlsx" onchange={onUpload} disabled={mutating} />
      </label>
      <label class="action" class:disabled={mutating}>
        <Icon name="database" />
        <span class="label">DuckDB file</span>
        <small>Copied into the project</small>
        <input type="file" accept=".duckdb,.db" onchange={onUpload} disabled={mutating} />
      </label>
      <button type="button" class="action" class:open={attachOpen} aria-expanded={attachOpen} onclick={() => attachOpen = !attachOpen}>
        <Icon name="link" />
        <span class="label">Attach path</span>
        <small>Opened read-only in place</small>
      </button>
    </div>
    {#if attachOpen}
      <div class="attach-form">{@render attachForm()}</div>
    {/if}
  </div>
</section>

<style>
  .welcome {
    width: 100%;
    max-width: 520px;
    margin: auto;
    padding: 48px 24px;
    max-height: 100%;
    overflow-y: auto;
  }
  header { display: flex; flex-direction: column; }
  .icon {
    width: 40px; height: 40px;
    display: flex; align-items: center; justify-content: center;
    border-radius: var(--radius-lg);
    background: var(--ink-fill);
    color: #FFFFFF;
    font-family: var(--font-mono);
    font-weight: 600;
  }
  h1 { margin: 20px 0 0; font-size: 22px; font-weight: 600; letter-spacing: -0.01em; }
  p { margin: 8px 0 0; font-size: 14px; line-height: 1.55; color: var(--muted); }

  .sources-group { margin-top: 28px; }
  .group-title { display: flex; align-items: baseline; justify-content: space-between; padding-bottom: 8px; }
  .group-title span { font-family: var(--font-mono); font-size: 10px; color: var(--faint); }
  .sources { margin: 0; padding: 0; list-style: none; display: flex; flex-direction: column; gap: 4px; }
  .more {
    display: inline-flex; align-items: center; gap: 4px;
    margin-top: 8px;
    padding: 0;
    border: none;
    background: none;
    font-size: 12px;
    color: var(--action);
    cursor: pointer;
  }
  .more:hover { color: var(--action-dark); }
  .more-glyph { display: flex; color: var(--action); transition: transform 160ms cubic-bezier(0.22, 1, 0.36, 1); }
  .more:hover .more-glyph { transform: translateX(2px); }

  .steps { margin: 28px 0 0; padding: 0; list-style: none; display: flex; flex-direction: column; gap: 12px; }
  .steps li { display: flex; flex-direction: column; gap: 2px; }
  .steps b { font-size: 13px; color: var(--ink); }
  .steps span { font-size: 12px; color: var(--faint); }

  .error {
    display: flex; align-items: center; justify-content: space-between; gap: 12px;
    margin-top: 20px;
    padding: 12px;
    border-radius: var(--radius-lg);
    border: 1px solid var(--error);
    background: color-mix(in srgb, var(--error) 8%, var(--surface));
  }
  .error strong { color: var(--error); font-size: 12.5px; }
  .error p { margin: 2px 0 0; font-size: 12px; }

  .add-group { margin-top: 24px; padding-top: 20px; border-top: 1px solid var(--line); }
  .actions { display: grid; grid-template-columns: repeat(3, 1fr); gap: 8px; }
  .action {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 6px;
    min-width: 0;
    padding: 12px 12px 11px;
    border-radius: var(--radius-lg);
    border: 1px solid var(--control-border);
    background: var(--surface);
    text-align: left;
    color: var(--glyph);
    cursor: pointer;
    transition: border-color 120ms ease, background 120ms ease, color 120ms ease;
  }
  .action:hover:not(.disabled), .action.open { border-color: var(--action); background: var(--action-tint); color: var(--action); }
  .action.disabled { opacity: 0.5; pointer-events: none; }
  .action .label { font-size: 12.5px; font-weight: 500; color: var(--ink-2); }
  .action small { font-size: 10.5px; line-height: 1.35; color: var(--faint); }
  .action :global(svg) { transition: transform 160ms cubic-bezier(0.22, 1, 0.36, 1); }
  .action:hover:not(.disabled) :global(svg), .action.open :global(svg) { transform: translateY(-1px); }
  .action input { position: absolute; width: 1px; height: 1px; opacity: 0; }
  .attach-form { margin-top: 12px; }

  @media (prefers-reduced-motion: reduce) {
    .more-glyph { transition: none; }
    .action :global(svg) { transition: none; }
    .action:hover:not(.disabled) :global(svg), .action.open :global(svg) { transform: none; }
  }
</style>
