<script lang="ts">
  import Icon from './Icon.svelte';

  type Props = {
    active?: boolean;
    label: string;
    glyph?: string;
    icon?: 'upload' | 'database' | 'chevron' | 'file' | 'link' | 'plus' | 'arrow-right' | 'download' | 'grid' | 'list';
    [key: string]: unknown;
  };

  let { active = false, label, glyph, icon, ...rest }: Props = $props();
</script>

<button class="icon-btn" class:active aria-label={label} data-tip={label} {...rest}>
  {#if icon}<Icon name={icon} size={13} />{:else}{glyph}{/if}
</button>

<style>
  .icon-btn {
    position: relative;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 20px;
    height: 20px;
    flex: none;
    border-radius: var(--radius-sm);
    border: 1px solid transparent;
    background: transparent;
    color: var(--muted);
    font-size: 12px;
    line-height: 1;
  }
  .icon-btn:hover:not(:disabled) { background: var(--surface); border-color: var(--control-border); color: var(--ink); }
  .icon-btn:disabled { opacity: 0.35; }
  .icon-btn.active { color: var(--action); }

  /* Tooltip grows downward out of the button itself. */
  .icon-btn::after {
    content: attr(data-tip);
    position: absolute;
    top: calc(100% + 4px);
    left: 50%;
    z-index: 20;
    padding: 3px 6px;
    border-radius: var(--radius-sm);
    background: var(--ink);
    color: var(--surface);
    font-size: 10.5px;
    line-height: 1.3;
    white-space: nowrap;
    pointer-events: none;
    opacity: 0;
    transform: translate(-50%, -2px) scale(0.96);
    transform-origin: top center;
    transition: opacity 120ms ease, transform 120ms ease;
    transition-delay: 0s;
  }
  .icon-btn[aria-expanded='true']::after { content: none; }
  .icon-btn:hover::after,
  .icon-btn:focus-visible::after {
    opacity: 1;
    transform: translate(-50%, 0) scale(1);
    transition-delay: 260ms;
  }
  @media (prefers-reduced-motion: reduce) {
    .icon-btn::after { transition: none; transform: translate(-50%, 0); }
    .icon-btn[aria-expanded='true']::after { content: none; }
  .icon-btn:hover::after, .icon-btn:focus-visible::after { transform: translate(-50%, 0); }
  }
</style>
