<script lang="ts">
  import { onMount, tick } from 'svelte';
  import Button from '../atoms/Button.svelte';
  import Icon from '../atoms/Icon.svelte';
  import TextInput from '../atoms/TextInput.svelte';
  import type { IconName } from '../../lib/icons';
  let { title, wheel = false, simple = false, compact = false, searchable = false, query = '', items = [], busy = false, notice = '', onQuery, onChoose, onClose, onBack }: {
    title: string; wheel?: boolean; simple?: boolean; compact?: boolean; searchable?: boolean; query?: string;
    items?: { id: string; label: string; detail?: string; shortcut?: string; direction?: string; icon?: IconName; disabled?: boolean }[];
    busy?: boolean; notice?: string; onQuery?: (query: string) => void;
    onChoose: (id: string) => void; onClose: () => void; onBack?: () => void;
  } = $props();
  let dialog: HTMLDialogElement;
  let active = $state(0);
  let lastTitle = '';
  onMount(() => { dialog.showModal(); return () => dialog.close(); });
  $effect(() => {
    if (title !== lastTitle) {
      lastTitle = title;
      active = 0;
      void tick().then(() => (dialog?.querySelector<HTMLInputElement>('input') ?? dialog?.querySelector<HTMLButtonElement>('[data-choice]') ?? dialog?.querySelector<HTMLButtonElement>('button'))?.focus());
    }
  });
  function keydown(event: KeyboardEvent) {
    if (event.isComposing) return;
    if (event.key === 'Escape') { event.preventDefault(); event.stopPropagation(); (onBack ?? onClose)(); return; }
    if (event.metaKey || event.ctrlKey || event.altKey) return;
    if (event.key === 'Enter' && (event.target as HTMLElement).closest('button:not([data-choice])')) return;
    const delta = ['ArrowDown', 'ArrowRight'].includes(event.key) ? 1 : ['ArrowUp', 'ArrowLeft'].includes(event.key) ? -1 : 0;
    if (delta && items.length && (wheel || event.key === 'ArrowDown' || event.key === 'ArrowUp')) {
      event.preventDefault(); event.stopPropagation();
      const direction = ({ ArrowUp: '↑', ArrowLeft: '←', ArrowDown: '↓', ArrowRight: '→' } as Record<string, string>)[event.key];
      const mapped = simple ? items.findIndex((item) => item.direction === direction) : -1;
      active = mapped >= 0 ? mapped : (active + delta + items.length) % items.length;
      const choice = dialog.querySelectorAll<HTMLButtonElement>('[data-choice]')[active];
      choice?.scrollIntoView({ block: 'nearest' });
      if (!searchable) choice?.focus();
    } else if (event.key === 'Enter' && items[active] && !items[active].disabled) {
      event.preventDefault(); event.stopPropagation(); onChoose(items[active].id);
    } else if (wheel) {
      const item = items.find((item) => item.shortcut?.toLowerCase() === event.key.toLowerCase());
      if (item && !item.disabled) { event.preventDefault(); event.stopPropagation(); onChoose(item.id); }
    }
  }
