<script lang="ts">
  import { onMount, tick } from 'svelte';
  import { basicSetup, EditorView } from 'codemirror';
  import { sql, StandardSQL, type SQLNamespace } from '@codemirror/lang-sql';
  import * as api from './lib/api';
  import type { AggregateCount, CategoryValue, ColumnInfo, ColumnStats, DatasetInfo, FilterCondition, FilterOperator, NodeInfo, QueryResponse, SortCondition, WorkbookPreview } from './lib/types';

  type SavedQuery = { id: string; name: string; sql: string; nodeId: string; dataset: string };

  const pageSizes = [50, 100, 250, 500, 1000];
  const savedQueriesKey = 'quark.savedQueries';
  const baseOperators: { value: FilterOperator; label: string }[] = [{ value: '=', label: 'equals' }, { value: '!=', label: 'not equal' }, { value: 'is_null', label: 'is null' }, { value: 'not_null', label: 'is not null' }];
  const textOperators: { value: FilterOperator; label: string }[] = [{ value: 'contains', label: 'contains' }, { value: 'starts_with', label: 'starts with' }, { value: 'ends_with', label: 'ends with' }];
  const orderedOperators: { value: FilterOperator; label: string }[] = [{ value: '>', label: 'greater than' }, { value: '>=', label: 'at least' }, { value: '<', label: 'less than' }, { value: '<=', label: 'at most' }];

  let nodes = $state<NodeInfo[]>([]);
  let datasets = $state<DatasetInfo[]>([]);
  let selectedNodeId = $state('');
  let selectedDataset = $state('');
  let result = $state.raw<QueryResponse | null>(null);
  let filters = $state<FilterCondition[]>([]);
  let sorts = $state<SortCondition[]>([]);
  let dedupeColumns = $state<string[]>([]);
  let dedupeDraft = $state<string[]>([]);
  let hiddenColumns = $state<string[]>([]);
  let lastHiddenColumn = $state<string | null>(null);
  let railCollapsed = $state(false);
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
  let inspectorMode = $state<'filter' | 'profile' | null>(null);
  let railOpen = $state(false);
  let sourceOpen = $state(false);
  let binReadout = $state('Focus a bin to read its range and count.');
  let inspectorTrigger: HTMLButtonElement | null = null;
  let inspector = $state<HTMLElement | null>(null);
  let filterInput = $state<HTMLInputElement | null>(null);
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
  let activeSql = $state('');
  let sqlError = $state('');
  let storageError = $state('');
  let savedQueries = $state<SavedQuery[]>([]);
  let editorHost = $state<HTMLDivElement | null>(null);
  let sqlTrigger = $state<HTMLButtonElement | null>(null);
  let editorView: EditorView | null = null;

  let selectedNode = $derived(nodes.find((node) => node.id === selectedNodeId));
  let currentDataset = $derived(datasets.find((dataset) => dataset.id === selectedDataset));
  let totalPages = $derived(pageLimit(result?.total_pages ?? 0));
  let visibleColumns = $derived(result?.columns.filter((column) => !hiddenColumns.includes(column.name)) ?? []);
  let columnMatches = $derived.by(() => { const query = columnSearch.trim().toLowerCase(); return query ? visibleColumns.filter((column) => column.name.toLowerCase().includes(query)) : []; });
  let operators = $derived.by(() => filterColumn ? [...baseOperators, ...(!filterColumn.numeric && isTextType(filterColumn.type) ? textOperators : []), ...(filterColumn.numeric || isOrderedType(filterColumn.type) ? orderedOperators : [])] : baseOperators);
  let maxBin = $derived(stats && stats.kind !== 'categorical' && stats.histogram.length ? Math.max(...stats.histogram.map((bin) => Number(bin.count)), 1) : 1);
  let querySummary = $derived(result ? queryMode === 'sql' ? `${count(result.total_rows)} SQL result rows, page ${result.page} of ${count(result.total_pages)}.` : `${count(result.total_rows)} rows, page ${result.page} of ${count(result.total_pages)}, ${filters.length} filters, ${sorts.length} sorts, and ${dedupeColumns.length} dedupe keys.` : '');

  onMount(() => { loadSavedQueries(); loadNodes(); return () => editorView?.destroy(); });

  function message(reason: unknown): string { return reason instanceof Error ? reason.message : 'Something went wrong'; }
  function isOrderedType(type: string): boolean { return /VARCHAR|CHAR|TEXT|DATE|TIME|INT|DECIMAL|NUMERIC|REAL|FLOAT|DOUBLE/i.test(type); }
  function isTextType(type: string): boolean { return /VARCHAR|CHAR|TEXT/i.test(type); }
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
    const schema: Record<string, Record<string, readonly string[]>> = {};
    for (const dataset of datasets) {
      schema[dataset.schema] ??= {};
      schema[dataset.schema][dataset.name] = dataset.columns;
    }
    return schema;
  }

  function createSqlEditor() {
    editorView?.destroy();
    if (!editorHost) return;
    editorHost.replaceChildren();
    editorView = new EditorView({
      doc: sqlText,
      parent: editorHost,
      extensions: [
        basicSetup,
        sql({ dialect: StandardSQL, schema: editorSchema(), defaultSchema: currentDataset?.schema, defaultTable: currentDataset?.name, upperCaseKeywords: true }),
        EditorView.lineWrapping,
        EditorView.contentAttributes.of({ 'aria-label': 'SQL editor' }),
        EditorView.updateListener.of((update) => { if (update.docChanged) sqlText = update.state.doc.toString(); })
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

  function closeSql() { editorView?.destroy(); editorView = null; sqlOpen = false; tick().then(() => sqlTrigger?.focus()); }
  function resetSql(dataset: DatasetInfo | undefined) { closeSql(); sqlText = seedSql(dataset); activeSql = ''; sqlError = ''; }

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
    hiddenColumns = [];
    lastHiddenColumn = null;
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
    hiddenColumns = [];
    lastHiddenColumn = null;
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

  async function runSql(value = sqlText) {
    const query = value.trim();
    if (!selectedNodeId || !query) { sqlError = 'Enter a SQL query to run.'; return; }
    const id = ++requestId;
    loadingData = true;
    error = '';
    sqlError = '';
    try {
      const next = await api.querySql(selectedNodeId, { sql: query, page, page_size: pageSize });
      if (id !== requestId) return;
      result = next;
      activeSql = query;
      queryMode = 'sql';
      selectedCell = null;
      hiddenColumns = [];
      page = next.page;
      pageInput = String(next.page);
      if (sqlOpen) { await tick(); createSqlEditor(); }
    } catch (reason) { if (id === requestId) sqlError = message(reason); }
    finally { if (id === requestId) loadingData = false; }
  }

  async function loadActiveData() { if (queryMode === 'sql') await runSql(activeSql); else await loadData(); }

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
      const response = await api.getCategoryValues(selectedNodeId, selectedDataset, filterColumn.name, { search: categorySearch.trim(), offset });
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
    const value = noValue ? undefined : filterColumn.numeric ? numericValue! : filterValue;
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
  function toggleDedupe(column: string, checked: boolean) { dedupeDraft = checked ? [...dedupeDraft, column] : dedupeDraft.filter((item) => item !== column); }
  async function applyDedupe() { dedupeColumns = [...dedupeDraft]; page = 1; await loadData(); }
  async function clearDedupe() { dedupeColumns = []; dedupeDraft = []; page = 1; await loadData(); }
  function isColumnProtected(column: string): boolean { return queryMode === 'builder' && [...dedupeColumns, ...dedupeDraft].includes(column); }
  function hideColumn(column: string) { if (!isColumnProtected(column) && visibleColumns.length > 1) { hiddenColumns = [...hiddenColumns, column]; lastHiddenColumn = column; } }
  function restoreColumn(column: string) { hiddenColumns = hiddenColumns.filter((item) => item !== column); if (lastHiddenColumn === column) lastHiddenColumn = null; }
  function toggleColumn(column: string, checked: boolean) { if (checked) restoreColumn(column); else hideColumn(column); }
  async function changePage(next: number) { if (next < 1 || next > totalPages || next === page) return; page = next; await loadActiveData(); }
  async function jumpPage() { const next = Math.min(Math.max(1, Number.parseInt(pageInput) || 1), Math.max(totalPages, 1)); pageInput = String(next); await changePage(next); }
  async function changePageSize(event: Event) { pageSize = Number((event.currentTarget as HTMLSelectElement).value); page = 1; await loadActiveData(); }

  async function openStats(column: ColumnInfo, trigger?: HTMLButtonElement) {
    if (!column.profile_kind) return;
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
      const next = await api.getColumnStats(selectedNodeId, selectedDataset, column.name, { page, page_size: pageSize, filters, sorts, dedupe_columns: dedupeColumns });
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
  function toggleCellFromKeyboard(event: KeyboardEvent, row: number, column: string) {
    if (event.key !== 'Enter' && event.key !== ' ') return;
    event.preventDefault();
    if (selectedCell?.row === row && selectedCell.column === column) collapseCell(row, column);
    else expandCell(event as unknown as MouseEvent, row, column);
  }
  function collapseCell(row: number, column: string) { if (selectedCell?.row === row && selectedCell.column === column) selectedCell = null; }
  function filterDisplay(value: FilterCondition['value']): string { return Array.isArray(value) ? value.join(', ') : String(value ?? ''); }
  function sortFor(column: string): SortCondition | undefined { return sorts.find((sort) => sort.column === column); }
  function compact(value: number | string | null | undefined): string { return value == null ? '—' : typeof value === 'string' ? value : new Intl.NumberFormat(undefined, { maximumFractionDigits: 4 }).format(value); }
  function count(value: AggregateCount): string { return typeof value === 'string' ? value : value.toLocaleString(); }
  // ponytail: numeric page input stops before multiplication loses integer precision; add a BigInt text pager only if a human needs deeper pages.
  function isSafeCount(value: AggregateCount): boolean { return typeof value === 'number' || value.length < 16 || (value.length === 16 && value <= String(Number.MAX_SAFE_INTEGER)); }
  function pageLimit(value: AggregateCount): number { const ceiling = Math.floor(Number.MAX_SAFE_INTEGER / pageSize); return isSafeCount(value) ? Math.min(Number(value), ceiling) : ceiling; }
  function rangeStart(total: AggregateCount): string { return Number(total) === 0 ? '0' : ((page - 1) * pageSize + 1).toLocaleString(); }
  function rangeEnd(total: AggregateCount): string { const end = page * pageSize; return (isSafeCount(total) ? Math.min(end, Number(total)) : end).toLocaleString(); }
  function binLabel(bin: { lower: number | string; upper: number | string }): string { return `${compact(bin.lower)}–${compact(bin.upper)}`; }
  function showBin(bin: { lower: number | string; upper: number | string; count: AggregateCount }) { binReadout = `${binLabel(bin)} · ${count(bin.count)} rows`; }
</script>

<svelte:window onkeydown={handleKeydown} />
<div class:rail-collapsed={railCollapsed} class="app-shell">
  <header class="topbar" inert={!!inspectorMode}>
    <button class="menu-button" aria-label="Open sources" aria-expanded={railOpen} onclick={() => railOpen = true}>☰</button>
    <button class="rail-toggle" aria-label={railCollapsed ? 'Expand sources sidebar' : 'Collapse sources sidebar'} aria-expanded={!railCollapsed} onclick={() => railCollapsed = !railCollapsed}>☰</button>
    <div class="brand"><span aria-hidden="true">Q</span><strong>Quark</strong></div>
    <nav class="breadcrumbs" aria-label="Current location"><span>Sources</span><b>{selectedNode?.name ?? 'Choose a source'}</b>{#if currentDataset}<span>/</span><b>{currentDataset.name}</b>{/if}</nav>
    <div class="connection"><span class="status-dot" aria-hidden="true"></span>Connected</div>
  </header>

  <div class="shell">
    <aside class:open={railOpen} class="rail" aria-label="Sources" inert={!!inspectorMode}>
      <div class="rail-head">
        <button class="primary-button" aria-expanded={sourceOpen} onclick={() => sourceOpen = !sourceOpen}>Add source</button>
        {#if sourceOpen}
          <div class="source-disclosure">
            <label class="upload-button" class:disabled={mutating}>Upload file<input type="file" accept=".csv,.tsv,.parquet,.json,.ndjson,.jsonl,.xlsx,.duckdb,.db" onchange={upload} disabled={mutating} /></label>
            <form onsubmit={(event) => { event.preventDefault(); attach(); }}>
              <label for="database-path">Attach database path</label>
              <div class="input-row"><input id="database-path" bind:value={attachPath} placeholder="/data/example.duckdb" disabled={mutating} /><button type="submit" disabled={mutating || !attachPath.trim()}>Attach</button></div>
            </form>
          </div>
        {/if}
      </div>
      <nav class="nodes" aria-label="Available sources">
        <div class="section-title"><h2>Sources</h2><span>{nodes.length}</span></div>
        {#if loadingNodes}<p class="rail-state">Loading sources…</p>
        {:else if nodes.length === 0}<p class="rail-state">No active sources yet.</p>
        {:else}{#each nodes as node (node.id)}<button class:active={node.id === selectedNodeId} aria-current={node.id === selectedNodeId ? 'page' : undefined} onclick={() => selectNode(node.id)}><span class="status-dot" aria-hidden="true"></span><span><strong>{node.name}</strong><small>{node.kind}</small></span></button>{/each}{/if}
      </nav>
      <footer><span class="status-dot" aria-hidden="true"></span>Backend connected</footer>
    </aside>
    {#if railOpen}<button class="rail-backdrop" aria-label="Close sources" onclick={() => railOpen = false}></button>{/if}

    <main>
      {#if !selectedNodeId}
        <section class="welcome"><div class="welcome-icon">Q</div><h1>Explore local data</h1><p>Open a local file or a read-only DuckDB database. Quark keeps the work on this machine and loads only the page you are viewing.</p><ol class="onboarding-steps"><li><b>1. Add a source</b><span>CSV, TSV, Parquet, JSON, JSONL/NDJSON, XLSX, DuckDB, or DB</span></li><li><b>2. Choose a dataset</b><span>Tables and views appear after the source opens</span></li><li><b>3. Inspect the data</b><span>Filter, profile, hide columns, or dedupe by selected keys</span></li></ol>{#if error}<div class="error" role="alert"><div><strong>Could not load Quark</strong><p>{error}</p></div><button onclick={loadNodes}>Retry</button></div>{/if}<label class="primary-button">Choose a file<input type="file" accept=".csv,.tsv,.parquet,.json,.ndjson,.jsonl,.xlsx,.duckdb,.db" onchange={upload} disabled={mutating} /></label><details class="onboarding-attach"><summary>Attach a local DuckDB database</summary><form onsubmit={(event) => { event.preventDefault(); attach(); }}><label for="onboarding-database-path">Database path<input id="onboarding-database-path" bind:value={attachPath} placeholder="/data/example.duckdb" disabled={mutating} /></label><button class="secondary-button" type="submit" disabled={mutating || !attachPath.trim()}>Attach read-only database</button></form></details></section>
      {:else}
        <section class="workspace">
          <header class="dataset-head" inert={!!inspectorMode}>
            <div><h1>{workspaceTab === 'queries' ? 'Saved queries' : currentDataset?.name ?? selectedNode?.name}</h1>{#if workspaceTab === 'data' && result}<p class="dataset-meta"><span>{count(result.total_rows)} rows</span><span>{compact(result.elapsed_ms)} ms</span></p>{/if}</div>
            {#if workspaceTab === 'data'}<button class="icon-button" onclick={loadActiveData} disabled={loadingData} aria-label="Refresh data" title="Refresh data">↻</button>{/if}
          </header>
          <nav class="dataset-tabs" aria-label="Datasets and saved queries">{#each datasets as dataset (dataset.id)}<button class:active={workspaceTab === 'data' && dataset.id === selectedDataset} aria-current={workspaceTab === 'data' && dataset.id === selectedDataset ? 'page' : undefined} onclick={() => selectDataset(dataset.id)}>{dataset.name}</button>{/each}<button class:active={workspaceTab === 'queries'} aria-current={workspaceTab === 'queries' ? 'page' : undefined} onclick={() => { closeSql(); workspaceTab = 'queries'; }}>Queries ({savedQueries.length})</button></nav>
          {#if workspaceTab === 'queries'}
            <section class="queries-pane" aria-label="Saved queries">
              {#if storageError}<p class="sql-error" role="alert">{storageError}</p>{/if}
              {#each savedQueries as saved (saved.id)}<article><div><h2>{saved.name}</h2><pre>{saved.sql}</pre></div><footer><button class="secondary-button" onclick={() => runSavedQuery(saved)}>Run</button><button class="secondary-button" aria-label={`Delete saved query ${saved.name}`} onclick={() => deleteQuery(saved.id)}>Delete</button></footer></article>{:else}<div class="empty"><strong>No saved queries</strong><p>Save a builder query or SQL statement to keep it in this browser.</p></div>{/each}
            </section>
          {:else}
          {#if error}<div class="error" role="alert"><div><strong>Request failed</strong><p>{error}</p></div><button onclick={() => selectedDataset ? loadData() : loadNodes()}>Retry</button></div>{/if}
          {#if datasets.length === 0 && !error}<div class="empty"><strong>No datasets found</strong><p>This source has no tables or views to browse.</p></div>
          {:else if selectedDataset}
            <section class="querybar" aria-label="Query controls" inert={!!inspectorMode}>
              <details class="query-details"><summary>Columns</summary><div class="detail-list">{#each result?.columns ?? [] as column (column.name)}<label><input type="checkbox" checked={!hiddenColumns.includes(column.name)} disabled={isColumnProtected(column.name) || (!hiddenColumns.includes(column.name) && visibleColumns.length <= 1)} onchange={(event) => toggleColumn(column.name, event.currentTarget.checked)} /> {column.name}</label>{/each}</div></details>
              {#if queryMode === 'builder'}
                <details class="query-details"><summary>Dedupe{dedupeDraft.length ? ` (${dedupeDraft.length})` : ''}</summary><div class="detail-list">{#each visibleColumns as column (column.name)}<label><input type="checkbox" checked={dedupeDraft.includes(column.name)} onchange={(event) => toggleDedupe(column.name, event.currentTarget.checked)} /> {column.name}</label>{/each}<div><button type="button" class="secondary-button" onclick={applyDedupe} disabled={dedupeDraft.length === 0}>Apply</button><button type="button" class="secondary-button" onclick={clearDedupe} disabled={dedupeColumns.length === 0 && dedupeDraft.length === 0}>Clear</button></div></div></details>
                <div class="tokens" aria-label="Active query conditions">{#each filters as filter, index (filter)}<span class="token"><b>{filter.column}</b> {filter.operator.replace(/_/g, ' ')} <span title={filterDisplay(filter.value)}>{filterDisplay(filter.value)}</span><button aria-label={`Remove filter ${filter.column}`} onclick={() => removeFilter(index)}>×</button></span>{/each}{#each sorts as sort, index (sort.column)}<span class="token sort-token"><b>{index + 1}. {sort.column}</b> {sort.direction}<button aria-label={`Remove sort ${sort.column}`} onclick={() => removeSort(sort.column)}>×</button></span>{/each}{#if dedupeColumns.length}<span class="token"><b>Dedupe</b> <span title={dedupeColumns.join(', ')}>{dedupeColumns.join(', ')}</span><button aria-label="Clear dedupe keys" onclick={clearDedupe}>×</button></span>{/if}{#if filters.length === 0 && sorts.length === 0 && dedupeColumns.length === 0}<span class="muted">No filters, sorts, or dedupe applied</span>{/if}</div>
                {#if filters.length || sorts.length || dedupeColumns.length}<button class="secondary-button" onclick={() => saveQuery(result?.sql ?? '')} disabled={!result?.sql}>Save query</button><button class="clear-query" onclick={clearQuery}>Clear query</button>{/if}
              {:else}<span class="tokens muted">SQL result</span><button class="secondary-button" onclick={() => { page = 1; pageInput = '1'; loadData(); }}>Back to builder</button>{/if}
              <input class="column-search" type="search" bind:value={columnSearch} oninput={findColumn} onkeydown={cycleColumnMatch} placeholder="Find column" aria-label="Find column" />
              <span class="column-search-count" aria-live="polite" aria-label={`${columnMatches.length} matching columns`}>{columnMatches.length}</span>
              <button bind:this={sqlTrigger} class="secondary-button" class:active={sqlOpen} aria-expanded={sqlOpen} onclick={() => sqlOpen ? closeSql() : openSql()}>SQL</button>
              {#if storageError}<span class="query-error" role="alert">{storageError}</span>{/if}
            </section>
            {#if sqlOpen}<aside class="sql-panel" aria-labelledby="sql-editor-title"><header><div><strong id="sql-editor-title">SQL query</strong><span>DuckDB SQL</span></div><button class="icon-button" onclick={closeSql} aria-label="Close SQL editor" title="Close SQL editor">×</button></header><div bind:this={editorHost} class:error={!!sqlError} class="sql-editor"></div>{#if sqlError}<p class="sql-error" role="alert">{sqlError}</p>{/if}<footer><button class="secondary-button" onclick={() => saveQuery(sqlText)} disabled={!sqlText.trim()}>Save</button><button class="primary-button" onclick={() => { page = 1; pageInput = '1'; runSql(); }} disabled={loadingData || !sqlText.trim()}>{loadingData ? 'Running…' : 'Run SQL'}</button></footer></aside>{/if}
            <div class="data-stage">
              <section class="table-pane" aria-label="Dataset rows" inert={!!inspectorMode}>
                <div class="table-card" aria-busy={loadingData}>
                  {#if loadingData && !result}<div class="table-state"><span class="spinner"></span>Loading rows…</div>
                  {:else if result && result.rows.length === 0}<div class="table-state"><strong>No matching rows</strong><span>{queryMode === 'sql' ? 'The SQL query returned no rows.' : 'Change or remove filters to see more data.'}</span></div>
                  {:else if result}<!-- svelte-ignore a11y_no_noninteractive_tabindex --><div bind:this={tableScroll} class="table-scroll" role="region" tabindex="0" aria-label="Scrollable dataset table"><table><caption class="sr-only">Rows from {currentDataset?.name ?? selectedDataset}</caption><thead><tr>{#each visibleColumns as column (column.name)}<th data-column={column.name} tabindex="-1" scope="col" style:min-width={`${Math.max(168, Math.min(360, column.name.length * 9 + (column.profile_kind ? 150 : 118)))}px`}><div class="column-head"><div class="column-label"><strong title={column.name}>{#each columnLabelParts(column.name) as part, index (`${part.match}-${part.text}-${index}`)}{#if part.match}<mark>{part.text}</mark>{:else}{part.text}{/if}{/each}</strong><small>{column.type} · {(column.null_fraction * 100).toFixed(1)}% null</small></div><div class="header-actions">{#if queryMode === 'builder'}<button class:sorted={sorts.some((sort) => sort.column === column.name)} onclick={() => cycleSort(column)} aria-label={`Sort ${column.name}`} title={`Sort ${column.name}`}>{sortFor(column.name)?.direction === 'asc' ? '↑' : sortFor(column.name)?.direction === 'desc' ? '↓' : '↕'}</button><button class:filtered={filters.some((filter) => filter.column === column.name)} onclick={(event) => openFilter(column, event.currentTarget as HTMLButtonElement)} aria-label={`Filter ${column.name}`} title={`Filter ${column.name}`}>⌕</button>{#if column.profile_kind}<button onclick={(event) => openStats(column, event.currentTarget as HTMLButtonElement)} aria-label={`Profile column ${column.name}`} title={`Profile column ${column.name}`}>▥</button>{/if}{/if}<button onclick={() => hideColumn(column.name)} disabled={isColumnProtected(column.name) || visibleColumns.length <= 1} aria-label={`Hide column ${column.name}`} title={`Hide column ${column.name}`}>×</button></div><div class="null-gauge" title={`${(column.null_fraction * 100).toFixed(1)}% null`}><span style:width={`${Math.min(100, column.null_fraction * 100)}%`}></span></div></div></th>{/each}</tr></thead><tbody>{#each result.rows as row, index (index)}<tr>{#each visibleColumns as column (column.name)}<td tabindex="0" class:expanded-cell={selectedCell?.row === index && selectedCell.column === column.name} onclick={(event) => expandCell(event, index, column.name)} onkeydown={(event) => toggleCellFromKeyboard(event, index, column.name)} title={selectedCell?.row === index && selectedCell.column === column.name ? undefined : display(row[column.name])}><span>{display(row[column.name])}</span>{#if selectedCell?.row === index && selectedCell.column === column.name}<button class="collapse-cell" onclick={(event) => { event.stopPropagation(); collapseCell(index, column.name); }} aria-label={`Collapse ${column.name}`}>Collapse</button>{/if}</td>{/each}</tr>{/each}</tbody></table></div>{#if loadingData}<div class="loading-overlay"><span class="spinner"></span>Refreshing rows…</div>{/if}{/if}
                </div>
                {#if result}<footer class="pagination"><label>Rows per page <select value={pageSize} onchange={changePageSize}>{#each pageSizes as size (size)}<option value={size}>{size}</option>{/each}</select></label><span class="range">{rangeStart(result.total_rows)}–{rangeEnd(result.total_rows)} of {count(result.total_rows)}</span><div class="pager"><button onclick={() => changePage(page - 1)} disabled={page <= 1 || loadingData} aria-label="Previous page">←</button><form onsubmit={(event) => { event.preventDefault(); jumpPage(); }}><label for="page-number">Page</label><input id="page-number" type="number" min="1" max={Math.max(totalPages, 1)} bind:value={pageInput} /><span>of {count(result.total_pages)}</span></form><button onclick={() => changePage(page + 1)} disabled={page >= totalPages || loadingData} aria-label="Next page">→</button></div></footer>{/if}
              </section>
              {#if inspectorMode}<button class="inspector-backdrop" type="button" tabindex="-1" aria-label="Close inspector" onclick={closeInspector}></button><div bind:this={inspector} class="inspector" role="dialog" aria-modal="true" aria-labelledby="inspector-title"><header><div><p>{inspectorMode === 'filter' ? 'Filter column' : 'Column profile'}</p><h2 id="inspector-title">{filterColumn?.name ?? statsColumn?.name}</h2><small>{filterColumn?.type ?? statsColumn?.type}</small></div><button class="icon-button" onclick={closeInspector} aria-label="Close inspector" title="Close inspector">×</button></header><div class="inspector-body">
                {#if inspectorMode === 'filter' && filterColumn}
                  {#if isTextType(filterColumn.type)}<section class="category-picker" aria-label={`Categories for ${filterColumn.name}`}><form onsubmit={(event) => { event.preventDefault(); loadCategoryValues(true); }}><label>Find a category<input bind:this={filterInput} type="search" bind:value={categorySearch} placeholder="Search values" /></label><button class="secondary-button" type="submit" disabled={categoriesLoading}>Search</button></form><div class="category-actions"><button type="button" onclick={selectVisibleCategories} disabled={categoriesLoading || categoryValues.length === 0}>Select visible</button><button type="button" onclick={() => selectedCategories = []} disabled={selectedCategories.length === 0}>Clear</button><span>{selectedCategories.length} selected</span></div>{#if categoriesLoading && categoryValues.length === 0}<p class="panel-state"><span class="spinner"></span>Loading values…</p>{:else if categoriesError}<p class="panel-state error-text" role="alert">{categoriesError}</p>{:else}<div class="category-list">{#each categoryValues as item (item.value)}<label><input type="checkbox" checked={selectedCategories.includes(item.value)} onchange={(event) => toggleCategory(item.value, event.currentTarget.checked)} /><span title={item.value}>{item.value}</span><small>{count(item.count)}</small></label>{:else}<p class="panel-state">No matching values.</p>{/each}</div><div class="category-page"><span>{categoryValues.length.toLocaleString()} / {count(categoryTotal)}</span>{#if categoryHasMore}<button type="button" onclick={() => loadCategoryValues(false)} disabled={categoriesLoading}>{categoriesLoading ? 'Loading…' : 'Load more'}</button>{/if}</div>{/if}</section><details><summary>Advanced condition</summary><form class="advanced-filter" onsubmit={(event) => { event.preventDefault(); addFilter(); }}><label>Operator<select bind:value={filterOperator}>{#each operators as operator (operator.value)}<option value={operator.value}>{operator.label}</option>{/each}</select></label>{#if filterOperator !== 'is_null' && filterOperator !== 'not_null'}<label>Value<input type="text" bind:value={filterValue} /></label>{/if}<button class="primary-button" type="submit">Apply filter</button></form></details>
                  {:else}<form class="direct-filter" onsubmit={(event) => { event.preventDefault(); addFilter(); }}><label>Operator<select bind:value={filterOperator}>{#each operators as operator (operator.value)}<option value={operator.value}>{operator.label}</option>{/each}</select></label>{#if filterOperator !== 'is_null' && filterOperator !== 'not_null'}<label>Value<input bind:this={filterInput} type="text" inputmode={filterColumn.numeric ? 'decimal' : undefined} bind:value={filterValue} onblur={filterColumn.numeric ? normalizeNumericFilter : undefined} /></label>{/if}<button class="primary-button" type="submit" disabled={filterOperator !== 'is_null' && filterOperator !== 'not_null' && filterValue === ''}>Apply filter</button></form>{/if}
                {:else if inspectorMode === 'profile' && statsColumn}
                  {#if statsLoading}<div class="panel-state"><span class="spinner"></span>Computing statistics…</div>
                  {:else if statsError}<div class="panel-state error-text"><strong>Statistics unavailable</strong><span>{statsError}</span></div>
                  {:else if stats}
                    <dl class="profile-summary"><div><dt>Completeness</dt><dd>{count(stats.non_null_count)} non-null · {count(stats.null_count)} null</dd></div></dl>
                    {#if stats.kind === 'numeric'}
                      <dl class="profile-summary"><div><dt>Range</dt><dd>{compact(stats.min)} — {compact(stats.max)}</dd></div><div><dt>Center</dt><dd>mean {compact(stats.mean)} · median {compact(stats.median)}</dd></div><div><dt>Spread</dt><dd>σ {compact(stats.stddev)} · P25 {compact(stats.p25)} · P75 {compact(stats.p75)}</dd></div></dl>
                      <section class="histogram"><header><h3>Distribution</h3><span>{stats.histogram.length} bins</span></header>{#if stats.histogram.length}<div class="bars">{#each stats.histogram as bin (bin.lower)}<button style:height={`${Math.max(3, (Number(bin.count) / maxBin) * 100)}%`} onclick={() => showBin(bin)} onfocus={() => showBin(bin)} aria-label={`${binLabel(bin)}: ${count(bin.count)} rows`} title={`${binLabel(bin)}: ${count(bin.count)} rows`}></button>{/each}</div><p class="bin-readout" aria-live="polite">{binReadout}</p>{:else}<p class="muted">No values to chart.</p>{/if}</section>
                    {:else if stats.kind === 'categorical'}
                      <dl class="profile-summary"><div><dt>Distinct values</dt><dd>{count(stats.distinct_count)}</dd></div></dl>
                      <section class="profile-values"><header><h3>Top values</h3></header>{#each stats.top_values as value (value.value)}<div><span title={display(value.value)}>{display(value.value)}</span><b>{count(value.count)}</b></div>{:else}<p class="muted">No values to show.</p>{/each}</section>
                    {:else}
                      <dl class="profile-summary"><div><dt>Range</dt><dd>{compact(stats.min)} — {compact(stats.max)}</dd></div><div><dt>Distinct values</dt><dd>{count(stats.distinct_count)}</dd></div></dl>
                      <section class="histogram"><header><h3>Distribution</h3><span>{stats.histogram.length} bins</span></header>{#if stats.histogram.length}<div class="bars">{#each stats.histogram as bin (bin.lower)}<button style:height={`${Math.max(3, (Number(bin.count) / maxBin) * 100)}%`} onclick={() => showBin(bin)} onfocus={() => showBin(bin)} aria-label={`${binLabel(bin)}: ${count(bin.count)} rows`} title={`${binLabel(bin)}: ${count(bin.count)} rows`}></button>{/each}</div><p class="bin-readout" aria-live="polite">{binReadout}</p>{:else}<p class="muted">No values to chart.</p>{/if}</section>
                    {/if}
                  {:else}<div class="panel-state">No statistics available.</div>{/if}
                {/if}
              </div><footer class="inspector-footer">{#if inspectorMode === 'filter' && isTextType(filterColumn?.type ?? '')}<button class="secondary-button" onclick={closeInspector}>Cancel</button><button class="primary-button" onclick={addCategoryFilter} disabled={selectedCategories.length === 0}>Apply categories</button>{:else}<button class="secondary-button" onclick={closeInspector}>{inspectorMode === 'profile' ? 'Done' : 'Cancel'}</button>{/if}</footer></div>{/if}
            </div>
          {/if}
          {/if}
        </section>
      {/if}
    </main>
  </div>
  <p class="sr-only" aria-live="polite">{querySummary}</p>
</div>
{#if workbookPreview}
  <dialog bind:this={workbookDialog} aria-labelledby="workbook-title" onclose={discardWorkbook} oncancel={(event) => { if (confirmingWorkbook) event.preventDefault(); }} onclick={(event) => { if (event.target === workbookDialog && !confirmingWorkbook) workbookDialog?.close(); }}>
    <section class="workbook-dialog">
      <h2 id="workbook-title">Workbook detected</h2>
      <p>Choose the worksheets to add before continuing.</p>
      <div class="workbook-sheet-list">
        {#each workbookPreview.sheets as sheet (sheet)}
          <label><input type="checkbox" checked={workbookSheets.includes(sheet)} onchange={(event) => toggleWorkbookSheet(sheet, event.currentTarget.checked)} disabled={confirmingWorkbook} /> {sheet}</label>
        {/each}
      </div>
      <p class="workbook-count">{workbookSheets.length} of {workbookPreview.sheets.length} selected</p>
      <footer><button class="secondary-button" onclick={() => workbookDialog?.close()} disabled={confirmingWorkbook}>Cancel</button><button class="primary-button" onclick={confirmWorkbook} disabled={confirmingWorkbook || workbookSheets.length === 0}>{confirmingWorkbook ? 'Adding…' : 'Continue'}</button></footer>
    </section>
  </dialog>
{/if}
