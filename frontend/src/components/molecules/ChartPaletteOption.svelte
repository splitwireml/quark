<script lang="ts">
  import PreferenceOption from './PreferenceOption.svelte';
  import type { ChartPaletteDefinition } from '../../lib/chartThemes';

  let { palette, selected, onSelect }: {
    palette: ChartPaletteDefinition;
    selected: boolean;
    onSelect: () => void;
  } = $props();
</script>

<PreferenceOption name="chart-palette" value={palette.id} title={palette.title} detail={palette.detail} {selected} {onSelect} previewHeight={68}>
  <div class="preview" aria-hidden="true">
    <div class="swatches">
      {#each palette.colors.seriesFill as color, index (`${palette.id}-${index}`)}
        <span class="swatch" style={`background:${color}`}></span>
      {/each}
    </div>
    <div class="rule" style={`background:${palette.colors.strong}`}></div>
  </div>
</PreferenceOption>

<style>
  .preview { display: flex; flex-direction: column; justify-content: center; gap: 8px; padding: 12px 14px 8px; }
  .swatches { display: grid; grid-template-columns: repeat(6, 1fr); gap: 3px; height: 18px; }
  .swatch { min-width: 0; border-radius: 2px; }
  .rule { width: 48%; height: 3px; border-radius: 1px; }
</style>
