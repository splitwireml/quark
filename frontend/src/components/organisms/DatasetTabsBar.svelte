<script lang="ts">
  import Button from '../atoms/Button.svelte';
  import DatasetTab from '../molecules/DatasetTab.svelte';
  import type { DatasetInfo, RowDensity } from '../../lib/types';

  type Props = {
    datasets: DatasetInfo[];
    selectedDataset: string;
    workspaceTab: 'data' | 'queries';
    tableExpanded: boolean;
    rowDensity: RowDensity;
    savedQueryCount: number;
    onSelectDataset: (id: string) => void;
    onSelectQueries: () => void;
    setRowDensity: (density: RowDensity) => void;
    onToggleExpanded: () => void;
  };
  let { datasets, selectedDataset, workspaceTab, tableExpanded, rowDensity, savedQueryCount, onSelectDataset, onSelectQueries, setRowDensity, onToggleExpanded }: Props = $props();
</script>

<nav class:expanded={tableExpanded} class="tabs" aria-label="Datasets and saved queries">
  {#each datasets as dataset (dataset.id)}
    <DatasetTab active={workspaceTab === 'data' && dataset.id === selectedDataset} disabled={tableExpanded} onselect={() => onSelectDataset(dataset.id)}>{dataset.name}</DatasetTab>
  {/each}
  <DatasetTab active={workspaceTab === 'queries'} disabled={tableExpanded} onselect={onSelectQueries}>Queries ({savedQueryCount})</DatasetTab>
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
  .tabs {
    flex: none;
    display: flex;
    align-items: center;
    gap: 4px;
    height: 40px;
    padding: 0 20px;
    border-bottom: 1px solid var(--line);
    overflow-x: auto;
  }
  .toolbar { display: flex; align-items: center; gap: 6px; margin-left: auto; flex: none; }
  .density { display: flex; gap: 4px; }
  .tabs.expanded { display: none; }
</style>
