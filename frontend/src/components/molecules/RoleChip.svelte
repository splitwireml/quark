<script lang="ts">
  import type { EncodingRole } from '../../lib/types';

  type Props = {
    name: string;
    role: EncodingRole | null;
    roles: EncodingRole[];
    onSetRole: (role: EncodingRole) => void;
    onRemove: () => void;
  };
  let { name, role, roles, onSetRole, onRemove }: Props = $props();
  let open = $state(false);

  const labels: Record<EncodingRole, string> = {
    x: 'X axis', y: 'Y axis', category: 'Category', value: 'Value',
    group: 'Group', size: 'Size', color: 'Color', pattern: 'Shape'
  };

  function pick(next: EncodingRole) {
    onSetRole(next);
    open = false;
  }
</script>

<div class="role-chip" class:open>
  <div class="head">
    <button
      type="button"
      class="label"
      aria-expanded={open}
      title={name}
      onclick={() => open = !open}
    >
      <span class="name">{name}</span>
      <span class="role">{role ? labels[role] : 'unused'}</span>
    </button>
    <button type="button" class="remove" aria-label={`Remove ${name}`} onclick={onRemove}>×</button>
  </div>
  <div class="roles" inert={!open}>
    <div class="roles-inner">
      {#each roles as option (option)}
        <button
          type="button"
          class="role-option"
          class:on={role === option}
          aria-pressed={role === option}
          onclick={() => pick(option)}
        >{labels[option]}</button>
      {/each}
    </div>
  </div>
</div>

<style>
  .role-chip {
    display: grid;
    grid-template-rows: auto 0fr;
    width: 100%;
    border: 1px solid var(--action-tint-border);
    border-radius: var(--radius-sm);
    background: var(--action-tint);
    color: var(--action-dark);
    overflow: hidden;
  }
  .role-chip.open { grid-template-rows: auto 1fr; }
  .head { display: flex; align-items: center; gap: 4px; padding: 0 4px 0 8px; }
  .label {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: baseline;
    gap: 6px;
    height: 24px;
    padding: 0;
    border: 0;
    background: transparent;
    color: inherit;
    text-align: left;
    font-family: var(--font-mono);
    font-size: 11px;
  }
  .name { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .role { flex: none; font-size: 10px; opacity: .7; }
  .remove {
    flex: none;
    width: 18px;
    height: 18px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    border: 0;
    border-radius: 2px;
    background: transparent;
    color: inherit;
    font-size: 12px;
    line-height: 1;
    padding: 0;
    opacity: .55;
  }
  .remove:hover { opacity: 1; background: color-mix(in srgb, currentColor 8%, transparent); }
  .roles { min-height: 0; overflow: hidden; }
  .roles-inner { display: flex; flex-wrap: wrap; gap: 4px; padding: 4px 8px 6px; }
  .role-option {
    height: 22px;
    padding: 0 7px;
    border: 1px solid var(--control-border);
    border-radius: var(--radius-sm);
    background: var(--surface);
    color: var(--ink-2);
    font-family: var(--font-mono);
    font-size: 10.5px;
  }
  .role-option.on { border-color: var(--ink-fill); background: var(--ink-fill); color: var(--on-fill); }
  .role-option:hover:not(.on) { border-color: var(--faint); color: var(--ink); }

  @media (prefers-reduced-motion: no-preference) {
    .role-chip { transition: grid-template-rows 200ms cubic-bezier(0.22, 1, 0.36, 1); }
  }
</style>
