<script lang="ts">
  type Props = { steps: string[]; current: number; onSelect: (index: number) => void };
  let { steps, current, onSelect }: Props = $props();
</script>

<ol class="rail" style:--n={steps.length} style:--i={current} aria-label="Progress">
  {#each steps as label, index (label)}
    <li>
      <button type="button" class:done={index < current} aria-current={index === current ? 'step' : undefined} disabled={index > current} onclick={() => onSelect(index)}>
        <span class="num" aria-hidden="true">{index < current ? '✓' : index + 1}</span>{label}
      </button>
    </li>
  {/each}
</ol>

<style>
  .rail { position: relative; display: grid; grid-template-columns: repeat(var(--n), minmax(0, 1fr)); margin: 0; padding: 0; list-style: none; border-bottom: 1px solid var(--line); }
  .rail::after { content: ''; position: absolute; left: 0; bottom: -1px; width: calc(100% / var(--n)); height: 2px; background: var(--action); transform: translateX(calc(var(--i) * 100%)); transition: transform 240ms cubic-bezier(0.2, 0.8, 0.2, 1); }
  button { width: 100%; display: flex; align-items: center; gap: 7px; padding: 8px 0 9px; border: 0; background: transparent; color: var(--faint); font-size: 11.5px; font-weight: 500; text-align: left; }
  button[aria-current='step'] { color: var(--ink); }
  button.done { color: var(--muted); }
  button.done:hover { color: var(--ink); }
  button:disabled { cursor: default; }
  .num { display: inline-grid; place-items: center; width: 16px; height: 16px; border: 1px solid currentColor; border-radius: var(--radius-sm); font: 500 9.5px var(--font-mono); }
  button[aria-current='step'] .num { border-color: var(--action); background: var(--action-tint); color: var(--action-dark); }
  @media (prefers-reduced-motion: reduce) { .rail::after { transition: none; } }
</style>
