<script lang="ts">
  type Props = { fraction: number };
  let { fraction }: Props = $props();
  let percent = $derived(Math.round(fraction * 100));
  let warn = $derived(fraction >= 0.3);
</script>

<span class="gauge-row">
  <span class="track" title={`${percent}% null`}>
    <span class="fill" class:warn style:width={`${Math.min(100, percent)}%`}></span>
  </span>
  {#if percent > 0}<span class="label" class:warn>{percent}%</span>{/if}
</span>

<style>
  .gauge-row { display: inline-flex; align-items: center; gap: 6px; flex: 1; min-width: 0; }
  .track {
    position: relative;
    flex: 1;
    height: 2px;
    border-radius: 1px;
    background: var(--line-soft);
    overflow: hidden;
  }
  .fill { position: absolute; inset: 0 auto 0 0; background: var(--control-border); border-radius: 1px; }
  .fill.warn { background: var(--warning-fill); }
  .label { font-family: var(--font-mono); font-size: 9px; color: var(--placeholder); flex: none; }
  .label.warn { color: var(--warning); }
</style>
