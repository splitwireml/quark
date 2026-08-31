<script lang="ts">
  type Props = { label: string; value: string };
  let { label, value }: Props = $props();
</script>

<div class="row">
  <span title={label}>{label}</span>
  <b>{value}</b>
</div>

<style>
  .row { display: flex; align-items: center; gap: 8px; padding: 5px 0; border-bottom: 1px solid var(--line-soft-2); font-size: 12px; }
  .row span { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; color: var(--ink-2); }
  .row b { flex: none; font-family: var(--font-mono); font-size: 11px; color: var(--muted); }
</style>
