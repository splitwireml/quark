<script lang="ts">
  import NullGauge from '../atoms/NullGauge.svelte';
  import type { ColumnInfo, SortCondition } from '../../lib/types';

  type LabelPart = { text: string; match: boolean };

  type Props = {
    column: ColumnInfo;
    labelParts: LabelPart[];
    sort: SortCondition | undefined;
    filtered: boolean;
    canQuery: boolean;
    protectedColumn: boolean;
    canHide: boolean;
    onsort: () => void;
    onfilter: (trigger: HTMLButtonElement) => void;
    onprofile: (trigger: HTMLButtonElement) => void;
    onhide: () => void;
  };

  let { column, labelParts, sort, filtered, canQuery, protectedColumn, canHide, onsort, onfilter, onprofile, onhide }: Props = $props();
  let width = $derived(Math.max(168, Math.min(360, column.name.length * 9 + (column.profile_kind ? 150 : 118))));
</script>

<th data-column={column.name} tabindex="-1" scope="col" style:min-width={`${width}px`}>
  <div class="head">
    <div class="label">
      <strong title={column.name}>
        {#each labelParts as part, index ((part.match ? 'm' : 't') + index + part.text)}
          {#if part.match}<mark>{part.text}</mark>{:else}{part.text}{/if}
        {/each}
      </strong>
      <small>{column.type}</small>
    </div>
    <div class="actions">
      {#if canQuery}
        <button class:on={!!sort} onclick={onsort} aria-label={`Sort ${column.name}`} title={`Sort ${column.name}`}>{sort?.direction === 'asc' ? '↑' : sort?.direction === 'desc' ? '↓' : '↕'}</button>
        <button class:on={filtered} onclick={(event) => onfilter(event.currentTarget as HTMLButtonElement)} aria-label={`Filter ${column.name}`} title={`Filter ${column.name}`}>⌕</button>
        {#if column.profile_kind}
          <button onclick={(event) => onprofile(event.currentTarget as HTMLButtonElement)} aria-label={`Profile column ${column.name}`} title={`Profile column ${column.name}`}>▥</button>
        {/if}
      {/if}
      <button onclick={onhide} disabled={protectedColumn || !canHide} aria-label={`Hide column ${column.name}`} title={`Hide column ${column.name}`}>×</button>
    </div>
    <NullGauge fraction={column.null_fraction} />
  </div>
</th>

<style>
  th {
    position: sticky;
    top: 0;
    z-index: 3;
    height: 48px;
    padding: 6px 10px 8px;
    box-sizing: border-box;
    background: var(--surface-3);
    border-right: 1px solid var(--line-soft);
    border-bottom: 1px solid var(--line);
    text-align: left;
    vertical-align: top;
  }
  .head { display: flex; flex-direction: column; gap: 4px; height: 100%; }
  .label { display: flex; align-items: baseline; gap: 6px; min-width: 0; }
  strong {
    font-family: var(--font-mono);
    font-size: 11.5px;
    font-weight: 600;
    color: var(--ink);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  strong mark { background: var(--action-tint); color: var(--action-dark); }
  small { font-family: var(--font-mono); font-size: 9px; letter-spacing: 0.04em; text-transform: uppercase; color: var(--faint); flex: none; }
  .actions { display: flex; align-items: center; gap: 3px; height: 16px; }
  .actions button {
    display: flex; align-items: center; justify-content: center;
    width: 16px; height: 16px;
    border-radius: var(--radius-sm);
    background: var(--surface);
    border: 1px solid var(--line);
    color: var(--muted);
    font-size: 9px;
    line-height: 1;
  }
  .actions button:hover:not(:disabled) { border-color: var(--faint); color: var(--ink); }
  .actions button.on { color: var(--action); border-color: var(--action-tint-border); background: var(--action-tint); }
  .actions button:disabled { opacity: 0.35; }
</style>
