<script lang="ts">
  import type { HistogramBin, AggregateCount } from '../../lib/types';

  type Props = {
    bins: HistogramBin[];
    maxBin: number;
    readout: string;
    binLabel: (bin: HistogramBin) => string;
    count: (value: AggregateCount) => string;
    onfocusBin: (bin: HistogramBin) => void;
  };
  let { bins, maxBin, readout, binLabel, count, onfocusBin }: Props = $props();
</script>

<div class="bars">
  {#each bins as bin (bin.lower)}
    <button
      style:height={`${Math.max(3, (Number(bin.count) / maxBin) * 100)}%`}
      onclick={() => onfocusBin(bin)}
      onfocus={() => onfocusBin(bin)}
      aria-label={`${binLabel(bin)}: ${count(bin.count)} rows`}
      title={`${binLabel(bin)}: ${count(bin.count)} rows`}
    ></button>
  {/each}
</div>
<p class="readout" aria-live="polite">{readout}</p>

<style>
  .bars { display: flex; align-items: flex-end; gap: 2px; height: 96px; }
  .bars button { flex: 1; min-width: 2px; background: var(--action-tint-border); border-radius: 1px 1px 0 0; }
  .bars button:hover, .bars button:focus-visible { background: var(--action); }
  .readout { margin: 6px 0 0; font-size: 11px; color: var(--muted); }
</style>
