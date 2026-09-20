<script lang="ts">
  import { onMount } from 'svelte';
  import Button from '../atoms/Button.svelte';
  import Icon from '../atoms/Icon.svelte';
  import ToolbarVisibilityOption from '../molecules/ToolbarVisibilityOption.svelte';
  import ActionMenuOption from '../molecules/ActionMenuOption.svelte';
  import ChartThemeOption from '../molecules/ChartThemeOption.svelte';
  import ThemeOption from '../molecules/ThemeOption.svelte';
  import ChartPaletteOption from '../molecules/ChartPaletteOption.svelte';
  import { paletteOptions, type ChartPalette, type ChartTheme } from '../../lib/chartThemes';
  import { themePreferences, type ColorScheme, type ThemePreference } from '../../lib/theme';
  import type { ActionMenuMode, ToolbarVisibility } from '../../lib/commands';
  let { visibility, actionMenuMode, chartTheme, chartPalette, themePreference, colorScheme, error, onActionMenuMode, onVisibility, onThemePreference, onChartTheme, onChartPalette, onClose }: {
    visibility: ToolbarVisibility; actionMenuMode: ActionMenuMode; chartTheme: ChartTheme; chartPalette: ChartPalette;
    themePreference: ThemePreference; colorScheme: ColorScheme; error: string;
    onActionMenuMode: (value: ActionMenuMode) => void; onVisibility: (value: ToolbarVisibility) => void;
    onThemePreference: (value: ThemePreference) => void;
    onChartTheme: (value: ChartTheme) => void; onChartPalette: (value: ChartPalette) => void; onClose: () => void;
  } = $props();
  const themes: ChartTheme[] = ['single', 'monotone', 'multicolor'];
  let palettes = $derived(paletteOptions(chartTheme));
  let heading: HTMLHeadingElement;
  onMount(() => heading.focus());
  const choices: { value: ToolbarVisibility; title: string; detail: string }[] = [
    { value: 'show', title: 'Always show', detail: 'Keep operations, sizing, and column search within reach.' },
    { value: 'hover', title: 'Show on hover', detail: 'Reveal from the toolbar edge on hover or keyboard focus.' },
    { value: 'hide', title: 'Always hide', detail: 'More room for your data. Open operations with Actions or the keyboard.' },
  ];
</script>
<section class="settings" aria-label="Settings">
  <header><Button variant="ghost" onclick={onClose}>← Back to View</Button><span>Workspace preferences</span></header>
  <div class="heading"><h1 bind:this={heading} tabindex="-1">Settings</h1><p>Choose how your workspace looks and behaves.</p></div>
  <h2><Icon name="grid" size={15} />Appearance</h2>
  <fieldset class="theme-setting"><legend>Color scheme</legend><p class="description">Match this computer, or hold one scheme whatever the system does.</p>
    <div class="options">{#each themePreferences as preference (preference)}
      <ThemeOption value={preference} selected={themePreference === preference} onSelect={() => onThemePreference(preference)} />
    {/each}</div>
  </fieldset>
  <fieldset class="toolbar-setting"><legend>Toolbar visibility</legend><p class="description">Choose how your tools fit into the View.</p>
    <div class="options">{#each choices as choice (choice.value)}
      <ToolbarVisibilityOption {...choice} selected={visibility === choice.value} onSelect={() => onVisibility(choice.value)} />
    {/each}</div>
  </fieldset>
  <fieldset class="chart-theme-setting"><legend>Chart colors</legend><p class="description">Choose one hue, a shade scale, or distinct hues for bars, boxes, and scatter marks.</p>
    <div class="options">{#each themes as theme (theme)}
      <ChartThemeOption value={theme} selected={chartTheme === theme} onSelect={() => onChartTheme(theme)} />
    {/each}</div>
    <div class="palette-heading"><strong>Palette</strong><span>Choose the colors used by this mode.</span></div>
    <div class="palette-options">{#each palettes as palette (palette.id)}
      <ChartPaletteOption {palette} {colorScheme} selected={chartPalette === palette.id} onSelect={() => onChartPalette(palette.id)} />
    {/each}</div>
  </fieldset>
  <fieldset class="action-menu-setting"><legend>Action menu</legend><p class="description">Choose what appears when you press ⌘A.</p>
    <div class="menu-options">
      <ActionMenuOption value="simple" selected={actionMenuMode === 'simple'} onSelect={() => onActionMenuMode('simple')} />
      <ActionMenuOption value="comprehensive" selected={actionMenuMode === 'comprehensive'} onSelect={() => onActionMenuMode('comprehensive')} />
    </div>
  </fieldset>
  <div class="keyboard-note"><span class="key"><kbd>⌘</kbd><kbd>A</kbd></span><p><strong>Your tools are always one shortcut away.</strong><br />Open the action wheel in any mode. Use Ctrl+A on Windows / Linux, outside text fields.</p></div>
  <footer><span>Filters, sorts, and Version state stay visible in every mode.</span>{#if !error}<span>Saved on this device</span>{/if}</footer>
  {#if error}<p class="error" role="alert">{error}</p>{/if}
</section>
<style>
  .settings { flex: 1; min-height: 0; width: 100%; max-width: 960px; overflow-y: auto; margin: 0 auto; padding: 20px 36px 40px; color: var(--ink); }
  header { display: flex; align-items: center; justify-content: space-between; gap: 24px; }
  header > span { font-size: 11px; color: var(--muted); }
  header :global(.btn) { margin-left: -12px; }
  .heading { margin: 32px 0 40px; }
  h1 { font-size: 30px; font-weight: 500; letter-spacing: -.8px; margin: 0 0 8px; }
  .heading p { font-size: 14px; }
  h2 { display: flex; align-items: center; gap: 8px; margin: 0 0 26px; padding-bottom: 14px; border-bottom: 1px solid var(--line); font-size: 13px; font-weight: 500; }
  fieldset { border: 0; padding: 0; margin: 0; }
  legend { font-size: 15px; font-weight: 500; margin-bottom: 5px; }
  p { margin: 0; font-size: 12px; line-height: 1.6; color: var(--muted); }
  .description { margin-bottom: 20px; }
  .options { display: grid; grid-template-columns: repeat(3, minmax(0, 1fr)); gap: 14px; }
  .toolbar-setting, .chart-theme-setting, .action-menu-setting { margin-top: 30px; }
  .palette-heading { display: flex; align-items: baseline; gap: 10px; margin: 24px 0 12px; }
  .palette-heading strong { font-size: 13px; font-weight: 500; color: var(--ink); }
  .palette-heading span { font-size: 12px; color: var(--muted); }
  .palette-options { display: grid; grid-template-columns: repeat(4, minmax(0, 1fr)); gap: 10px; }
  .menu-options { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 14px; }
  .keyboard-note { display: flex; align-items: center; gap: 18px; margin: 28px 0; padding: 18px 0; border-bottom: 1px solid var(--line); }
  .key { display: flex; gap: 4px; }
  kbd { display: grid; place-items: center; width: 29px; height: 30px; border: 1px solid var(--line-strong); border-bottom-width: 2px; border-radius: var(--radius-md); background: var(--surface); color: var(--ink-2); font: 13px var(--font-mono); }
  strong { color: var(--ink-2); font-weight: 500; }
  footer { display: flex; justify-content: space-between; flex-wrap: wrap; gap: 8px 24px; font-size: 11px; line-height: 1.5; color: var(--muted); }
  .error { margin-top: 16px; color: var(--error); }
  @media (max-width: 760px) {
    .options, .palette-options, .menu-options { grid-template-columns: repeat(2, minmax(0, 1fr)); }
  }
</style>
