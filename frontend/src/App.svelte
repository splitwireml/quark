<script lang="ts">
  import { onMount, tick } from 'svelte';
  import { basicSetup, EditorView } from 'codemirror';
  import { autocompletion, completionStatus, startCompletion, type CompletionSource } from '@codemirror/autocomplete';
  import { keywordCompletionSource, schemaCompletionSource, sql, StandardSQL, type SQLConfig, type SQLNamespace } from '@codemirror/lang-sql';
  import { keymap } from '@codemirror/view';
  import * as api from './lib/api';
  import { buildAggregateSql } from './lib/aggregate-sql';
  import type { AggregateCount, AggregateMetric, CategoryValue, ColumnInfo, ColumnStats, DatasetInfo, DistributionMode, FilterCondition, FilterOperator, NodeInfo, QueryResponse, RowDensity, SavedQuery, SortCondition, WorkbookPreview } from './lib/types';

  import Button from './components/atoms/Button.svelte';
  import TitleBar from './components/organisms/TitleBar.svelte';
  import SourceRail from './components/organisms/SourceRail.svelte';
  import SourceDisclosure from './components/organisms/SourceDisclosure.svelte';
  import WelcomeScreen from './components/organisms/WelcomeScreen.svelte';
  import DatasetHead from './components/organisms/DatasetHead.svelte';
  import DatasetTabsBar from './components/organisms/DatasetTabsBar.svelte';
  import SavedQueriesPane from './components/organisms/SavedQueriesPane.svelte';
  import QueryConditionBar from './components/organisms/QueryConditionBar.svelte';
  import ColumnsMenuPopover from './components/organisms/ColumnsMenuPopover.svelte';
  import AggregateMenuPopover from './components/organisms/AggregateMenuPopover.svelte';
  import DedupeMenuPopover from './components/organisms/DedupeMenuPopover.svelte';
  import SqlEditorPanel from './components/organisms/SqlEditorPanel.svelte';
  import DataGridTable from './components/organisms/DataGridTable.svelte';
  import PaginationFooter from './components/organisms/PaginationFooter.svelte';
  import InspectorPanel from './components/organisms/InspectorPanel.svelte';
  import FilterInspector from './components/organisms/FilterInspector.svelte';
  import ProfileInspector from './components/organisms/ProfileInspector.svelte';
  import WorkbookDialog from './components/organisms/WorkbookDialog.svelte';
  import AppShell from './components/templates/AppShell.svelte';

  const pageSizes = [50, 100, 250, 500, 1000];
  const savedQueriesKey = 'quark.savedQueries';
  const baseOperators: { value: FilterOperator; label: string }[] = [{ value: '=', label: 'equals' }, { value: '!=', label: 'not equal' }, { value: 'is_null', label: 'is null' }, { value: 'not_null', label: 'is not null' }];
  const textOperators: { value: FilterOperator; label: string }[] = [{ value: 'contains', label: 'contains' }, { value: 'starts_with', label: 'starts with' }, { value: 'ends_with', label: 'ends with' }];
  const orderedOperators: { value: FilterOperator; label: string }[] = [{ value: '>', label: 'greater than' }, { value: '>=', label: 'at least' }, { value: '<', label: 'less than' }, { value: '<=', label: 'at most' }];
  const aggregateMetricOptions: { value: AggregateMetric; label: string; numeric?: true; ordered?: true }[] = [{ value: 'count', label: 'Count' }, { value: 'distinct', label: 'Distinct' }, { value: 'min', label: 'Min', ordered: true }, { value: 'max', label: 'Max', ordered: true }, { value: 'sum', label: 'Sum', numeric: true }, { value: 'avg', label: 'Average', numeric: true }, { value: 'median', label: 'Median', numeric: true }, { value: 'stddev', label: 'Std. dev.', numeric: true }];


  let nodes = $state<NodeInfo[]>([]);
  let datasets = $state<DatasetInfo[]>([]);
  let selectedNodeId = $state('');
  let selectedDataset = $state('');
  let result = $state.raw<QueryResponse | null>(null);
  let filters = $state<FilterCondition[]>([]);
  let sorts = $state<SortCondition[]>([]);
  let dedupeColumns = $state<string[]>([]);
  let dedupeDraft = $state<string[]>([]);
  let aggregateColumn = $state('');
  let aggregateColumnSearch = $state('');
  let aggregateColumns = $state<string[]>([]);
  let aggregateMetrics = $state<AggregateMetric[]>([]);
  let aggregateSourceSql = $state('');
  let aggregateSourceColumns = $state.raw<ColumnInfo[]>([]);
  let hiddenColumns = $state<string[]>([]);
  let lastHiddenColumn = $state<string | null>(null);
  let railCollapsed = $state(false);
  let tableExpanded = $state(false);
  let rowDensity = $state<RowDensity>('default');
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
  let columnSearch = $state('');
  let columnMenuSearch = $state('');
  let nullThreshold = $state(50);
  let activeColumnMatch = $state(0);
  let selectedCell = $state<{ row: number; column: string } | null>(null);
  let tableScroll = $state<HTMLDivElement | null>(null);
  let categoryValues = $state.raw<CategoryValue[]>([]);
  let categorySearch = $state('');
  let categoryTotal = $state<AggregateCount>(0);
  let categoryHasMore = $state(false);
  let selectedCategories = $state<string[]>([]);
  let categoriesLoading = $state(false);
  let categoriesError = $state('');
  let stats = $state.raw<ColumnStats | null>(null);
  let statsColumn = $state<ColumnInfo | null>(null);
  let statsLoading = $state(false);
  let statsError = $state('');
  let distributionMode = $state<DistributionMode>('count');
  let cumulativeDistribution = $state(false);
  let shownColumnTypes = $state<string[]>([]);
  let inspectorMode = $state<'filter' | 'profile' | null>(null);
  let railOpen = $state(false);
  let sourceOpen = $state(false);
  let binReadout = $state('Focus a bin to read its range and count.');
  let inspectorTrigger: HTMLButtonElement | null = null;
  let inspector = $state<HTMLElement | null>(null);
  let filterInput = $state<HTMLInputElement | HTMLSelectElement | null>(null);
  let requestId = 0;
  let datasetRequestId = 0;
  let categoryRequestId = 0;
  let statsRequestId = 0;
  let workbookDialog = $state<HTMLDialogElement | null>(null);
  let workbookPreview = $state<WorkbookPreview | null>(null);
  let workbookSheets = $state<string[]>([]);
  let confirmingWorkbook = $state(false);
  let workspaceTab = $state<'data' | 'queries'>('data');
  let queryMode = $state<'builder' | 'sql'>('builder');
  let sqlOpen = $state(false);
  let sqlText = $state('');
  let sqlBase = $state('');
  let activeSql = $state('');
  let sqlError = $state('');
  let storageError = $state('');
  let savedQueries = $state<SavedQuery[]>([]);
  let editorHost = $state<HTMLDivElement | null>(null);
  let sqlTrigger = $state<HTMLButtonElement | null>(null);
  let editorView: EditorView | null = null;
  let queryMenuOpen = $state<'columns' | 'aggregate' | 'dedupe' | null>(null);

  let selectedNode = $derived(nodes.find((node) => node.id === selectedNodeId));
  let currentDataset = $derived(datasets.find((dataset) => dataset.id === selectedDataset));
  let totalPages = $derived(pageLimit(result?.total_pages ?? 0));
  let visibleColumns = $derived(result?.columns.filter((column) => !hiddenColumns.includes(column.name)) ?? []);
  let aggregateFieldOptions = $derived((aggregateSourceColumns.length ? aggregateSourceColumns : result?.columns ?? []).filter((column) => column.profile_kind !== null));
  let aggregateColumnMatches = $derived.by(() => { const query = aggregateColumnSearch.trim().toLowerCase(); return query ? aggregateFieldOptions.filter((column) => column.name.toLowerCase().includes(query)) : aggregateFieldOptions; });
  let selectedAggregateColumn = $derived(aggregateFieldOptions.find((column) => column.name === aggregateColumns[aggregateColumns.length - 1]));
  let availableAggregateMetrics = $derived(aggregateMetricOptions.filter((metric) => (!metric.numeric || selectedAggregateColumn?.numeric) && (!metric.ordered || selectedAggregateColumn?.numeric || selectedAggregateColumn?.profile_kind === 'date')));
  let columnMatches = $derived.by(() => { const query = columnSearch.trim().toLowerCase(); return query ? visibleColumns.filter((column) => column.name.toLowerCase().includes(query)) : []; });
  let columnMenuItems = $derived.by(() => { const query = columnMenuSearch.trim().toLowerCase(); return query ? (result?.columns ?? []).filter((column) => column.name.toLowerCase().includes(query)) : result?.columns ?? []; });
  let columnTypes = $derived([...new Set((result?.columns ?? []).map((column) => column.type))]);
  let columnTypeCounts = $derived.by(() => { const counts: Record<string, number> = Object.create(null); for (const column of result?.columns ?? []) counts[column.type] = (counts[column.type] ?? 0) + 1; return counts; });
  let aggregateRowTones = $derived.by(() => {
    const rows = result?.rows ?? [];
    const majorIndex = queryMode === 'sql' && aggregateSourceSql ? aggregateColumns[0] : '';
    if (!majorIndex) return [];
    let alternate = false;
    return rows.map((row, index) => { if (index && !Object.is(row[majorIndex], rows[index - 1][majorIndex])) alternate = !alternate; return alternate; });
  });
  let operators = $derived.by(() => filterColumn ? [...baseOperators, ...(!filterColumn.numeric && isTextType(filterColumn.type) ? textOperators : []), ...(filterColumn.numeric || isOrderedType(filterColumn.type) ? orderedOperators : [])] : baseOperators);
  let maxBin = $derived(stats && stats.kind !== 'categorical' && stats.histogram.length ? Math.max(...stats.histogram.map((bin) => Number(bin.count)), 1) : 1);
  let querySummary = $derived(result ? queryMode === 'sql' ? `${count(result.total_rows)} SQL result rows, page ${result.page} of ${count(result.total_pages)}.` : `${count(result.total_rows)} rows, page ${result.page} of ${count(result.total_pages)}, ${filters.length} filters, ${sorts.length} sorts, and ${dedupeColumns.length} dedupe keys.` : '');

  onMount(() => { loadSavedQueries(); loadNodes(); return () => editorView?.destroy(); });

  function message(reason: unknown): string { return reason instanceof Error ? reason.message : 'Something went wrong'; }
  function isOrderedType(type: string): boolean { return /VARCHAR|CHAR|TEXT|DATE|TIME|INT|DECIMAL|NUMERIC|REAL|FLOAT|DOUBLE/i.test(type); }
  function isTextType(type: string): boolean { return /VARCHAR|CHAR|TEXT/i.test(type); }
  function isBooleanType(type: string): boolean { return type.toLowerCase() === 'boolean'; }
  function syncQueryMenu(menu: 'columns' | 'aggregate' | 'dedupe', event: Event) {
    const open = (event.currentTarget as HTMLDetailsElement).open;
    queryMenuOpen = open ? menu : queryMenuOpen === menu ? null : queryMenuOpen;
  }
  function clearAggregateDraft() { aggregateColumn = ''; aggregateColumnSearch = ''; aggregateColumns = []; aggregateMetrics = []; aggregateSourceSql = ''; aggregateSourceColumns = []; if (queryMenuOpen === 'aggregate') queryMenuOpen = null; }
  function addAggregateColumn(column: string) { if (!column || aggregateColumns.includes(column)) return; aggregateColumns = [...aggregateColumns, column]; aggregateColumn = ''; aggregateMetrics = ['count']; }
  function removeAggregateColumn(column: string) { const next = aggregateColumns.filter((item) => item !== column); aggregateColumns = next; aggregateMetrics = next.length ? ['count'] : []; }
  function toggleAggregateMetric(metric: AggregateMetric, checked: boolean) { aggregateMetrics = checked ? [...aggregateMetrics, metric] : aggregateMetrics.filter((item) => item !== metric); }
  function isWorkbookPreview(node: NodeInfo | WorkbookPreview): node is WorkbookPreview { return node.kind === 'workbook' && 'sheets' in node && Array.isArray(node.sheets); }
  function toggleWorkbookSheet(sheet: string, checked: boolean) { workbookSheets = checked ? [...workbookSheets, sheet] : workbookSheets.filter((item) => item !== sheet); }
  function quoteIdentifier(value: string): string { return `"${value.replace(/"/g, '""')}"`; }
  function seedSql(dataset = currentDataset): string { return dataset ? `SELECT * FROM ${quoteIdentifier(dataset.schema)}.${quoteIdentifier(dataset.name)}` : ''; }

  function loadSavedQueries() {
    try {
      const stored: unknown = JSON.parse(localStorage.getItem(savedQueriesKey) ?? '[]');
      savedQueries = Array.isArray(stored) ? stored.filter((item): item is SavedQuery => typeof item === 'object' && item !== null && ['id', 'name', 'sql', 'nodeId', 'dataset'].every((key) => typeof (item as Record<string, unknown>)[key] === 'string')) : [];
    } catch { savedQueries = []; storageError = 'Saved queries could not be read from this browser.'; }
  }

  function storeSavedQueries(next: SavedQuery[]) {
    try { localStorage.setItem(savedQueriesKey, JSON.stringify(next)); savedQueries = next; storageError = ''; }
    catch { storageError = 'Saved queries could not be stored in this browser.'; }
  }

  function saveQuery(value: string) {
    const query = value.trim();
    if (!query) return;
    const firstLine = query.split(/\r?\n/, 1)[0].replace(/\s+/g, ' ').trim();
    storeSavedQueries([...savedQueries, { id: crypto.randomUUID(), name: firstLine.slice(0, 64) || `Query ${savedQueries.length + 1}`, sql: query, nodeId: selectedNodeId, dataset: selectedDataset }]);
  }

  function deleteQuery(id: string) { storeSavedQueries(savedQueries.filter((query) => query.id !== id)); }

  function editorSchema(): SQLNamespace {
    const schema: Record<string, Record<string, SQLNamespace>> = {};
    for (const dataset of datasets) {
      schema[dataset.schema] ??= {};
      schema[dataset.schema][dataset.name] = {
        self: { label: dataset.name, type: 'type', apply: quoteIdentifier(dataset.name) },
        children: dataset.columns
      };
    }
    return schema;
  }

  function sqlCompletionAllowed(text: string) {
    const nodeName = StandardSQL.language.parser.parse(text).resolveInner(text.length, -1).name;
    return nodeName !== 'String' && !nodeName.includes('Comment');
  }

  function guardCompletion(source: CompletionSource): CompletionSource {
    return (context) => sqlCompletionAllowed(context.state.sliceDoc(0, context.pos)) ? source(context) : null;
  }

  function createSqlEditor() {
    editorView?.destroy();
    if (!editorHost) return;
    editorHost.replaceChildren();
    const sqlConfig: SQLConfig = { dialect: StandardSQL, schema: editorSchema(), defaultSchema: currentDataset?.schema, defaultTable: currentDataset?.name, upperCaseKeywords: true };
    editorView = new EditorView({
      doc: sqlText,
      parent: editorHost,
      extensions: [
        keymap.of([{ key: 'Shift-Enter', preventDefault: true, run: () => {
          if (!loadingData && sqlText.trim()) {
            page = 1;
            pageInput = '1';
            void runSql().then(() => editorView?.focus());
          }
          return true;
        } }]),
        basicSetup,
        sql(sqlConfig),
        autocompletion({ override: [guardCompletion(schemaCompletionSource(sqlConfig)), guardCompletion(keywordCompletionSource(StandardSQL, true))] }),
        EditorView.lineWrapping,
        EditorView.contentAttributes.of({ 'aria-label': 'SQL editor', 'aria-keyshortcuts': 'Shift+Enter' }),

        EditorView.updateListener.of((update) => {
          if (!update.docChanged) return;
          sqlText = update.state.doc.toString();
          const cursor = update.state.selection.main.head;
          const beforeCursor = update.state.sliceDoc(0, cursor);
          if (/\b(?:FROM|JOIN)\s+(?:"[^"]*"?|[\w$]+)?(?:\.(?:"[^"]*"?|[\w$]*))?$/i.test(beforeCursor) && completionStatus(update.state) !== 'active') {
            if (sqlCompletionAllowed(beforeCursor)) queueMicrotask(() => {
              if (editorView === update.view && completionStatus(update.view.state) !== 'active') startCompletion(update.view);
            });
          }
        })
      ]
    });
  }

  async function openSql(value?: string) {
    if (value !== undefined) sqlText = value;
    sqlOpen = true;
    sqlError = '';
    await tick();
    createSqlEditor();
    editorView?.focus();
  }

  function closeSql(restoreFocus = true) { editorView?.destroy(); editorView = null; sqlOpen = false; if (restoreFocus) tick().then(() => sqlTrigger?.focus()); }
  function toggleTableExpanded() { if (!tableExpanded) { if (sqlOpen) closeSql(false); queryMenuOpen = null; railOpen = false; } tableExpanded = !tableExpanded; }
  function resetSql(dataset: DatasetInfo | undefined) { closeSql(); queryMode = 'builder'; sqlText = seedSql(dataset); sqlBase = ''; activeSql = ''; sqlError = ''; }

  async function runSavedQuery(saved: SavedQuery) {
    workspaceTab = 'data';
    const node = nodes.find((item) => item.id === saved.nodeId);
    if (!node) { await openSql(saved.sql); sqlError = 'The source for this saved query is no longer available.'; return; }
    if (node.id !== selectedNodeId) {
      await selectNode(node.id, saved.dataset);
      if (selectedNodeId !== saved.nodeId || (datasets.some((dataset) => dataset.id === saved.dataset) && selectedDataset !== saved.dataset)) { await openSql(saved.sql); sqlError = 'Selection changed while opening this saved query. Review it before running.'; return; }
    }
    if (!datasets.some((dataset) => dataset.id === saved.dataset)) { await openSql(saved.sql); sqlError = 'The dataset for this saved query is no longer available.'; return; }
    if (saved.dataset !== selectedDataset) {
      await selectDataset(saved.dataset);
      if (selectedNodeId !== saved.nodeId || selectedDataset !== saved.dataset) { await openSql(saved.sql); sqlError = 'Selection changed while opening this saved query. Review it before running.'; return; }
    }
    await openSql(saved.sql);
    if (selectedNodeId !== saved.nodeId || selectedDataset !== saved.dataset) { sqlError = 'Selection changed while opening this saved query. Review it before running.'; return; }
    page = 1;
    pageInput = '1';
    await runSql();
  }

  async function discardWorkbook() {
    const preview = workbookPreview;
    workbookPreview = null;
    workbookSheets = [];
    if (!preview) return;
    try { await api.discardWorkbook(preview.id); }
    catch (reason) { error = message(reason); }
  }

  async function confirmWorkbook() {
    const preview = workbookPreview;
    if (!preview || !workbookSheets.length || confirmingWorkbook) return;
    confirmingWorkbook = true;
    error = '';
    try {
      const node = await api.confirmWorkbook(preview.id, workbookSheets);
      workbookPreview = null;
      workbookSheets = [];
      workbookDialog?.close();
      nodes = [...nodes.filter((item) => item.id !== node.id), node];
      await selectNode(node.id);
    } catch (reason) { error = message(reason); }
    finally { confirmingWorkbook = false; }
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

  function closeInspector() {
    statsRequestId++;
    inspectorMode = null;
    closeFilter();
    statsColumn = null;
    stats = null;
    statsLoading = false;
    statsError = '';
    const trigger = inspectorTrigger;
    inspectorTrigger = null;
    tick().then(() => trigger?.focus());
  }

  function handleKeydown(event: KeyboardEvent) {
    if (event.key === 'Escape') {
      if (inspectorMode) closeInspector();
      else if (sqlOpen) closeSql();
      else if (tableExpanded) tableExpanded = false;
      else railOpen = false;
      return;
    }
    if (event.key === 'u' && !event.altKey && !event.ctrlKey && !event.metaKey && !event.shiftKey && !isEditableElement(document.activeElement) && lastHiddenColumn) {
      restoreColumn(lastHiddenColumn);
      return;
    }
    if (event.key !== 'Tab' || !inspector) return;
    const focusable = [...inspector.querySelectorAll<HTMLElement>('button:not([disabled]), input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [href], [tabindex]:not([tabindex="-1"])')];
    if (!focusable.length) return;
    const first = focusable[0];
    const last = focusable[focusable.length - 1];
    if (!inspector.contains(document.activeElement)) { event.preventDefault(); first.focus(); }
    else if (event.shiftKey && document.activeElement === first) { event.preventDefault(); last.focus(); }
    else if (!event.shiftKey && document.activeElement === last) { event.preventDefault(); first.focus(); }
  }

  function focusInspector() { (filterInput ?? inspector?.querySelector<HTMLElement>('button, input, select, textarea, [href], [tabindex]:not([tabindex="-1"])'))?.focus(); }
  function isEditableElement(element: Element | null): boolean { return element instanceof HTMLInputElement && !['checkbox', 'radio'].includes(element.type) || element instanceof HTMLTextAreaElement || element instanceof HTMLSelectElement || element instanceof HTMLElement && element.isContentEditable; }

  async function loadNodes() {
    loadingNodes = true;
    error = '';
    try {
      nodes = await api.listNodes();
      if (nodes.length) await selectNode(nodes[0].id);
    } catch (reason) { error = message(reason); }
    finally { loadingNodes = false; }
  }

  async function selectNode(id: string, preferredDataset = '') {
    if (id === selectedNodeId && result) { railOpen = false; return; }
    const datasetId = ++datasetRequestId;
    requestId++;
    closeInspector();
    resetSql(undefined);
    workspaceTab = 'data';
    selectedNodeId = id;
    selectedDataset = '';
    datasets = [];
    result = null;
    filters = [];
    sorts = [];
    dedupeColumns = [];
    dedupeDraft = [];
    clearAggregateDraft();
    hiddenColumns = [];
    lastHiddenColumn = null;
    shownColumnTypes = [];
    error = '';
    railOpen = false;
    try {
      const next = await api.listDatasets(id);
      if (datasetId !== datasetRequestId) return;
      datasets = next;
      if (next.length) await selectDataset(next.find((dataset) => dataset.id === preferredDataset)?.id ?? next[0].id);
    } catch (reason) { if (datasetId === datasetRequestId) error = message(reason); }
  }

  async function selectDataset(name: string) {
    closeInspector();
    selectedDataset = name;
    resetSql(datasets.find((dataset) => dataset.id === name));
    workspaceTab = 'data';
    filters = [];
    sorts = [];
    dedupeColumns = [];
    dedupeDraft = [];
    clearAggregateDraft();
    hiddenColumns = [];
    lastHiddenColumn = null;
    shownColumnTypes = [];
    page = 1;
    pageInput = '1';
    await loadData();
  }

  async function loadData() {
    if (queryMode === 'sql') { await runSql(sqlBase || activeSql, true); return; }
    if (!selectedNodeId || !selectedDataset) return;
    const id = ++requestId;
    loadingData = true;
    error = '';
    try {
      const next = await api.queryDataset(selectedNodeId, selectedDataset, { page, page_size: pageSize, filters, sorts, dedupe_columns: dedupeColumns });
      if (id !== requestId) return;
      result = next;
      queryMode = 'builder';
      sqlError = '';
      selectedCell = null;
      page = next.page;
      pageInput = String(next.page);
    } catch (reason) { if (id === requestId) error = message(reason); }
    finally { if (id === requestId) loadingData = false; }
  }

  async function createAggregateView() {
    const source = queryMode === 'builder' ? result?.sql : aggregateSourceSql || sqlBase || activeSql;
    const columns = queryMode === 'builder' ? result?.columns ?? [] : aggregateSourceColumns.length ? aggregateSourceColumns : result?.columns ?? [];
    if (!selectedAggregateColumn || aggregateMetrics.length === 0 || !source) return;
    const query = buildAggregateSql(source, aggregateColumns, aggregateMetrics);
    if (!query) return;
    aggregateSourceSql = source;
    aggregateSourceColumns = columns;
    filters = [];
    sorts = [];
    dedupeColumns = [];
    dedupeDraft = [];
    page = 1;
    pageInput = '1';
    sqlText = query;
    closeSql(false);
    await runSql(query, false, true);
  }

  async function runSql(value = sqlText, keepSqlBase = false, keepAggregateBuilder = false) {
    const query = value.trim();
    if (!selectedNodeId || !query) { sqlError = 'Enter a SQL query to run.'; return; }
    const source = keepSqlBase ? sqlBase || query : query;
    if (!keepSqlBase) {
      filters = [];
      sorts = [];
      dedupeColumns = [];
      dedupeDraft = [];
      if (!keepAggregateBuilder) clearAggregateDraft();
    }
    const id = ++requestId;
    loadingData = true;
    error = '';
    sqlError = '';
    try {
      const next = await api.querySql(selectedNodeId, { sql: source, page, page_size: pageSize, filters, sorts, dedupe_columns: dedupeColumns });
      if (id !== requestId) return;
      result = next;
      sqlBase = source;
      activeSql = next.sql;
      queryMode = 'sql';
      selectedCell = null;
      hiddenColumns = [];
      shownColumnTypes = [];
      page = next.page;
      pageInput = String(next.page);
      if (sqlOpen) { await tick(); createSqlEditor(); }
    } catch (reason) { if (id === requestId) sqlError = message(reason); }
    finally { if (id === requestId) loadingData = false; }
  }

  async function loadActiveData() { if (queryMode === 'sql') await runSql(sqlBase || activeSql, true); else await loadData(); }

  async function upload(event: Event) {
    const input = event.currentTarget as HTMLInputElement;
    const file = input.files?.[0];
    if (!file) return;
    mutating = true;
    error = '';
    try {
      const node = await api.uploadNode(file);
      if (isWorkbookPreview(node)) {
        workbookPreview = node;
        workbookSheets = [...node.sheets];
        await tick();
        workbookDialog?.showModal();
      } else {
        nodes = [...nodes.filter((item) => item.id !== node.id), node];
        await selectNode(node.id);
      }
    } catch (reason) { error = message(reason); }
    finally { mutating = false; input.value = ''; }
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
    } catch (reason) { error = message(reason); }
    finally { mutating = false; }
  }

  async function openFilter(column: ColumnInfo, trigger?: HTMLButtonElement) {
    closeInspector();
    inspectorTrigger = trigger ?? null;
    inspectorMode = 'filter';
    filterColumn = column;
    await tick();
    focusInspector();
    if (isTextType(column.type)) await loadCategoryValues(true);
  }

  async function loadCategoryValues(reset: boolean) {
    if (!filterColumn) return;
    const id = ++categoryRequestId;
    const offset = reset ? 0 : categoryValues.length;
    if (reset) { categoryValues = []; categoryTotal = 0; categoryHasMore = false; }
    categoriesLoading = true;
    categoriesError = '';
    try {
      const response = queryMode === 'sql'
        ? await api.getSqlCategoryValues(selectedNodeId, filterColumn.name, { sql: sqlBase || activeSql }, { search: categorySearch.trim(), offset })
        : await api.getCategoryValues(selectedNodeId, selectedDataset, filterColumn.name, { search: categorySearch.trim(), offset });
      if (id !== categoryRequestId) return;
      categoryValues = reset ? response.values : [...categoryValues, ...response.values];
      categoryTotal = response.total;
      categoryHasMore = response.has_more;
    } catch (reason) { if (id === categoryRequestId) categoriesError = message(reason); }
    finally { if (id === categoryRequestId) categoriesLoading = false; }
  }

  function toggleCategory(value: string, checked: boolean) { selectedCategories = checked ? [...selectedCategories, value] : selectedCategories.filter((item) => item !== value); }
  function selectVisibleCategories() { selectedCategories = [...new Set([...selectedCategories, ...categoryValues.map((item) => item.value)])]; }

  async function addCategoryFilter() {
    if (!filterColumn || selectedCategories.length === 0) return;
    filters = [...filters, { column: filterColumn.name, operator: 'in', value: selectedCategories }];
    closeInspector();
    page = 1;
    await loadData();
  }

  async function addFilter() {
    if (!filterColumn) return;
    const noValue = filterOperator === 'is_null' || filterOperator === 'not_null';
    if (!noValue && filterValue === '') return;
    const numericValue = filterColumn.numeric ? normalizedNumber(filterValue) : filterValue;
    if (!noValue && numericValue === null) return;
    if (filterColumn.numeric && numericValue !== null) filterValue = formattedNumber(numericValue);
    const value = noValue ? undefined : filterColumn.numeric ? numericValue! : isBooleanType(filterColumn.type) ? filterValue === 'true' : filterValue;
    filters = [...filters, { column: filterColumn.name, operator: filterOperator, ...(value === undefined ? {} : { value }) }];
    closeInspector();
    page = 1;
    await loadData();
  }

  async function removeFilter(index: number) { filters = filters.filter((_, itemIndex) => itemIndex !== index); page = 1; await loadData(); }
  async function cycleSort(column: ColumnInfo) {
    const existing = sorts.find((sort) => sort.column === column.name);
    sorts = existing?.direction === 'asc' ? sorts.map((sort) => sort.column === column.name ? { ...sort, direction: 'desc' } : sort) : existing ? sorts.filter((sort) => sort.column !== column.name) : [...sorts, { column: column.name, direction: 'asc' }];
    page = 1;
    await loadData();
  }
  async function removeSort(column: string) { sorts = sorts.filter((sort) => sort.column !== column); page = 1; await loadData(); }
  async function clearQuery() { filters = []; sorts = []; dedupeColumns = []; dedupeDraft = []; page = 1; await loadData(); }
  async function backToBuilder() {
    queryMode = 'builder';
    clearAggregateDraft();
    filters = [];
    sorts = [];
    dedupeColumns = [];
    dedupeDraft = [];
    page = 1;
    pageInput = '1';
    await loadData();
  }
  function toggleDedupe(column: string, checked: boolean) { dedupeDraft = checked ? [...dedupeDraft, column] : dedupeDraft.filter((item) => item !== column); }
  async function applyDedupe() { dedupeColumns = [...dedupeDraft]; page = 1; await loadData(); }
  async function clearDedupe() { dedupeColumns = []; dedupeDraft = []; page = 1; await loadData(); }
  function isColumnProtected(column: string): boolean { return [...dedupeColumns, ...dedupeDraft].includes(column); }
  function hideColumn(column: string) { if (!isColumnProtected(column) && visibleColumns.length > 1) { hiddenColumns = [...hiddenColumns, column]; lastHiddenColumn = column; } }
  function hideColumnsAtNullFraction(fraction: number) {
    const columns = visibleColumns.filter((column) => !isColumnProtected(column.name) && column.null_fraction >= fraction).slice(0, visibleColumns.length - 1);
    if (columns.length) { hiddenColumns = [...hiddenColumns, ...columns.map((column) => column.name)]; lastHiddenColumn = columns[columns.length - 1].name; }
  }
  function restoreColumn(column: string) { hiddenColumns = hiddenColumns.filter((item) => item !== column); if (lastHiddenColumn === column) lastHiddenColumn = null; }
  function showAllColumns() { hiddenColumns = []; lastHiddenColumn = null; shownColumnTypes = []; }
  function isTypeShown(type: string): boolean { return shownColumnTypes.length === 0 || shownColumnTypes.includes(type); }
  function showColumnsOfTypes(types: string[]) {
    const columns = result?.columns ?? [];
    if (!columns.some((column) => types.includes(column.type) || isColumnProtected(column.name))) return;
    hiddenColumns = columns.filter((column) => !types.includes(column.type) && !isColumnProtected(column.name)).map((column) => column.name);
    lastHiddenColumn = null;
  }
  function toggleShownType(type: string, checked: boolean) {
    const selected = shownColumnTypes.length ? shownColumnTypes : [...columnTypes];
    const next = checked ? [...new Set([...selected, type])] : selected.filter((item) => item !== type);
    if (!next.length) return;
    shownColumnTypes = next.length === columnTypes.length ? [] : next;
    showColumnsOfTypes(next);
  }
  function toggleColumn(column: string, checked: boolean) { if (checked) restoreColumn(column); else hideColumn(column); }
  async function changePage(next: number) { if (next < 1 || next > totalPages || next === page) return; page = next; await loadActiveData(); }
  async function jumpPage() { const next = Math.min(Math.max(1, Number.parseInt(pageInput) || 1), Math.max(totalPages, 1)); pageInput = String(next); await changePage(next); }
  async function changePageSize(event: Event) { pageSize = Number((event.currentTarget as HTMLSelectElement).value); page = 1; await loadActiveData(); }

  async function openStats(column: ColumnInfo, trigger?: HTMLButtonElement) {
    if (!column.profile_kind) return;
    distributionMode = 'count';
    cumulativeDistribution = false;
    closeInspector();
    const id = ++statsRequestId;
    inspectorTrigger = trigger ?? null;
    inspectorMode = 'profile';
    statsColumn = column;
    stats = null;
    statsError = '';
    statsLoading = true;
    binReadout = 'Focus a bin to read its range and count.';
    await tick();
    focusInspector();
    try {
      const next = queryMode === 'sql'
        ? await api.getSqlColumnStats(selectedNodeId, column.name, { sql: sqlBase || activeSql, page, page_size: pageSize, filters, sorts, dedupe_columns: dedupeColumns })
        : await api.getColumnStats(selectedNodeId, selectedDataset, column.name, { page, page_size: pageSize, filters, sorts, dedupe_columns: dedupeColumns });
      if (id === statsRequestId) stats = next;
    } catch (reason) { if (id === statsRequestId) statsError = message(reason); }
    finally { if (id === statsRequestId) statsLoading = false; }
  }

  function display(value: unknown): string { return typeof value === 'object' && value !== null ? JSON.stringify(value) : String(value); }
  type NumberPart = { type: string; value: string };
  function numberParts(value: number): NumberPart[] { return (new Intl.NumberFormat() as unknown as { formatToParts(value: number): NumberPart[] }).formatToParts(value); }
  function numberSymbols() { const parts = numberParts(1234.5); return { group: parts.find((part) => part.type === 'group')?.value ?? ',', decimal: parts.find((part) => part.type === 'decimal')?.value ?? '.' }; }
  function digitMaps() {
    const localized = numberParts(9876543210).filter((part) => part.type === 'integer').map((part) => part.value).join('');
    return { toAscii: new Map([...localized].map((digit, index) => [digit, String(9 - index)])), fromAscii: new Map([...localized].map((digit, index) => [String(9 - index), digit])) };
  }
  function normalizeDigits(value: string, map = digitMaps().toAscii) { return [...value].map((digit) => map.get(digit) ?? digit).join(''); }
  function normalizedNumber(value: string): string | null {
    const { group, decimal } = numberSymbols();
    const compact = value.trim().replace(/[\s_']/g, '');
    const sign = compact.startsWith('-') || compact.startsWith('+') ? compact[0] : '';
    const magnitude = compact.slice(sign.length);
    const parts = magnitude.split(decimal);
    if (parts.length > 2) return null;
    const [integerSource, fractionSource] = parts;
    const integer = normalizeDigits(integerSource).split(group).join('');
    const fraction = fractionSource === undefined ? undefined : normalizeDigits(fractionSource);
    if ((!/^\d+$/.test(integer) && !(integer === '' && fraction)) || (fraction !== undefined && !/^\d*$/.test(fraction))) return null;
    if (integerSource.includes(group)) {
      const canonical = numberParts(Number(integer)).filter((part) => part.type === 'integer' || part.type === 'group').map((part) => part.value).join('');
      if (normalizeDigits(integerSource) !== normalizeDigits(canonical)) return null;
    }
    const normalized = `${sign}${integer}${fraction === undefined ? '' : `.${fraction}`}`;
    return Number.isFinite(Number(normalized)) ? normalized : null;
  }
  function formattedNumber(value: string): string {
    const sign = value.startsWith('-') || value.startsWith('+') ? value[0] : '';
    const [integer, fraction] = value.slice(sign.length).split('.');
    const { decimal } = numberSymbols();
    const { fromAscii } = digitMaps();
    const formattedInteger = numberParts(Number(integer || '0')).filter((part) => part.type === 'integer' || part.type === 'group').map((part) => part.value).join('');
    const formattedFraction = fraction === undefined ? '' : [...fraction].map((digit) => fromAscii.get(digit) ?? digit).join('');
    return `${sign}${formattedInteger}${fraction === undefined ? '' : `${decimal}${formattedFraction}`}`;
  }
  function normalizeNumericFilter() { const value = normalizedNumber(filterValue); if (value !== null) filterValue = formattedNumber(value); }
  async function scrollToColumn(name: string) {
    await tick();
    const header = [...(tableScroll?.querySelectorAll<HTMLTableCellElement>('th[data-column]') ?? [])].find((element) => element.dataset.column === name);
    if (!header || !tableScroll) return;
    tableScroll.scrollTo({ left: Math.max(0, header.offsetLeft - (tableScroll.clientWidth - header.offsetWidth) / 2), behavior: 'smooth' });
  }
  function findColumn() { activeColumnMatch = 0; if (columnMatches.length) scrollToColumn(columnMatches[0].name); }
  function cycleColumnMatch(event: KeyboardEvent) { if (event.key === 'Enter' && columnMatches.length) { event.preventDefault(); activeColumnMatch = (activeColumnMatch + 1) % columnMatches.length; scrollToColumn(columnMatches[activeColumnMatch].name); } }
  function columnLabelParts(name: string): { text: string; match: boolean }[] {
    const query = columnSearch.trim().toLowerCase();
    if (!query) return [{ text: name, match: false }];
    const parts = []; let start = 0; let index = name.toLowerCase().indexOf(query, start);
    while (index !== -1) { if (index > start) parts.push({ text: name.slice(start, index), match: false }); parts.push({ text: name.slice(index, index + query.length), match: true }); start = index + query.length; index = name.toLowerCase().indexOf(query, start); }
    if (start < name.length) parts.push({ text: name.slice(start), match: false });
    return parts;
  }
  function expandCell(event: MouseEvent, row: number, column: string) { const cell = event.currentTarget as HTMLTableCellElement; if (cell.scrollWidth > cell.clientWidth) selectedCell = { row, column }; }
  async function filterCategoricalCell(column: ColumnInfo, value: unknown) {
    if (column.profile_kind !== 'categorical' || (typeof value !== 'string' && typeof value !== 'boolean') || filters.some((filter) => filter.column === column.name && filter.operator === '=' && filter.value === value)) return;
    filters = [...filters, { column: column.name, operator: '=', value }];
    page = 1;
    pageInput = '1';
    await loadData();
  }
  function cellTitle(column: ColumnInfo, value: unknown): string { const text = display(value); return column.profile_kind === 'categorical' && (typeof value === 'string' || typeof value === 'boolean') ? `${text} — Double-click to filter by this value` : text; }
  function toggleCellFromKeyboard(event: KeyboardEvent, row: number, column: string) {
    if (event.key !== 'Enter' && event.key !== ' ') return;
    event.preventDefault();
    if (selectedCell?.row === row && selectedCell.column === column) collapseCell(row, column);
    else expandCell(event as unknown as MouseEvent, row, column);
  }
  function collapseCell(row: number, column: string) { if (selectedCell?.row === row && selectedCell.column === column) selectedCell = null; }
  function sortFor(column: string): SortCondition | undefined { return sorts.find((sort) => sort.column === column); }
  function compact(value: number | string | null | undefined): string { return value == null ? '—' : typeof value === 'string' ? value : new Intl.NumberFormat(undefined, { maximumFractionDigits: 4 }).format(value); }
  function count(value: AggregateCount): string { return typeof value === 'string' ? value : value.toLocaleString(); }
  function distributionText(value: AggregateCount, values: { count: AggregateCount }[], index: number, total: AggregateCount): string {
    const amount = cumulativeDistribution ? (() => { const bigint = (globalThis as unknown as { BigInt(value: number | string): bigint }).BigInt; const sum = values.slice(0, index + 1).reduce((sum, item) => sum + bigint(item.count), bigint(0)); return sum <= bigint(Number.MAX_SAFE_INTEGER) ? Number(sum) : sum.toString(); })() : value;
    return distributionMode === 'percent' ? `${Number(total) === 0 ? '0.0' : (Number(amount) * 100 / Number(total)).toFixed(1)}%` : count(amount);
  }
  // ponytail: numeric page input stops before multiplication loses integer precision; add a BigInt text pager only if a human needs deeper pages.
  function isSafeCount(value: AggregateCount): boolean { return typeof value === 'number' || value.length < 16 || (value.length === 16 && value <= String(Number.MAX_SAFE_INTEGER)); }
  function pageLimit(value: AggregateCount): number { const ceiling = Math.floor(Number.MAX_SAFE_INTEGER / pageSize); return isSafeCount(value) ? Math.min(Number(value), ceiling) : ceiling; }
  function rangeStart(total: AggregateCount): string { return Number(total) === 0 ? '0' : ((page - 1) * pageSize + 1).toLocaleString(); }
  function rangeEnd(total: AggregateCount): string { const end = page * pageSize; return (isSafeCount(total) ? Math.min(end, Number(total)) : end).toLocaleString(); }
  function binLabel(bin: { lower: number | string; upper: number | string }): string { return `${compact(bin.lower)}–${compact(bin.upper)}`; }
  function showBin(bin: { lower: number | string; upper: number | string; count: AggregateCount }) { binReadout = `${binLabel(bin)} · ${count(bin.count)} rows`; }

  // -- view-layer adapters for the atomic component split below; no behavior change --
  let canQuery = $derived(!!result);
  let aggregateMenuLabel = $derived(queryMode === 'sql' ? 'Aggregate builder' : 'Aggregate');
  function typeToggleDisabled(type: string): boolean { return isTypeShown(type) && (columnTypes.length === 1 || (shownColumnTypes.length === 1 && shownColumnTypes[0] === type)); }
  function setCategorySearchLive(value: string) { categorySearch = value; loadCategoryValues(true); }
</script>

<svelte:window onkeydown={handleKeydown} />

<AppShell liveSummary={querySummary}>
  {#snippet titlebar()}
    <TitleBar
      {selectedNode} {currentDataset} {railCollapsed}
      inert={!!inspectorMode || tableExpanded}
      onToggleRailCollapsed={() => railCollapsed = !railCollapsed}
      onOpenRail={() => railOpen = true}
    />
  {/snippet}

  {#snippet rail()}
    <SourceRail
      {nodes} {selectedNodeId} {loadingNodes} {railOpen} collapsed={railCollapsed} {sourceOpen}
      inert={!!inspectorMode || tableExpanded}
      onSelectNode={(id) => selectNode(id)}
      onToggleSource={() => sourceOpen = !sourceOpen}
      onCloseRail={() => railOpen = false}
    >
      {#snippet disclosure()}
        <SourceDisclosure {mutating} {attachPath} onUpload={upload} onAttach={(event) => { event.preventDefault(); attach(); }} setAttachPath={(value) => attachPath = value} />
      {/snippet}
    </SourceRail>
  {/snippet}

  {#snippet main()}
    <main>
      {#if !selectedNodeId}
        <WelcomeScreen {error} {mutating} onUpload={upload} onRetry={loadNodes}>
          {#snippet attachForm()}
            <SourceDisclosure {mutating} {attachPath} onUpload={upload} onAttach={(event) => { event.preventDefault(); attach(); }} setAttachPath={(value) => attachPath = value} idPrefix="onboarding-database-path" />
          {/snippet}
        </WelcomeScreen>
      {:else}
        <section class="workspace">
          <DatasetHead
            title={workspaceTab === 'queries' ? 'Saved queries' : (currentDataset?.name ?? selectedNode?.name ?? '')}
            showMeta={workspaceTab === 'data' && !!result}
            rows={result ? count(result.total_rows) : ''}
            ms={result ? compact(result.elapsed_ms) : ''}
            showRefresh={workspaceTab === 'data'}
            onRefresh={loadActiveData}
            {loadingData}
            inert={!!inspectorMode || tableExpanded}
          />
          <DatasetTabsBar
            {datasets} {selectedDataset} {workspaceTab} {tableExpanded} {rowDensity}
            savedQueryCount={savedQueries.length}
            onSelectDataset={(id) => selectDataset(id)}
            onSelectQueries={() => { closeSql(); workspaceTab = 'queries'; }}
            setRowDensity={(density) => rowDensity = density}
            onToggleExpanded={toggleTableExpanded}
          />
          {#if workspaceTab === 'queries'}
            <SavedQueriesPane {savedQueries} {storageError} onRun={runSavedQuery} onDelete={deleteQuery} />
          {:else}
            {#if error}
              <div class="banner error-banner" role="alert" inert={tableExpanded}><div><strong>Request failed</strong><p>{error}</p></div><button onclick={() => selectedDataset ? loadData() : loadNodes()}>Retry</button></div>
            {/if}
            {#if datasets.length === 0 && !error}
              <div class="banner"><strong>No datasets found</strong><p>This source has no tables or views to browse.</p></div>
            {:else if selectedDataset}
              <QueryConditionBar
                inert={!!inspectorMode || tableExpanded}
                showBuilder={canQuery}
                {filters} {sorts} {dedupeColumns}
                {activeSql} resultSql={result?.sql ?? ''}
                onRemoveFilter={removeFilter} onRemoveSort={removeSort} onClearDedupe={clearDedupe}
                onSaveQuery={() => saveQuery(result?.sql ?? '')} canSaveBuilderQuery={!!result?.sql}
                onClearQuery={clearQuery}
                isSqlMode={queryMode === 'sql'}
                onBackToFullTable={backToBuilder}
                onSaveView={() => saveQuery(activeSql)}
                onBackToBuilder={backToBuilder}
                {columnSearch} setColumnSearch={(value) => columnSearch = value}
                onFindColumn={findColumn} onColumnSearchKeydown={cycleColumnMatch}
                columnMatchCount={columnMatches.length}
                {sqlOpen} onToggleSql={() => sqlOpen ? closeSql() : openSql()}
                setSqlTrigger={(el) => sqlTrigger = el}
                {storageError}
              >
                {#snippet columnsMenu()}
                  <ColumnsMenuPopover
                    open={queryMenuOpen === 'columns'} ontoggle={(event) => syncQueryMenu('columns', event)}
                    visibleCount={visibleColumns.length} totalCount={result?.columns.length ?? 0}
                    {columnMenuSearch} setColumnMenuSearch={(value) => columnMenuSearch = value}
                    {columnTypes} {columnTypeCounts} {isTypeShown} {toggleShownType} {typeToggleDisabled}
                    {nullThreshold} setNullThreshold={(value) => nullThreshold = value}
                    onHideFullyEmpty={() => hideColumnsAtNullFraction(1)}
                    onApplyThreshold={() => hideColumnsAtNullFraction(Math.min(100, Math.max(0, nullThreshold)) / 100)}
                    onShowAll={showAllColumns} hiddenCount={hiddenColumns.length}
                    {columnMenuItems} {hiddenColumns} {isColumnProtected}
                    visibleColumnsLength={visibleColumns.length} onToggleColumn={toggleColumn}
                  />
                {/snippet}
                {#snippet aggregateMenu()}
                  <AggregateMenuPopover
                    open={queryMenuOpen === 'aggregate'} ontoggle={(event) => syncQueryMenu('aggregate', event)}
                    label={aggregateMenuLabel}
                    {aggregateColumnSearch} setAggregateColumnSearch={(value) => aggregateColumnSearch = value}
                    {aggregateColumn} {aggregateColumnMatches} {aggregateColumns}
                    onAddColumn={addAggregateColumn} onRemoveColumn={removeAggregateColumn}
                    {selectedAggregateColumn} availableMetrics={availableAggregateMetrics}
                    {aggregateMetrics} onToggleMetric={toggleAggregateMetric}
                    onCreateView={createAggregateView} creating={loadingData}
                  />
                {/snippet}
                {#snippet dedupeMenu()}
                  <DedupeMenuPopover
                    open={queryMenuOpen === 'dedupe'} ontoggle={(event) => syncQueryMenu('dedupe', event)}
                    label={`Dedupe${dedupeDraft.length ? ` (${dedupeDraft.length})` : ''}`}
                    columns={visibleColumns} {dedupeDraft} onToggle={toggleDedupe}
                    onApply={applyDedupe} onClear={clearDedupe} dedupeAppliedCount={dedupeColumns.length}
                  />
                {/snippet}
              </QueryConditionBar>
              {#if sqlOpen}
                <SqlEditorPanel
                  setEditorHost={(el) => editorHost = el}
                  hasError={!!sqlError} {sqlError}
                  onClose={() => closeSql()}
                  onSave={() => saveQuery(sqlText)} canSave={!!sqlText.trim()}
                  onRun={() => { page = 1; pageInput = '1'; runSql(); }}
                  canRun={!loadingData && !!sqlText.trim()} running={loadingData}
                />
              {/if}
              <div class:expanded={tableExpanded} class="data-stage">
                {#if tableExpanded}
                  <div class="expanded-toolbar">
                    <span>Expanded table</span>
                    <button onclick={toggleTableExpanded}>Back <kbd>Esc</kbd></button>
                  </div>
                {/if}
                <section class="table-pane {rowDensity}" aria-label="Dataset rows" inert={!!inspectorMode}>
                  <div class="table-card" aria-busy={loadingData}>
                    {#if loadingData && !result}
                      <div class="table-state"><span class="spinner"></span>Loading rows…</div>
                    {:else if result && result.rows.length === 0}
                      <div class="table-state"><strong>No matching rows</strong><span>{queryMode === 'sql' ? 'The SQL query returned no rows.' : 'Change or remove filters to see more data.'}</span></div>
                    {:else if result}
                      <DataGridTable
                        columns={visibleColumns} rows={result.rows}
                        caption={`Rows from ${currentDataset?.name ?? selectedDataset}`}
                        {canQuery} {sorts} {filters} {columnLabelParts} {isColumnProtected}
                        onSort={cycleSort}
                        onFilter={(column, trigger) => openFilter(column, trigger)}
                        onProfile={(column, trigger) => openStats(column, trigger)}
                        onHide={hideColumn} {display} {cellTitle}
                        {selectedCell} onExpandCell={expandCell} onFilterCategoricalCell={filterCategoricalCell}
                        onCellKeydown={toggleCellFromKeyboard} onCollapseCell={collapseCell}
                        {aggregateRowTones} setTableScroll={(el) => tableScroll = el}
                      />
                      {#if loadingData}<div class="loading-overlay"><span class="spinner"></span>Refreshing rows…</div>{/if}
                    {/if}
                  </div>
                  {#if result}
                    <PaginationFooter
                      {pageSizes} {pageSize} onChangePageSize={changePageSize}
                      rangeStart={rangeStart(result.total_rows)} rangeEnd={rangeEnd(result.total_rows)} totalRows={count(result.total_rows)}
                      {page} {pageInput} {totalPages} {loadingData}
                      onPrev={() => changePage(page - 1)} onNext={() => changePage(page + 1)}
                      onJump={(event) => { event.preventDefault(); jumpPage(); }}
                      setPageInput={(value) => pageInput = value}
                    />
                  {/if}
                </section>
                {#if inspectorMode}
                  <InspectorPanel
                    title={filterColumn?.name ?? statsColumn?.name ?? ''}
                    subtitle={inspectorMode === 'filter' ? 'Filter column' : 'Column profile'}
                    typeLabel={filterColumn?.type ?? statsColumn?.type ?? ''}
                    onClose={closeInspector}
                    setPanel={(el) => inspector = el}
                  >
                    {#snippet body()}
                      {#if inspectorMode === 'filter' && filterColumn}
                        <FilterInspector
                          column={filterColumn} isText={isTextType(filterColumn.type)}
                          {operators} operator={filterOperator} value={filterValue}
                          setOperator={(value) => filterOperator = value} setValue={(value) => filterValue = value}
                          onblurValue={filterColumn.numeric ? normalizeNumericFilter : undefined}
                          bind:valueInput={filterInput}
                          onSubmitFilter={(event) => { event.preventDefault(); addFilter(); }}
                          {categorySearch} setCategorySearch={setCategorySearchLive}
                          setCategoryInputRef={(el) => filterInput = el}
                          onSearchCategories={(event) => { event.preventDefault(); loadCategoryValues(true); }}
                          {categoryValues} {categoriesLoading} {categoriesError}
                          {categoryTotal} {categoryHasMore} onLoadMore={() => loadCategoryValues(false)}
                          {selectedCategories} onToggleCategory={toggleCategory}
                          onSelectVisible={selectVisibleCategories} onClearSelected={() => selectedCategories = []}
                          {count}
                        />
                      {:else if inspectorMode === 'profile' && statsColumn}
                        <ProfileInspector
                          loading={statsLoading} error={statsError} {stats} {count} {compact}
                          {maxBin} {binReadout} {binLabel} onFocusBin={showBin}
                          {distributionMode} setDistributionMode={(mode) => distributionMode = mode}
                          {cumulativeDistribution} toggleCumulative={() => cumulativeDistribution = !cumulativeDistribution}
                          {distributionText}
                        />
                      {/if}
                    {/snippet}
                    {#snippet footer()}
                      {#if inspectorMode === 'filter' && isTextType(filterColumn?.type ?? '')}
                        <Button onclick={closeInspector}>Cancel</Button>
                        <Button variant="primary" onclick={addCategoryFilter} disabled={selectedCategories.length === 0}>Apply categories</Button>
                      {:else}
                        <Button onclick={closeInspector}>{inspectorMode === 'profile' ? 'Done' : 'Cancel'}</Button>
                      {/if}
                    {/snippet}
                  </InspectorPanel>
                {/if}
              </div>
            {/if}
          {/if}
        </section>
      {/if}
    </main>
  {/snippet}
</AppShell>

{#if workbookPreview}
  <WorkbookDialog
    preview={workbookPreview} selectedSheets={workbookSheets} confirming={confirmingWorkbook}
    setDialog={(el) => workbookDialog = el}
    onClose={discardWorkbook}
    onCancelAttempt={(event) => { if (confirmingWorkbook) event.preventDefault(); }}
    onBackdropClick={(event) => { if (event.target === workbookDialog && !confirmingWorkbook) workbookDialog?.close(); }}
    onToggleSheet={toggleWorkbookSheet}
    onCancel={() => workbookDialog?.close()}
    onConfirm={confirmWorkbook}
  />
{/if}

<style>
  .workspace { flex: 1; min-height: 0; display: flex; flex-direction: column; }
  .data-stage { flex: 1; min-height: 0; display: flex; position: relative; }
  .data-stage.expanded { position: fixed; inset: 20px; z-index: 6; flex-direction: column; overflow: hidden; border: 1px solid var(--line-strong); border-radius: var(--radius-card); background: var(--surface); box-shadow: var(--shadow-panel); }
  .expanded-toolbar { flex: none; height: 40px; padding: 0 10px 0 14px; display: flex; align-items: center; justify-content: space-between; border-bottom: 1px solid var(--line); background: var(--surface-2); font-size: 12px; font-weight: 600; }
  .expanded-toolbar button { height: 28px; padding: 0 9px; border: 1px solid var(--control-border); border-radius: var(--radius-md); background: var(--surface); font-size: 12px; }
  .expanded-toolbar kbd { margin-left: 5px; font: 10px var(--font-mono); color: var(--faint); }
  .table-pane { min-width: 0; min-height: 0; flex: 1; display: flex; flex-direction: column; }
  .table-pane.compact { --row-height: 26px; }
  .table-pane.comfortable { --row-height: 42px; }
  .table-card { position: relative; min-height: 180px; flex: 1; overflow: hidden; }
  .table-state { display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 8px; height: 100%; padding: 40px; text-align: center; color: var(--muted); font-family: var(--font-ui); font-size: 13px; }
  .loading-overlay { position: absolute; inset: 0; z-index: 5; display: flex; align-items: center; justify-content: center; gap: 8px; background: rgba(255, 255, 255, 0.7); font-size: 12.5px; color: var(--muted); }
  .banner { margin: 14px 20px; padding: 12px 14px; border-radius: var(--radius-lg); border: 1px solid var(--line); background: var(--surface-inset); font-size: 12.5px; color: var(--muted); }
  .banner strong { display: block; margin-bottom: 4px; color: var(--ink); font-size: 13px; }
  .banner p { margin: 0; }
  .error-banner { display: flex; align-items: center; justify-content: space-between; gap: 12px; border-color: var(--error); }
  .error-banner strong { color: var(--error); }
  .error-banner button { flex: none; height: 28px; padding: 0 12px; border-radius: var(--radius-md); border: 1px solid var(--control-border); background: var(--surface); }
</style>
