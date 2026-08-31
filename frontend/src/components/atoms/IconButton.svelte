<script lang="ts">
  type Props = {
    active?: boolean;
    label: string;
    glyph: string;
    [key: string]: unknown;
  };

  let { active = false, label, glyph, ...rest }: Props = $props();
</script>

<button class="icon-btn" class:active aria-label={label} title={label} {...rest}>{glyph}</button>

<style>
  .icon-btn {
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
</style>
