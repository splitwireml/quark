<script lang="ts">
  import Icon from '../atoms/Icon.svelte';
  import { chartMeta } from '../../lib/visualize';
  import type { ChartSuggestion, ChartType } from '../../lib/types';

  type Props = {
    suggestions: ChartSuggestion[];
    selected: ChartType | null;
    onSelect: (chart: ChartType) => void;
  };
  let { suggestions, selected, onSelect }: Props = $props();
</script>

{#if suggestions.length}
  <div class="types" role="group" aria-label="Chart type">
    {#each suggestions as suggestion (suggestion.chart)}
      {@const meta = chartMeta[suggestion.chart]}
      <button
        type="button"
        class="menu-trigger"
        class:active={selected === suggestion.chart}
        aria-pressed={selected === suggestion.chart}
        data-tip={meta.tip}
        data-tip-position="top"
        onclick={() => onSelect(suggestion.chart)}
      >
        <Icon name={meta.icon} size={14} />
        <span class="menu-label"><span>{meta.label}</span></span>
      </button>
    {/each}
  </div>
{/if}

<style>
  .types { display: flex; flex-wrap: wrap; gap: 6px; }
</style>
