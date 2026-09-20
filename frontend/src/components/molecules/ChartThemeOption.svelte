<script lang="ts">
  import PreferenceOption from './PreferenceOption.svelte';
  import { chartPaletteFor, defaultChartThemePreferences, paletteColorsFor, type ChartTheme } from '../../lib/chartThemes';
  import type { ColorScheme } from '../../lib/theme';

  let { value, colorScheme, selected, onSelect }: {
    value: ChartTheme; colorScheme: ColorScheme; selected: boolean; onSelect: () => void;
  } = $props();

  const copy: Record<ChartTheme, { title: string; detail: string }> = {
    single: { title: 'Single color', detail: 'One hue throughout the chart, with emphasis on hover.' },
    monotone: { title: 'Monotone', detail: 'One hue in a light-to-dark shade scale.' },
    multicolor: { title: 'Multicolor', detail: 'Distinct hues for categories and chart series.' }
  };

  /* The preview draws the mode's own default palette rather than restating its colours here,
     so it follows the scheme — and a palette edit — without a second copy to keep in step. */
  let colors = $derived(paletteColorsFor(chartPaletteFor(value, defaultChartThemePreferences.palettes[value]), colorScheme));
</script>

<PreferenceOption name="chart-theme" {value} title={copy[value].title} detail={copy[value].detail} {selected} {onSelect} previewHeight={112}>
  <div class="preview" aria-hidden="true" style:--rest={colors.mark} style:--strong={colors.strong}>
    <svg viewBox="0 0 160 72">
      {#if value === 'multicolor'}
        {#each [[28, 28], [46, 18], [62, 36], [78, 22], [94, 40], [110, 16]] as [cx, cy], index (index)}
          <circle {cx} {cy} r="4.5" fill={colors.series[index]} />
        {/each}
        {#each [[122, 28, 28], [132, 18, 38], [142, 36, 20]] as [x, y, height], index (index)}
          <rect {x} {y} width="8" {height} rx="1" fill={colors.series[index]} />
        {/each}
      {:else}
        <rect class="bar" x="18" y="32" width="14" height="24" rx="1" />
        <rect class="bar strong" x="36" y="18" width="14" height="38" rx="1" />
        <rect class="bar" x="54" y="26" width="14" height="30" rx="1" />
        <rect class="bar strong" x="72" y="14" width="14" height="42" rx="1" />
        <circle class="dot" cx="108" cy="30" r="4.5" />
        <circle class="dot" cx="122" cy="20" r="4.5" />
        <circle class="dot" cx="136" cy="38" r="4.5" />
        <circle class="dot strong" cx="118" cy="44" r="4.5" />
      {/if}
    </svg>
  </div>
</PreferenceOption>

<style>
  .preview { height: 100%; display: grid; place-items: center; padding: 12px 16px 0; }
  svg { width: 100%; max-width: 168px; height: 72px; }
  .bar, .dot { fill: var(--rest); }
  .strong { fill: var(--strong); }
</style>
