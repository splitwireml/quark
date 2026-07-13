<script lang="ts">
  import { onMount, tick } from 'svelte';
  import * as api from './lib/api';
  import type { CategoryValue, ColumnInfo, ColumnStats, DatasetInfo, FilterCondition, FilterOperator, NodeInfo, QueryResponse, SortCondition } from './lib/types';

  const pageSizes = [50, 100, 250, 500, 1000];
  const baseOperators: { value: FilterOperator; label: string }[] = [
    { value: '=', label: 'equals' }, { value: '!=', label: 'not equal' },
    { value: 'is_null', label: 'is null' }, { value: 'not_null', label: 'is not null' }
  ];
  const textOperators: { value: FilterOperator; label: string }[] = [
    { value: 'contains', label: 'contains' }, { value: 'starts_with', label: 'starts with' }, { value: 'ends_with', label: 'ends with' }
  ];
  const orderedOperators: { value: FilterOperator; label: string }[] = [
    { value: '>', label: 'greater than' }, { value: '>=', label: 'at least' }, { value: '<', label: 'less than' }, { value: '<=', label: 'at most' }
  ];

  let nodes = $state<NodeInfo[]>([]);
  let datasets = $state<DatasetInfo[]>([]);
  let selectedNodeId = $state('');
  let selectedDataset = $state('');
  let result = $state.raw<QueryResponse | null>(null);
  let filters = $state<FilterCondition[]>([]);
  let sorts = $state<SortCondition[]>([]);
  let page = $state(1);
  let pageSize = $state(100);
  let pageInput = $state('1');
  let attachPath = $state('');
  let loadingNodes = $state(true);
  let loadingData = $state(false);
  let mutating = $state(false);
  let error = $state('');
  let filterColumn = $state<ColumnInfo | null>(null);
  let filterOperator = $state<FilterOperator>('=');
  let filterValue = $state('');
  let categoryValues = $state.raw<CategoryValue[]>([]);
  let categorySearch = $state('');
  let categoryTotal = $state(0);
  let categoryHasMore = $state(false);
  let selectedCategories = $state<string[]>([]);
  let categoriesLoading = $state(false);
  let categoriesError = $state('');
  let stats = $state.raw<ColumnStats | null>(null);
  let statsColumn = $state<ColumnInfo | null>(null);
  let statsLoading = $state(false);
  let statsError = $state('');
  let statsDialog = $state<HTMLDialogElement>(null!);
  let requestId = 0;
  let datasetRequestId = 0;
  let categoryRequestId = 0;

  let selectedNode = $derived(nodes.find((node) => node.id === selectedNodeId));
  let totalPages = $derived(result?.total_pages ?? 0);
  let operators = $derived.by(() => filterColumn ? [
    ...baseOperators,
    ...(!filterColumn.numeric && isTextType(filterColumn.type) ? textOperators : []),
    ...(filterColumn.numeric || isOrderedType(filterColumn.type) ? orderedOperators : [])
  ] : baseOperators);
  let maxBin = $derived(stats?.histogram.length ? Math.max(...stats.histogram.map((bin) => bin.count), 1) : 1);

  onMount(loadNodes);

  function message(reason: unknown): string {
    return reason instanceof Error ? reason.message : 'Something went wrong';
  }

  function isOrderedType(type: string): boolean {
    return /VARCHAR|CHAR|TEXT|DATE|TIME|INT|DECIMAL|NUMERIC|REAL|FLOAT|DOUBLE/i.test(type);
  }

  function isTextType(type: string): boolean {
    return /VARCHAR|CHAR|TEXT/i.test(type);
  }

  function closeFilter() {
    categoryRequestId++;
    filterColumn = null;
    filterOperator = '=';
    filterValue = '';
    categoryValues = [];
    categorySearch = '';
    categoryTotal = 0;
    categoryHasMore = false;
    selectedCategories = [];
    categoriesLoading = false;
    categoriesError = '';
  }

  async function loadNodes() {
    loadingNodes = true;
    error = '';
    try {
      nodes = await api.listNodes();
      if (nodes.length) await selectNode(nodes[0].id);
    } catch (reason) {
      error = message(reason);
    } finally {
      loadingNodes = false;
    }
  }

  async function selectNode(id: string) {
    const datasetId = ++datasetRequestId;
    requestId++;
    closeFilter();
    selectedNodeId = id;
    selectedDataset = '';
    datasets = [];
    result = null;
    filters = [];
    sorts = [];
    error = '';
    try {
      const next = await api.listDatasets(id);
      if (datasetId !== datasetRequestId) return;
      datasets = next;
      if (next.length) await selectDataset(next[0].id);
    } catch (reason) {
      if (datasetId === datasetRequestId) error = message(reason);
    }
  }

  async function selectDataset(name: string) {
    closeFilter();
    selectedDataset = name;
    filters = [];
    sorts = [];
    page = 1;
    pageInput = '1';
    await loadData();
  }

  async function loadData() {
    if (!selectedNodeId || !selectedDataset) return;
    const id = ++requestId;
    loadingData = true;
    error = '';
    try {
      const next = await api.queryDataset(selectedNodeId, selectedDataset, { page, page_size: pageSize, filters, sorts });
      if (id !== requestId) return;
      result = next;
      page = next.page;
      pageInput = String(next.page);
    } catch (reason) {
      if (id === requestId) error = message(reason);
    } finally {
      if (id === requestId) loadingData = false;
    }
  }

  async function upload(event: Event) {
    const input = event.currentTarget as HTMLInputElement;
    const file = input.files?.[0];
    if (!file) return;
    mutating = true;
    error = '';
    try {
      const node = await api.uploadNode(file);
      nodes = [...nodes.filter((item) => item.id !== node.id), node];
      await selectNode(node.id);
    } catch (reason) {
      error = message(reason);
    } finally {
      mutating = false;
      input.value = '';
    }
  }

  async function attach() {
    const path = attachPath.trim();
    if (!path) return;
    mutating = true;
    error = '';
    try {
      const node = await api.attachNode(path);
      nodes = [...nodes.filter((item) => item.id !== node.id), node];
      attachPath = '';
      await selectNode(node.id);
    } catch (reason) {
      error = message(reason);
    } finally {
      mutating = false;
    }
  }

  async function openFilter(column: ColumnInfo) {
    closeFilter();
    filterColumn = column;
    if (isTextType(column.type)) await loadCategoryValues(true);
  }

  async function loadCategoryValues(reset: boolean) {
    if (!filterColumn) return;
    const id = ++categoryRequestId;
    const offset = reset ? 0 : categoryValues.length;
    if (reset) {
      categoryValues = [];
      categoryTotal = 0;
      categoryHasMore = false;
    }
    categoriesLoading = true;
    categoriesError = '';
    try {
      const response = await api.getCategoryValues(selectedNodeId, selectedDataset, filterColumn.name, {
        search: categorySearch.trim(), offset
      });
      if (id !== categoryRequestId) return;
      categoryValues = reset ? response.values : [...categoryValues, ...response.values];
      categoryTotal = response.total;
      categoryHasMore = response.has_more;
    } catch (reason) {
      if (id === categoryRequestId) categoriesError = message(reason);
    } finally {
      if (id === categoryRequestId) categoriesLoading = false;
    }
  }

  function toggleCategory(value: string, checked: boolean) {
    selectedCategories = checked ? [...selectedCategories, value] : selectedCategories.filter((item) => item !== value);
  }

  function selectVisibleCategories() {
    selectedCategories = [...new Set([...selectedCategories, ...categoryValues.map((item) => item.value)])];
  }

  async function addCategoryFilter() {
    if (!filterColumn || selectedCategories.length === 0) return;
    filters = [...filters, { column: filterColumn.name, operator: 'in', value: selectedCategories }];
    closeFilter();
    page = 1;
    await loadData();
  }

  async function addFilter() {
    if (!filterColumn) return;
    const noValue = filterOperator === 'is_null' || filterOperator === 'not_null';
    if (!noValue && filterValue === '') return;
    const value = noValue ? undefined : filterValue;
    filters = [...filters, { column: filterColumn.name, operator: filterOperator, ...(value === undefined ? {} : { value }) }];
    closeFilter();
    page = 1;
    await loadData();
  }

  async function removeFilter(index: number) {
    filters = filters.filter((_, itemIndex) => itemIndex !== index);
    page = 1;
    await loadData();
  }

  async function cycleSort(column: ColumnInfo) {
    const existing = sorts.find((sort) => sort.column === column.name);
    sorts = existing?.direction === 'asc'
      ? sorts.map((sort) => sort.column === column.name ? { ...sort, direction: 'desc' } : sort)
      : existing
        ? sorts.filter((sort) => sort.column !== column.name)
        : [...sorts, { column: column.name, direction: 'asc' }];
    page = 1;
    await loadData();
  }

  async function removeSort(column: string) {
    sorts = sorts.filter((sort) => sort.column !== column);
    page = 1;
    await loadData();
  }

  async function changePage(next: number) {
    if (next < 1 || next > totalPages || next === page) return;
    page = next;
    await loadData();
  }

  async function jumpPage() {
    const next = Math.min(Math.max(1, Number.parseInt(pageInput) || 1), Math.max(totalPages, 1));
    pageInput = String(next);
    await changePage(next);
  }

  async function changePageSize(event: Event) {
    pageSize = Number((event.currentTarget as HTMLSelectElement).value);
    page = 1;
    await loadData();
  }

  async function openStats(column: ColumnInfo) {
    if (!column.numeric) return;
    statsColumn = column;
    stats = null;
    statsError = '';
    statsLoading = true;
    await tick();
    statsDialog.showModal();
    try {
      stats = await api.getColumnStats(selectedNodeId, selectedDataset, column.name);
    } catch (reason) {
      statsError = message(reason);
    } finally {
      statsLoading = false;
    }
  }

  function closeStats() {
    statsColumn = null;
    stats = null;
  }

  function closeStatsBackdrop(event: MouseEvent) {
    const bounds = statsDialog.getBoundingClientRect();
    if (event.clientX < bounds.left || event.clientX > bounds.right || event.clientY < bounds.top || event.clientY > bounds.bottom) statsDialog.close();
  }

  function display(value: unknown): string {
    if (typeof value === 'object' && value !== null) return JSON.stringify(value);
    return String(value);
  }

  function filterDisplay(value: FilterCondition['value']): string {
    return Array.isArray(value) ? value.join(', ') : String(value ?? '');
  }

  function sortFor(column: string): SortCondition | undefined {
    return sorts.find((sort) => sort.column === column);
  }

  function compact(value: number | null | undefined): string {
    if (value == null) return '—';
    return new Intl.NumberFormat(undefined, { maximumFractionDigits: 4 }).format(value);
  }

  function binLabel(bin: ColumnStats['histogram'][number]): string {
    return `${compact(bin.lower)}–${compact(bin.upper)}`;
  }
