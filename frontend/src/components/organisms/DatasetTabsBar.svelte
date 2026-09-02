<script lang="ts">
  import Button from '../atoms/Button.svelte';
  import DatasetTab from '../molecules/DatasetTab.svelte';
  import type { RowDensity } from '../../lib/types';

  type Props = {
    workspaceTab: 'data' | 'history';
    tableExpanded: boolean;
    rowDensity: RowDensity;
    historyCount: number;
    onSelectData: () => void;
    onSelectHistory: () => void;
    setRowDensity: (density: RowDensity) => void;
    onToggleExpanded: () => void;
  };
  let { workspaceTab, tableExpanded, rowDensity, historyCount, onSelectData, onSelectHistory, setRowDensity, onToggleExpanded }: Props = $props();
</script>

<nav class:expanded={tableExpanded} class="tabs" aria-label="Data and versions">
  <DatasetTab active={workspaceTab === 'data'} disabled={tableExpanded} onselect={onSelectData}>Data</DatasetTab>
  <DatasetTab active={workspaceTab === 'history'} disabled={tableExpanded} onselect={onSelectHistory}>Versions ({historyCount})</DatasetTab>
  <div class="toolbar">
    <div class="density" role="group" aria-label="Row density">
      <Button active={rowDensity === 'compact'} onclick={() => setRowDensity('compact')}>Compact</Button>
      <Button active={rowDensity === 'default'} onclick={() => setRowDensity('default')}>Default</Button>
      <Button active={rowDensity === 'comfortable'} onclick={() => setRowDensity('comfortable')}>Comfortable</Button>
    </div>
    <Button onclick={onToggleExpanded} aria-label={tableExpanded ? 'Exit expanded table view' : 'Expand table to fill viewport'}>{tableExpanded ? 'Back' : 'Expand table'}</Button>
  </div>
</nav>

<style>
  .tabs { flex: none; display: flex; align-items: center; gap: 4px; height: 40px; padding: 0 20px; border-bottom: 1px solid var(--line); overflow-x: auto; }
  .toolbar { display: flex; align-items: center; gap: 6px; margin-left: auto; flex: none; }
  .density { display: flex; gap: 4px; }
  .tabs.expanded { display: none; }
</style>
