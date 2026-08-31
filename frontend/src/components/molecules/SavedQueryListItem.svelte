<script lang="ts">
  type Props = { name: string; active?: boolean; onselect: () => void };
  let { name, active = false, onselect }: Props = $props();
</script>

<button class="item" class:active onclick={onselect} title={name}>{name}</button>

<style>
  .item {
    display: block;
    width: 100%;
    text-align: left;
    padding: 5px 8px;
    border-radius: var(--radius-md);
    background: transparent;
    border: none;
    font-size: 11.5px;
    color: var(--ink-2);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .item:hover { background: var(--surface-hover); }
  .item.active { background: var(--surface-hover); color: var(--ink); font-weight: 500; }
</style>
