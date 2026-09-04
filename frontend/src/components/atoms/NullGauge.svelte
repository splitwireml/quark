<script lang="ts">
  type Props = { fraction: number };
  let { fraction }: Props = $props();
  let percent = $derived(Math.round(fraction * 100));
  let warn = $derived(fraction >= 0.3);
</script>

<span class="gauge-row">
  <span class="track">
    <span class="fill" class:warn style:width={`${Math.min(100, percent)}%`}></span>
  </span>
  {#if percent > 0}<span class="label" class:warn>{percent}% null</span>{/if}
</span>

<style>
  .gauge-row { display: inline-flex; align-items: center; gap: 6px; flex: 1; min-width: 0; }
  /* Reads as a ruled scale across the seam even at zero nulls. */
  .track {
    position: relative;
    flex: 1;
    min-width: 12px;
    height: 3px;
    border-radius: 1.5px;
    background: color-mix(in srgb, var(--action) 15%, transparent);
    overflow: hidden;
  }
  .fill { position: absolute; inset: 0 auto 0 0; background: var(--glyph); border-radius: 1.5px; }
  .fill.warn { background: var(--warning-fill); }
  .label { font-family: var(--font-mono); font-size: 10.5px; color: var(--faint); flex: none; }
  .label.warn { color: var(--warning); }
</style>
