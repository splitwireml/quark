<script lang="ts">
  type Item = { label: string; color: string };
  type Props = {
    kind: 'series' | 'gradient' | 'size';
    title: string;
    items?: Item[];
    from?: string;
    to?: string;
    min?: string;
    max?: string;
  };
  let { kind, title, items = [], from = '', to = '', min = '', max = '' }: Props = $props();
</script>

<div class="legend" role="group" aria-label={title}>
  <span class="legend-title">{title}</span>
  {#if kind === 'series'}
    <div class="items">
      {#each items as item (item.label)}
        <span class="item"><span class="swatch" style:background={item.color}></span>{item.label}</span>
      {/each}
    </div>
  {:else if kind === 'gradient'}
    <div class="ramp">
      <span class="bound">{min}</span>
      <span class="bar" style:background={`linear-gradient(to right, ${from}, ${to})`}></span>
      <span class="bound">{max}</span>
    </div>
  {:else}
    <div class="sizes">
      <span class="bound">{min}</span>
      <span class="dot small"></span>
      <span class="dot large"></span>
      <span class="bound">{max}</span>
    </div>
  {/if}
</div>

<style>
  .legend { flex: none; display: flex; align-items: center; flex-wrap: wrap; gap: 4px 10px; padding: 2px 8px 4px; font-size: 10.5px; color: var(--muted); }
  .legend-title { font: 10px var(--font-mono); color: var(--faint); text-transform: uppercase; letter-spacing: .04em; }
  .items { display: flex; flex-wrap: wrap; gap: 4px 10px; }
  .item { display: inline-flex; align-items: center; gap: 5px; font-family: var(--font-mono); }
  .swatch { width: 9px; height: 9px; border-radius: 2px; }
  .ramp, .sizes { display: inline-flex; align-items: center; gap: 6px; }
  .bar { width: 96px; height: 9px; border-radius: 2px; }
  .bound { font: 10px var(--font-mono); color: var(--faint); }
  .dot { border-radius: 50%; background: var(--chart-mark-strong); }
  .dot.small { width: 5px; height: 5px; }
  .dot.large { width: 12px; height: 12px; }
</style>
