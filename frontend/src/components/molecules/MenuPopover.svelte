<script lang="ts">
  import type { Snippet } from 'svelte';
  import Icon from '../atoms/Icon.svelte';
  import type { IconName } from '../../lib/icons';

  type Props = {
    open: boolean;
    ontoggle: (event: Event) => void;
    icon: IconName;
    label: string;
    hint?: string;
    /** Panel width in px; clamped so the panel stays inside the window. */
    width?: number;
    /** Body supplies its own edge padding (full-bleed rows, section rules). */
    flush?: boolean;
    /** Title bar: a <strong> name and one trailing mono count. */
    header?: Snippet;
    /** Action bar pinned under the scrolling body. */
    footer?: Snippet;
    children: Snippet;
  };

  let { open, ontoggle, icon, label, hint, width = 300, flush = false, header, footer, children }: Props = $props();

  let panel = $state<HTMLElement>();
  let viewportWidth = $state(0);

  // Anchored under its own trigger; slid back in only when the right edge would
  // leave the window, so the panel keeps the button's left edge when it can.
  $effect(() => {
    void viewportWidth;
    if (!open || !panel) return;
    // offsetWidth/offsetLeft ignore the open animation's transform, so the
    // measurement is stable while the panel is still growing in.
    const host = panel.parentElement as HTMLElement;
    const right = host.getBoundingClientRect().left + panel.offsetLeft + panel.offsetWidth;
    const overflow = right - (window.innerWidth - 12);
    panel.style.translate = overflow > 0 ? `${-overflow}px` : '';
  });
</script>

<svelte:window bind:innerWidth={viewportWidth} />

<details class="popover-host" {open} {ontoggle}>
  <summary class="trigger menu-trigger" class:active={open}>
    <Icon name={icon} size={14} />
    <span class="menu-label"><span>{label}{#if hint}<small>{hint}</small>{/if}</span></span>
  </summary>
  <div bind:this={panel} class="popover" style="--menu-width: {width}px">
    {#if header}<div class="menu-header">{@render header()}</div>{/if}
    <div class="menu-body" class:flush>{@render children()}</div>
    {#if footer}<div class="menu-footer">{@render footer()}</div>{/if}
  </div>
</details>

<style>
  .popover-host { position: relative; }
  summary { list-style: none; cursor: pointer; }
  summary::-webkit-details-marker { display: none; }

  /* Every operation menu opens directly under its own trigger, same offset,
     same growth, so the four read as one control surface. */
  .popover {
    position: absolute;
    top: calc(100% + 6px);
    left: 0;
    z-index: 10;
    display: flex;
    flex-direction: column;
    width: min(var(--menu-width), calc(100vw - 24px));
    max-height: min(680px, calc(100vh - 140px));
    overflow: var(--menu-overflow, hidden);
    border-radius: var(--radius-xl);
    background: var(--surface);
    border: 1px solid var(--line-strong);
    box-shadow: var(--shadow-popover-wide);
    transform-origin: top left;
    animation: menu-in 200ms cubic-bezier(0.32, 0.72, 0, 1);
  }

  .menu-header, .menu-footer {
    display: flex;
    align-items: center;
    gap: 8px;
    flex: none;
    min-height: 32px;
    padding: 0 11px;
    background: var(--surface-2);
  }
  .menu-header { border-bottom: 1px solid var(--line-soft); }
  .menu-footer { border-top: 1px solid var(--line-soft); padding: 6px 11px; }
  .menu-header :global(strong) { font-size: 11.5px; }
  .menu-header :global(strong + *) { margin-left: auto; }
  .menu-header :global(span) { font-family: var(--font-mono); font-size: 10px; color: var(--faint); }

  .menu-body { min-height: 0; overflow: var(--menu-body-overflow, auto); }
  .menu-body:not(.flush) {
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 12px;
  }

  @keyframes menu-in {
    from { opacity: 0; transform: translateY(-6px) scale(0.97); }
    to { opacity: 1; transform: none; }
  }
  @media (prefers-reduced-motion: reduce) {
    .popover { animation: none; }
  }
</style>
