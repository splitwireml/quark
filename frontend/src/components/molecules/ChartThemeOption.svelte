<script lang="ts">
  import PreferenceOption from './PreferenceOption.svelte';
  import type { ChartTheme } from '../../lib/commands';

  let { value, selected, onSelect }: { value: ChartTheme; selected: boolean; onSelect: () => void } = $props();

  const copy: Record<ChartTheme, { title: string; detail: string }> = {
    primary: { title: 'Primary', detail: 'The workspace accent. One blue, used with weight rather than extra hues.' },
    monotone: { title: 'Monotone', detail: 'Ink on paper. Marks, fills, and emphasis stay in the grey scale.' },
    rich: { title: 'Rich', detail: 'A fuller palette when a chart carries more than one series or encoding.' }
  };
</script>

<PreferenceOption name="chart-theme" {value} title={copy[value].title} detail={copy[value].detail} {selected} {onSelect} previewHeight={112}>
  <div class="preview" data-theme={value} aria-hidden="true">
    <svg viewBox="0 0 160 72">
      {#if value === 'rich'}
        <circle class="s1" cx="28" cy="28" r="4.5" />
        <circle class="s2" cx="46" cy="18" r="4.5" />
        <circle class="s3" cx="62" cy="36" r="4.5" />
        <circle class="s4" cx="78" cy="22" r="4.5" />
        <circle class="s5" cx="94" cy="40" r="4.5" />
        <circle class="s6" cx="110" cy="16" r="4.5" />
        <rect class="s1" x="122" y="28" width="8" height="28" rx="1" />
        <rect class="s2" x="132" y="18" width="8" height="38" rx="1" />
        <rect class="s3" x="142" y="36" width="8" height="20" rx="1" />
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
  .preview[data-theme='primary'] .bar, .preview[data-theme='primary'] .dot { fill: #C9DBFF; }
  .preview[data-theme='primary'] .strong { fill: #1155F5; }
  .preview[data-theme='monotone'] .bar, .preview[data-theme='monotone'] .dot { fill: #C5CDD8; }
  .preview[data-theme='monotone'] .strong { fill: #1F2533; }
  .s1 { fill: #1155F5; }
  .s2 { fill: #0F9D8A; }
  .s3 { fill: #C45C16; }
  .s4 { fill: #6B4CE6; }
  .s5 { fill: #C43B5C; }
  .s6 { fill: #2A8F3A; }
</style>
