<script lang="ts">
  type Props = { tone?: 'success' | 'warning' | 'error' | 'muted' };
  let { tone = 'success' }: Props = $props();
</script>

<span class="dot {tone}" aria-hidden="true"></span>

<style>
  .dot { display: inline-block; width: 6px; height: 6px; border-radius: 50%; flex: none; }
  .success { background: var(--success); }
  .warning { background: var(--warning-fill); }
  .error { background: var(--error); }
  .muted { background: var(--faint); }
</style>
