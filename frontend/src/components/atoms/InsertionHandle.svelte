<script lang="ts">
  type Props = { left: string; right: string | null; disabled?: boolean; onclick: (event: MouseEvent) => void };
  let { left, right, disabled = false, onclick }: Props = $props();
  let label = $derived(right ? `Insert column between ${left} and ${right}` : `Insert column after ${left}`);
</script>

<button type="button" class="insertion-handle" {disabled} {onclick} aria-label={label} title={label}>
  <span aria-hidden="true"></span>
</button>

<style>
  .insertion-handle { position: absolute; inset: 0 50% 0 auto; z-index: 5; width: 12px; padding: 0; transform: translateX(50%); border: 0; background: transparent; }
  .insertion-handle span { position: absolute; inset: 0 auto 0 5px; width: 2px; background: color-mix(in srgb, var(--success) 35%, transparent); }
  .insertion-handle:hover span, .insertion-handle:focus-visible span { background: var(--success); }
  .insertion-handle:disabled span { opacity: 0.35; }
</style>
