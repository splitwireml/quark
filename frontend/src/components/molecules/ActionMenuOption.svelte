<script lang="ts">
  import Icon from '../atoms/Icon.svelte';
  import PreferenceOption from './PreferenceOption.svelte';
  import { operationsFor, type ActionMenuMode } from '../../lib/commands';
  let { value, selected, onSelect }: { value: ActionMenuMode; selected: boolean; onSelect: () => void } = $props();
  const simple = $derived(value === 'simple');
</script>

<PreferenceOption name="action-menu" {value} {selected} {onSelect} previewHeight={184}
  title={simple ? 'Simple' : 'Comprehensive'}
  detail={simple ? 'Aggregate, Join, Columns, and Dedupe, with small sizing controls.' : 'The full wheel, including Find column and SQL.'}>
  <div class="preview-content">
    <div class="mini-menu" class:simple class:radial={!simple}>
      {#each operationsFor(value) as operation, index (operation.id)}
        <div class="operation" class:primary={index === 0} class:utility={simple && !operation.direction} data-operation={operation.id}
          style={!simple ? `--x: ${50 + 37 * Math.sin(index * Math.PI / 4)}%; --y: ${50 - 37 * Math.cos(index * Math.PI / 4)}%` : undefined}>
          <Icon name={operation.icon} size={12} />
          <span>{operation.label}</span><kbd>{simple && operation.direction ? operation.direction : operation.key.toUpperCase()}</kbd>
        </div>
      {/each}
      {#if !simple}<span class="center">Actions</span>{/if}
    </div>
  </div>
</PreferenceOption>

<style>
  .preview-content { height: 100%; display: grid; place-items: center; padding: 10px; }
  .mini-menu { width: 222px; max-width: 100%; height: 162px; border: 1px solid var(--glass-line-strong); border-radius: var(--radius-lg); background: linear-gradient(135deg, var(--glass-fill), var(--glass-fill-soft)); box-shadow: 0 5px 14px -5px rgb(24 42 72 / 18%); color: var(--ink-2); }
  .operation { display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 3px; border: 1px solid transparent; border-radius: 5px; }
  .operation span { font-size: 8px; line-height: 1.2; white-space: nowrap; }
  kbd { font: 7px var(--font-mono); color: var(--muted); }
  .primary { background: var(--glass-active); border-color: var(--glass-line-strong); color: var(--action-dark); transition: box-shadow 160ms ease; }
  :global(label:hover) .primary, :global(label:focus-within) .primary { box-shadow: 0 0 0 1px var(--action); }
  .simple { display: grid; grid-template-columns: repeat(6, minmax(0, 1fr)); grid-template-rows: 53px 53px auto; gap: 4px; padding: 9px; }
  .simple .operation { grid-column: span 2; }
  .simple .operation[data-operation='aggregate'] { grid-column: 3 / span 2; }
  .simple .operation[data-operation='joins'], .simple .operation[data-operation='columns'], .simple .operation[data-operation='dedupe'] { grid-row: 2; background: var(--glass-tile); border-color: var(--glass-line); }
  .simple .operation.utility { grid-column: span 3; justify-self: start; flex-direction: row; gap: 3px; }
  .simple .operation[data-operation='density'] { justify-self: end; }
  .utility span { font-size: 6px; }
  .utility :global(svg) { width: 8px; height: 8px; }
  .radial { position: relative; }
  .radial .operation { position: absolute; left: var(--x); top: var(--y); transform: translate(-50%, -50%); width: 58px; height: 40px; gap: 2px; }
  .radial .operation span { font-size: 7px; }
  .center { position: absolute; left: 50%; top: 50%; transform: translate(-50%, -50%); font-size: 10px; font-weight: 500; }
  @media (prefers-reduced-motion: reduce) { .primary { transition: none; } }
</style>
