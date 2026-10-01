<script lang="ts">
  import type { AggregateCount, MetricAlign, MetricFont, MetricSize } from '../../lib/types';

  type Props = {
    value: string | number | boolean | null;
    title: string;
    rows: AggregateCount;
    count: (value: AggregateCount) => string;
    compact: (value: number | string | null | undefined) => string;
    align?: MetricAlign;
    font?: MetricFont;
    size?: MetricSize;
    frameTitle?: string;
  };

  let { value, title, rows, count, compact, align = 'center', font = 'sans', size = 'fit', frameTitle }: Props = $props();
  let text = $derived(typeof value === 'boolean' ? String(value) : compact(value));
  // The tile header already names the card, so the caption only earns its line when it says something else.
  let caption = $derived(frameTitle?.trim() === title.trim() ? '' : title);
</script>

<div class="metric" class:mono={font === 'mono'} style:--metric-align={align} role="figure" aria-label={`${title}: ${text}`}>
  <!-- The number is keyed on its own text so a filtered dashboard re-reads it in place. -->
  {#key text}<strong class={`size-${size}`} class:long={size === 'fit' && text.length > 12}>{text}</strong>{/key}
  {#if caption}<span class="title">{caption}</span>{/if}
  <span class="rows">{count(rows)} rows</span>
</div>

<style>
  .metric { flex: 1; min-width: 0; min-height: 0; display: flex; flex-direction: column; align-items: var(--metric-align); justify-content: center; gap: 4px; padding: 10px; container-type: inline-size; text-align: var(--metric-align); }
  strong { max-width: 100%; overflow-wrap: anywhere; color: var(--ink); font: 600 clamp(20px, 15cqw, 52px)/1.1 var(--font-ui); font-variant-numeric: tabular-nums; animation: settle 260ms cubic-bezier(0.22, 1, 0.36, 1); }
  strong.long { font-size: clamp(15px, 7cqw, 26px); }
  .size-sm { font-size: 20px; }
  .size-md { font-size: 30px; }
  .size-lg { font-size: 44px; }
  .mono strong { font-family: var(--font-mono); letter-spacing: -0.02em; }
  .title { max-width: 100%; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; color: var(--muted); font-size: 12.5px; }
  .mono .title { font-family: var(--font-mono); font-size: 11.5px; }
  .rows { color: var(--muted); font-size: 10.5px; }
  @keyframes settle { from { opacity: 0; transform: translateY(4px); } to { opacity: 1; transform: none; } }
  @media (prefers-reduced-motion: reduce) {
    strong { animation: none; }
  }
</style>
