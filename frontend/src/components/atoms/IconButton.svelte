<script lang="ts">
  import Icon from './Icon.svelte';
  import type { IconName } from '../../lib/icons';

  type Props = {
    active?: boolean;
    label: string;
    glyph?: string;
    icon?: IconName;
    size?: 'sm' | 'md';
    [key: string]: unknown;
  };

  let { active = false, label, glyph, icon, size = 'sm', ...rest }: Props = $props();
</script>

<button class="icon-btn {size}" class:active aria-label={label} data-tip={label} {...rest}>
  {#if icon}<Icon name={icon} size={size === 'md' ? 15 : 13} />{:else}{glyph}{/if}
</button>

<style>
  .icon-btn {
    position: relative;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    flex: none;
    border-radius: var(--radius-sm);
    border: 1px solid transparent;
    background: transparent;
    color: var(--muted);
    font-size: 12px;
    line-height: 1;
  }
  .icon-btn.sm { width: 20px; height: 20px; }
  .icon-btn.md { width: 30px; height: 30px; border-radius: var(--radius-lg); border-color: var(--control-border); background: var(--surface); color: var(--muted); }
  .icon-btn.sm:hover:not(:disabled) { background: var(--surface); border-color: var(--control-border); color: var(--ink); }
  .icon-btn.md:hover:not(:disabled) { border-color: var(--faint); color: var(--ink); }
  .icon-btn.md.active { border-color: var(--action); background: var(--action-tint); color: var(--action-dark); }
  .icon-btn:disabled { opacity: 0.35; }
  .icon-btn.active { color: var(--action); }

</style>
