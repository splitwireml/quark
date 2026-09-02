<script lang="ts">
  type Props = {
    name: string;
    sourceName?: string;
    versionLabel: string;
    active: boolean;
    onselect: () => void;
  };

  let { name, sourceName, versionLabel, active, onselect }: Props = $props();
</script>

<button type="button" class="row" class:active aria-current={active ? 'page' : undefined} onclick={onselect}>
  <span class="details">
    <span class="name" title={name}>{name}</span>
    {#if sourceName}<small title={sourceName}>{sourceName}</small>{/if}
  </span>
  <span class="version" aria-label={`Version ${versionLabel}`}>{versionLabel}</span>
</button>

<style>
  .row {
    width: 100%;
    min-width: 0;
    min-height: 32px;
    padding: 5px 8px;
    display: flex;
    align-items: center;
    gap: 8px;
    border: none;
    border-radius: var(--radius-md);
    background: transparent;
    text-align: left;
  }
  .row:hover, .row.active { background: var(--surface-hover); }
  .details { flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 1px; }
  .name, small { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .name { font: 11px var(--font-mono); color: var(--ink); }
  small { font-size: 10px; color: var(--muted-2); }
  .version { flex: none; margin-left: auto; padding: 1px 5px; border: 1px solid var(--line); border-radius: var(--radius-sm); background: var(--surface); font: 9px var(--font-mono); color: var(--muted); }
  .active .version { border-color: var(--action-tint-border); color: var(--action-dark); }
</style>
