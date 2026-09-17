<script lang="ts">
  type Props = {
    on: boolean;
    label: string;
    /** Trailing mono badge: a count, or a pair index. */
    badge?: string | number;
    disabled?: boolean;
    onclick: () => void;
    [key: string]: unknown;
  };

  let { on, label, badge, disabled = false, onclick, ...rest }: Props = $props();
</script>

<button type="button" class="chip" class:on {disabled} aria-pressed={on} {onclick} {...rest}>
  <span class="check" class:empty={!on}>{#if on}✓{/if}</span>
  <span class="label">{label}</span>
  {#if badge !== undefined}<b>{badge}</b>{/if}
</button>

<style>
  .chip {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    max-width: 100%;
    height: 24px;
    padding: 0 8px;
    border-radius: var(--radius-sm);
    border: 1px solid var(--control-border);
    background: var(--surface);
    font-family: var(--font-mono);
    font-size: 10.5px;
    color: var(--muted);
    transition: background 120ms ease, border-color 120ms ease, color 120ms ease;
  }
  .chip:hover:not(:disabled) { border-color: var(--faint); }
  .chip.on { border-color: var(--action-tint-border); background: var(--action-tint); color: var(--action-dark); }
  .chip:disabled { opacity: 0.5; }
  .label { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .check {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    flex: none;
    width: 11px; height: 11px;
    border-radius: 2px;
    font-size: 7.5px;
  }
  .chip.on .check { background: var(--action); color: #FFFFFF; }
  .check.empty { border: 1px solid var(--control-border); }
</style>
