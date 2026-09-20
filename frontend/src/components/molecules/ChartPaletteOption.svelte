<script lang="ts">
  import PreferenceOption from './PreferenceOption.svelte';
  import { paletteColorsFor, type ChartPaletteDefinition } from '../../lib/chartThemes';
  import type { ColorScheme } from '../../lib/theme';

  let { palette, colorScheme, selected, onSelect }: {
    palette: ChartPaletteDefinition;
    colorScheme: ColorScheme;
    selected: boolean;
    onSelect: () => void;
  } = $props();

  let colors = $derived(paletteColorsFor(palette, colorScheme));
</script>

<PreferenceOption name="chart-palette" value={palette.id} title={palette.title} detail={palette.detail} {selected} {onSelect} previewHeight={68}>
  <div class="preview" aria-hidden="true">
    <div class="swatches">
      {#each colors.seriesFill as color, index (`${palette.id}-${index}`)}
        <span class="swatch" style={`background:${color}`}></span>
      {/each}
    </div>
    <div class="rule" style={`background:${colors.strong}`}></div>
  </div>
</PreferenceOption>

<style>
  .preview { display: flex; flex-direction: column; justify-content: center; gap: 8px; padding: 12px 14px 8px; }
  .swatches { display: grid; grid-template-columns: repeat(6, 1fr); gap: 3px; height: 18px; }
  .swatch { min-width: 0; border-radius: 2px; }
  .rule { width: 48%; height: 3px; border-radius: 1px; }
</style>
