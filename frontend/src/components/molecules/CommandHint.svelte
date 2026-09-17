<script lang="ts">
  import { combinationTimeoutMs, type CommandPrefix } from '../../lib/commands';
  let { prefix, sourcePicking, sourceDigits, sourceCount }: {
    prefix: CommandPrefix; sourcePicking: boolean; sourceDigits: string; sourceCount: number;
  } = $props();
  const choices = $derived(sourcePicking ? [[sourceDigits || (sourceCount ? `1–${sourceCount}` : '0'), sourceCount ? 'Choose source' : 'No sources']]
    : prefix === 'find' ? [['C', 'Column'], ['F', 'Cell value']]
    : prefix === 'column' ? [['F', 'Filter'], ['S', 'Sort'], ['H', 'Hide'], ['P', 'Pin']]
    : [['B', 'Toggle'], ['T', 'Sources']]);
</script>

<aside class="command-hint" role="status" aria-label="Next command" style="--timeout: {combinationTimeoutMs}ms">
  <div class="heading"><strong>{sourcePicking ? 'Sources' : prefix === 'find' ? 'Find' : prefix === 'column' ? 'Column' : 'Sidebar'}</strong><span><kbd>esc</kbd> cancel</span></div>
  <div class="choices">{#each choices as [key, label] (key)}<span><kbd>{key}</kbd>{label}</span>{/each}</div>
  <div class="timer" role="img" aria-label="Combination expires after 3 seconds"></div>
</aside>

<style>
  .command-hint { position: fixed; right: 18px; bottom: 18px; z-index: 30; max-width: calc(100vw - 36px); padding: 10px 12px 12px; overflow: hidden; border: 1px solid var(--line-strong); border-radius: var(--radius-lg); color: var(--ink-2); background: var(--surface); box-shadow: var(--shadow-popover); pointer-events: none; }
  .heading { display: flex; align-items: center; justify-content: space-between; gap: 32px; margin-bottom: 8px; font-size: 10px; }
  strong { font-size: 11px; font-weight: 500; color: var(--ink); }
  .heading span { color: var(--muted); }
  .heading kbd { font-size: 9px; }
  .choices { display: flex; flex-wrap: wrap; gap: 10px; font-size: 11px; }
  .choices span { display: inline-flex; align-items: center; gap: 5px; }
  kbd { font-family: var(--font-mono); }
  .choices kbd { display: inline-flex; align-items: center; justify-content: center; min-width: 18px; height: 19px; padding: 0 3px; border: 1px solid var(--line); border-radius: var(--radius-sm); background: var(--surface-2); font-size: 10px; color: var(--ink); }
  .timer { position: absolute; bottom: 0; left: 0; width: 100%; height: 2px; background: var(--action); transform-origin: left; animation: countdown var(--timeout) linear forwards; }
  @keyframes countdown { to { transform: scaleX(0); } }
  @media (prefers-reduced-motion: reduce) { .timer { animation-timing-function: steps(3, end); } }
</style>
