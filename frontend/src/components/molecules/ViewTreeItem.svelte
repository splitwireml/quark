<script lang="ts">
  type Props = {
    name: string;
    sourceName?: string;
    versionLabel: string;
    active: boolean;
    pickable?: boolean;
    selectionMark?: string;
    selectLabel?: string;
    onselect: () => void;
  };

  let { name, sourceName, versionLabel, active, pickable = false, selectionMark = '', selectLabel, onselect }: Props = $props();
</script>

<button
  type="button" class="row" class:active class:pickable class:picked={!!selectionMark}
  aria-label={selectLabel} aria-current={!pickable && active ? 'page' : undefined}
  aria-pressed={pickable ? !!selectionMark : undefined} onclick={onselect}
>
  <span class="details">
    <span class="name" title={name}>{name}</span>
    {#if sourceName}<small title={sourceName}>{sourceName}</small>{/if}
  </span>
  {#if selectionMark}<span class="selection" aria-hidden="true">{selectionMark}</span>{/if}
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
  .row.pickable { box-shadow: inset 0 0 0 1px transparent; transition: background 120ms ease, box-shadow 120ms ease; }
  .row.pickable:hover, .row.pickable:focus-visible { background: var(--action-tint); box-shadow: inset 0 0 0 1px var(--action-tint-border); }
  .row.picked { background: var(--action-tint); color: var(--action-dark); }
  .details { flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 1px; }
  .name, small { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .name { font: 11px var(--font-mono); color: var(--ink); }
  small { font-size: 10px; color: var(--faint); }
  .selection { flex: none; min-width: 18px; text-align: center; font: 500 9px var(--font-mono); color: var(--action-dark); }
  .version { flex: none; margin-left: auto; padding: 1px 5px; border: 1px solid var(--line); border-radius: var(--radius-sm); background: var(--surface); font: 9px var(--font-mono); color: var(--muted); }
  .active .version { border-color: var(--action-tint-border); color: var(--action-dark); }
</style>