</script>

<svelte:window onkeydown={(event) => event.key === 'Escape' && !statsColumn && closeFilter()} />

<div class="shell">
  <aside class="rail">
    <div class="brand"><span class="mark" aria-hidden="true">D</span><div><strong>DuckScope</strong><small>Local data explorer</small></div></div>

    <section class="source-tools" aria-labelledby="add-source-heading">
      <h2 id="add-source-heading">Add source</h2>
      <label class="upload-button" class:disabled={mutating}>
        <span aria-hidden="true">↑</span> {mutating ? 'Opening…' : 'Upload file'}
        <input type="file" accept=".csv,.tsv,.parquet,.json,.ndjson,.duckdb,.db" onchange={upload} disabled={mutating} />
      </label>
      <form onsubmit={(event) => { event.preventDefault(); attach(); }}>
        <label for="database-path">Attach database path</label>
        <div class="input-row">
          <input id="database-path" bind:value={attachPath} placeholder="/data/example.duckdb" disabled={mutating} />
          <button type="submit" disabled={mutating || !attachPath.trim()} aria-label="Attach database path">→</button>
        </div>
      </form>
      <p class="hint">CSV, TSV, Parquet, JSON, NDJSON or DuckDB</p>
    </section>

    <nav class="nodes" aria-label="Running nodes">
      <div class="section-title"><h2>Running nodes</h2><span>{nodes.length}</span></div>
      {#if loadingNodes}
        <p class="rail-state">Loading nodes…</p>
      {:else if nodes.length === 0}
        <p class="rail-state">No active nodes yet.</p>
      {:else}
        {#each nodes as node (node.id)}
          <button class:active={node.id === selectedNodeId} onclick={() => selectNode(node.id)}>
            <span class="status-dot" aria-hidden="true"></span>
            <span><strong>{node.name}</strong><small>{node.kind}</small></span>
          </button>
        {/each}
      {/if}
    </nav>
    <footer><span class="status-dot" aria-hidden="true"></span> Backend connected</footer>
  </aside>

  <main>
    <header class="topbar">
      <div class="node-tabs" role="tablist" aria-label="Node tabs">
        {#each nodes as node (node.id)}
          <button role="tab" aria-selected={node.id === selectedNodeId} class:active={node.id === selectedNodeId} onclick={() => selectNode(node.id)}>{node.name}</button>
        {/each}
      </div>
      <div class="connection"><span class="status-dot"></span> Local</div>
    </header>

    {#if !selectedNodeId}
      <section class="welcome">
        <div class="welcome-icon">D</div>
        <h1>Explore data without the wait</h1>
        <p>Upload a supported data file or attach a local DuckDB database to inspect large datasets safely, one page at a time.</p>
        {#if error}<div class="error" role="alert"><span>!</span><div><strong>Could not load DuckScope</strong><p>{error}</p></div><button onclick={loadNodes}>Retry</button></div>{/if}
        <label class="primary-button">Choose a file<input type="file" accept=".csv,.tsv,.parquet,.json,.ndjson,.duckdb,.db" onchange={upload} /></label>
      </section>
    {:else}
      <section class="workspace">
        <div class="workspace-head">
          <div><p class="eyebrow">{selectedNode?.kind ?? 'ACTIVE NODE'}</p><h1>{selectedNode?.name}</h1></div>
          {#if result}<div class="query-meta"><strong>{result.total_rows.toLocaleString()}</strong> rows <span>•</span> {compact(result.elapsed_ms)} ms</div>{/if}
        </div>

        <nav class="dataset-tabs" aria-label="Datasets">
          {#each datasets as dataset (dataset.id)}
            <button class:active={dataset.id === selectedDataset} onclick={() => selectDataset(dataset.id)}><span aria-hidden="true">▦</span>{dataset.name}<small>{dataset.schema} · {dataset.type}</small></button>
          {/each}
        </nav>

        {#if error}
          <div class="error" role="alert"><span>!</span><div><strong>Request failed</strong><p>{error}</p></div><button onclick={() => selectedDataset ? loadData() : loadNodes()}>Retry</button></div>
        {/if}

        {#if datasets.length === 0 && !error}
          <div class="empty"><strong>No datasets found</strong><p>This node has no tables or views to browse.</p></div>
        {:else if selectedDataset}
          <div class="controls">
            <div class="chips" aria-label="Active query conditions">
              {#each filters as filter, index (filter)}
                <span class="chip filter-chip"><b>{filter.column}</b> {filter.operator.replace(/_/g, ' ')} <span class="chip-value" title={filterDisplay(filter.value)}>{filterDisplay(filter.value)}</span><button aria-label={`Remove filter ${filter.column}`} onclick={() => removeFilter(index)}>×</button></span>
              {/each}
              {#each sorts as sort, index (sort.column)}
                <span class="chip sort-chip"><i>{index + 1}</i><b>{sort.column}</b> {sort.direction}<button aria-label={`Remove sort ${sort.column}`} onclick={() => removeSort(sort.column)}>×</button></span>
              {/each}
              {#if filters.length === 0 && sorts.length === 0}<span class="muted">No filters or sorts applied</span>{/if}
            </div>
            <button class="refresh" onclick={loadData} disabled={loadingData} aria-label="Refresh data">↻</button>
          </div>

          {#if filterColumn}
            <div class="filter-editor">
              <strong>Filter <span>{filterColumn.name}</span></strong>
              {#if isTextType(filterColumn.type)}
                <section class="category-picker" aria-label={`Categories for ${filterColumn.name}`}>
                  <form class="category-search" onsubmit={(event) => { event.preventDefault(); loadCategoryValues(true); }}>
                    <label>Search categories<input type="search" bind:value={categorySearch} placeholder="Search…" /></label>
                    <button type="submit" disabled={categoriesLoading}>Search</button>
                  </form>
                  <div class="category-actions">
                    <button type="button" onclick={selectVisibleCategories} disabled={categoriesLoading || categoryValues.length === 0}>Select all visible</button>
                    <button type="button" onclick={() => selectedCategories = []} disabled={selectedCategories.length === 0}>Clear</button>
                    <span>{selectedCategories.length} selected</span>
                  </div>
                  {#if categoriesLoading && categoryValues.length === 0}<p class="category-state"><span class="spinner"></span>Loading values…</p>
                  {:else if categoriesError}<p class="category-state error-text" role="alert">{categoriesError}</p>
                  {:else}
                    <div class="category-list">
                      {#each categoryValues as item (item.value)}
                        <label><input type="checkbox" checked={selectedCategories.includes(item.value)} onchange={(event) => toggleCategory(item.value, event.currentTarget.checked)} /><span title={item.value}>{item.value}</span><small>{item.count.toLocaleString()}</small></label>
                      {:else}<p class="category-state">No matching values.</p>{/each}
                    </div>
                    <div class="category-page"><span>{categoryValues.length.toLocaleString()} / {categoryTotal.toLocaleString()}</span>{#if categoryHasMore}<button type="button" onclick={() => loadCategoryValues(false)} disabled={categoriesLoading}>{categoriesLoading ? 'Loading…' : 'Load more'}</button>{/if}</div>
                  {/if}
                  <button class="primary-button" type="button" onclick={addCategoryFilter} disabled={selectedCategories.length === 0}>Apply categories</button>
                </section>
              {/if}
              <form class="advanced-filter" aria-label="Advanced filter" onsubmit={(event) => { event.preventDefault(); addFilter(); }}>
                {#if isTextType(filterColumn.type)}<span>Advanced</span>{/if}
                <label>Operator<select bind:value={filterOperator}>{#each operators as operator (operator.value)}<option value={operator.value}>{operator.label}</option>{/each}</select></label>
                {#if filterOperator !== 'is_null' && filterOperator !== 'not_null'}
                  <label>Value<input type={filterColumn.numeric ? 'number' : 'text'} step="any" bind:value={filterValue} /></label>
                {/if}
                <button class="primary-button" type="submit" disabled={filterOperator !== 'is_null' && filterOperator !== 'not_null' && filterValue === ''}>Apply filter</button>
              </form>
              <button class="cancel-filter" type="button" onclick={closeFilter}>Cancel</button>
            </div>
          {/if}

          <div class="table-card" aria-busy={loadingData}>
            {#if loadingData && !result}<div class="table-state"><span class="spinner"></span>Loading rows…</div>
            {:else if result && result.rows.length === 0}<div class="table-state"><strong>No matching rows</strong><span>Change or remove filters to see more data.</span></div>
            {:else if result}
              <div class="table-scroll">
                <table>
                  <thead><tr>
                    {#each result.columns as column (column.name)}
                      <th style:min-width={`${Math.max(148, Math.min(260, column.name.length * 9 + 70))}px`}>
                        <div class="column-head">
                          <button class="column-title" class:numeric={column.numeric} onclick={() => column.numeric && openStats(column)} title={column.numeric ? `View statistics for ${column.name}` : column.name}>
                            <strong>{column.name}</strong><small>{column.type}</small>
                          </button>
                          <div class="header-actions">
                            <button class:sorted={sorts.some((sort) => sort.column === column.name)} onclick={() => cycleSort(column)} aria-label={`Sort ${column.name}`}>
                              {#if sortFor(column.name)}<i>{sorts.findIndex((item) => item.column === column.name) + 1}</i>{sortFor(column.name)?.direction === 'asc' ? '↑' : '↓'}{:else}↕{/if}
                            </button>
                            <button class:filtered={filters.some((filter) => filter.column === column.name)} onclick={() => openFilter(column)} aria-label={`Filter ${column.name}`}>⌕</button>
                          </div>
                        </div>
                        <div class="null-gauge" title={`${(column.null_fraction * 100).toFixed(1)}% null`}><span style:width={`${Math.min(100, column.null_fraction * 100)}%`}></span></div>
                      </th>
                    {/each}
                  </tr></thead>
                  <tbody>
                    {#each result.rows as row, index (index)}
                      <tr>{#each result.columns as column (column.name)}<td>{#if row[column.name] == null}<span class="null">NULL</span>{:else}<span title={display(row[column.name])}>{display(row[column.name])}</span>{/if}</td>{/each}</tr>
                    {/each}
                  </tbody>
                </table>
              </div>
              {#if loadingData}<div class="loading-overlay"><span class="spinner"></span>Updating…</div>{/if}
            {/if}
          </div>

          {#if result}
            <div class="pagination">
              <label>Rows per page <select value={pageSize} onchange={changePageSize}>{#each pageSizes as size (size)}<option value={size}>{size}</option>{/each}</select></label>
              <span class="range">{result.total_rows ? ((page - 1) * pageSize + 1).toLocaleString() : 0}–{Math.min(page * pageSize, result.total_rows).toLocaleString()} of {result.total_rows.toLocaleString()}</span>
              <div class="pager">
                <button onclick={() => changePage(page - 1)} disabled={page <= 1 || loadingData} aria-label="Previous page">←</button>
                <form onsubmit={(event) => { event.preventDefault(); jumpPage(); }}><label for="page-number">Page</label><input id="page-number" type="number" min="1" max={Math.max(totalPages, 1)} bind:value={pageInput} /><span>of {totalPages.toLocaleString()}</span></form>
                <button onclick={() => changePage(page + 1)} disabled={page >= totalPages || loadingData} aria-label="Next page">→</button>
              </div>
            </div>
          {/if}
        {/if}
      </section>
    {/if}
  </main>
</div>

{#if statsColumn}
  <dialog class="modal" bind:this={statsDialog} aria-labelledby="stats-title" onclose={closeStats} onclick={closeStatsBackdrop}>
      <header><div><p>Column profile</p><h2 id="stats-title">{statsColumn.name}</h2><span>{statsColumn.type}</span></div><button onclick={() => statsDialog.close()} aria-label="Close statistics">×</button></header>
      {#if statsLoading}<div class="modal-state"><span class="spinner"></span>Computing statistics…</div>
      {:else if statsError}<div class="modal-state error-text"><strong>Statistics unavailable</strong><span>{statsError}</span></div>
      {:else if stats}
        <div class="metrics">
          <div><span>Rows</span><strong>{stats.row_count.toLocaleString()}</strong></div><div><span>Non-null</span><strong>{stats.non_null_count.toLocaleString()}</strong></div>
          <div><span>Nulls</span><strong>{stats.null_count.toLocaleString()}</strong><small>{(stats.null_fraction * 100).toFixed(1)}%</small></div><div><span>Mean</span><strong>{compact(stats.mean)}</strong></div>
          <div><span>Minimum</span><strong>{compact(stats.min)}</strong></div><div><span>Maximum</span><strong>{compact(stats.max)}</strong></div>
          <div><span>Std. deviation</span><strong>{compact(stats.stddev)}</strong></div><div><span>Median</span><strong>{compact(stats.median)}</strong></div>
        </div>
        <div class="quartiles"><span><b>P25</b>{compact(stats.p25)}</span><span><b>P50</b>{compact(stats.median)}</span><span><b>P75</b>{compact(stats.p75)}</span></div>
        <section class="histogram"><div class="histogram-head"><h3>Distribution</h3><span>{stats.histogram.length} bins</span></div>
          {#if stats.histogram.length}
            <svg viewBox={`0 0 ${stats.histogram.length * 28} 160`} preserveAspectRatio="none" role="img" aria-label={`Histogram of ${statsColumn.name}`}>
              {#each stats.histogram as bin, index (index)}<rect x={index * 28 + 2} y={150 - (bin.count / maxBin) * 140} width="24" height={(bin.count / maxBin) * 140} rx="2"><title>{binLabel(bin)}: {bin.count.toLocaleString()}</title></rect>{/each}
            </svg>
            <div class="axis"><span>{compact(stats.min)}</span><span>{compact(stats.max)}</span></div>
          {:else}<p class="muted">No values to chart.</p>{/if}
        </section>
      {/if}
  </dialog>
{/if}