</script>
<dialog bind:this={dialog} class:wheel class:simple class:compact aria-label={title} onkeydown={keydown} oncancel={(event) => { event.preventDefault(); (onBack ?? onClose)(); }}>
  {#if !compact}
  <header><div>{#if onBack}<Button variant="ghost" onclick={onBack}>← Back</Button>{/if}<h2>{title}</h2></div><Button variant="ghost" onclick={onClose}>Close <kbd>Esc</kbd></Button></header>
  {/if}
  {#if searchable}<div class="search"><TextInput size="md" glyph={compact ? '⌕' : undefined} role="combobox" aria-expanded={!compact || !!query.trim()} aria-controls="command-options" aria-activedescendant={items[active] ? `command-option-${active}` : undefined} aria-autocomplete="list" aria-label={title} placeholder={title === 'Find column' ? 'Find column' : 'Search commands…'} value={query} oninput={(event: Event) => { active = 0; onQuery?.((event.target as HTMLInputElement).value); }}>
    {#snippet trailing()}{#if compact}<Button variant="ghost" aria-label="Close column finder" onclick={onClose}><kbd>esc</kbd></Button>{/if}{/snippet}
  </TextInput></div>{/if}
    <div id="command-options" hidden={compact && !query.trim()} role={searchable ? 'listbox' : undefined} aria-label={searchable ? title : undefined} class:radial={wheel && !simple} class:simple-grid={simple} class:list={!wheel} aria-busy={busy}>
      {#each items as item, index (item.id)}
        <button data-choice data-operation={item.id} id={`command-option-${index}`} role={searchable ? 'option' : undefined} aria-selected={searchable ? index === active : undefined} type="button" class:active={index === active} class:utility={simple && (item.id === 'density' || item.id === 'fit')} disabled={item.disabled} style={wheel && !simple ? `--x: ${50 + 34 * Math.sin(index * Math.PI / 4)}%; --y: ${50 - 35 * Math.cos(index * Math.PI / 4)}%` : undefined} onfocus={() => active = index} onpointerenter={() => active = index} onclick={() => onChoose(item.id)}>
          {#if item.icon}<Icon name={item.icon} size={wheel && !simple ? 22 : 16} />{/if}
          <span><strong>{item.label}</strong>{#if item.detail}<small>{item.detail}</small>{/if}</span>
          {#if item.shortcut}<kbd>{#if simple && item.direction}<span aria-hidden="true">{item.direction}</span> · {/if}{item.shortcut}</kbd>{/if}
        </button>
      {/each}
      {#if wheel && !simple}<div class="center"><span>Actions</span><small>Arrows to choose<br />Enter to open</small></div>{/if}
      {#if !items.length}<p>{busy ? 'Searching…' : 'No matches.'}</p>{/if}
    </div>
  {#if !compact && !simple}<footer role="status">{notice || (wheel ? 'Choose an operation • letter keys open directly' : '↑ ↓ to choose · Enter to open · Escape to return')}</footer>{/if}
</dialog>
<style>
  dialog { position: fixed; inset: 0; margin: auto; z-index: 40; width: min(580px, calc(100vw - 48px)); max-height: calc(100vh - 72px); padding: 0; border: 1px solid var(--line-strong); border-radius: 16px; color: var(--ink); background: var(--surface); box-shadow: var(--shadow-panel); overflow: auto; }
  dialog::backdrop { background: rgb(25 35 50 / 16%); }
  dialog.simple { width: min(360px, calc(100vw - 48px)); border-radius: var(--radius-xl); }
  .simple header { padding: 10px 14px; }
  .simple h2 { font-size: 13px; }
  .simple-grid { display: grid; grid-template-columns: repeat(6, minmax(0, 1fr)); gap: 6px; padding: 12px; }
  .simple-grid button { grid-column: span 2; }
  .simple-grid button[data-choice]:not(.utility) { min-height: 84px; padding: 10px 6px; gap: 5px; flex-direction: column; justify-content: center; text-align: center; background: rgb(255 255 255 / 24%); border-color: rgb(255 255 255 / 40%); }
  .simple-grid button[data-operation='aggregate'] { grid-column: 3 / span 2; grid-row: 1; }
  .simple-grid button[data-operation='joins'], .simple-grid button[data-operation='columns'], .simple-grid button[data-operation='dedupe'] { grid-row: 2; }
  .simple-grid kbd { font-size: 10px; }
  .simple-grid button.utility { grid-column: span 3; justify-self: start; width: max-content; min-height: 26px; padding: 5px 7px; margin-top: 6px; gap: 6px; border-radius: var(--radius-md); }
  .simple-grid button[data-operation='density'] { justify-self: end; }
  .simple-grid button.utility kbd { margin-left: 4px; }
  .simple-grid .utility strong { font-size: 10px; font-weight: 400; }
  .simple-grid .utility :global(svg) { width: 12px; height: 12px; }
  dialog.compact { inset: 88px 24px auto auto; margin: 0; width: min(320px, calc(100vw - 48px)); border-radius: var(--radius-lg); box-shadow: var(--shadow-popover); }
  dialog.compact::backdrop { background: transparent; }
  .compact .search { padding: 6px; }
  .search :global(.field) { display: flex; height: 36px; }
  .search :global(input) { width: 100%; height: auto; outline: none; }
  .search :global(.field:focus-within) { border-color: var(--action); box-shadow: 0 0 0 1px var(--action); }
  .compact .search :global(.field) { height: 30px; }
  .compact .search :global(.btn) { height: 22px; padding: 0 4px; }
  .compact .search kbd { font-size: 10px; }
  .compact .list { max-height: 180px; padding: 0 6px 6px; }
  .compact .list button { padding: 7px 8px; gap: 6px; border-radius: var(--radius-md); }
  .compact .list button span { display: flex; justify-content: space-between; gap: 10px; width: 100%; min-width: 0; }
  .compact .list strong { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: 12px; }
  .compact .list small { flex: none; font: 9px var(--font-mono); align-self: center; }
  .compact .list p { padding: 6px 8px; margin: 0; color: var(--muted); font-size: 12px; }
  dialog.wheel {
    background: linear-gradient(135deg, rgb(255 255 255 / 52%), rgb(233 240 252 / 24%) 60%, rgb(255 255 255 / 40%));
    backdrop-filter: blur(14px) saturate(1.6);
    -webkit-backdrop-filter: blur(14px) saturate(1.6);
    border-color: rgb(255 255 255 / 76%);
    box-shadow: inset 0 1px 0 rgb(255 255 255 / 90%), 0 24px 64px -20px rgb(24 42 72 / 38%);
  }
  dialog.wheel::backdrop { background: rgb(28 42 65 / 10%); }
  .wheel header, .wheel footer { border-color: rgb(255 255 255 / 48%); }
  .wheel button[data-choice].active { background: rgb(255 255 255 / 65%); border-color: rgb(255 255 255 / 90%); box-shadow: 0 5px 14px -6px rgb(26 48 85 / 28%); color: var(--action-dark); }
  .wheel .center span { font-weight: 600; }
  .wheel small, .wheel kbd, .wheel footer { color: var(--ink-2); }
  @supports not (backdrop-filter: blur(1px)) { dialog.wheel { background: rgb(243 247 253 / 96%); } }
  @media (prefers-reduced-transparency: reduce) { dialog.wheel { background: var(--surface); backdrop-filter: none; -webkit-backdrop-filter: none; } }
  header, footer { padding: 16px 20px; }
  header { display: flex; justify-content: space-between; align-items: center; border-bottom: 1px solid var(--line); }
  header > div { display: flex; align-items: center; gap: 12px; }
  h2 { margin: 0; font-size: 16px; font-weight: 500; }
  kbd { font: 12px var(--font-mono); color: var(--muted); }
  footer { color: var(--muted); font-size: 12px; border-top: 1px solid var(--line); }
  .search { padding: 16px 20px 4px; }
  .list { max-height: 420px; overflow: auto; padding: 12px; }
  button[data-choice] { display: flex; gap: 12px; align-items: center; border: 1px solid transparent; background: transparent; color: var(--ink); text-align: left; border-radius: 10px; padding: 12px; }
  button[data-choice]:disabled { opacity: .45; }
  button[data-choice].active { border-color: var(--action); background: var(--action-tint); }
  .list button { width: 100%; }
  .list kbd { margin-left: auto; }
  strong { font-weight: 500; font-size: 13px; }
  small { display: block; color: var(--muted); font-size: 12px; line-height: 1.5; }
  .radial { position: relative; height: min(440px, 62vh); min-height: 340px; }
  .radial button { position: absolute; left: var(--x); top: var(--y); transform: translate(-50%, -50%); width: 116px; min-height: 82px; flex-direction: column; justify-content: center; gap: 5px; text-align: center; }
  .center { position: absolute; top: 50%; left: 50%; transform: translate(-50%, -50%); text-align: center; pointer-events: none; }
  .center span { display: block; margin-bottom: 8px; font-size: 18px; font-weight: 500; }
  @media (prefers-reduced-motion: no-preference) { dialog { animation: reveal 150ms cubic-bezier(.2,.8,.2,1); } @keyframes reveal { from { opacity: 0; transform: scale(.97); } to { opacity: 1; transform: scale(1); } } }
</style>
