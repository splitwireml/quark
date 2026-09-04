<script lang="ts">
  import { onMount, tick } from 'svelte';
  import { basicSetup, EditorView } from 'codemirror';
  import { autocompletion, completionStatus, startCompletion, type CompletionSource } from '@codemirror/autocomplete';
  import { keywordCompletionSource, schemaCompletionSource, sql, StandardSQL, type SQLConfig, type SQLNamespace } from '@codemirror/lang-sql';
  import { keymap } from '@codemirror/view';
  import * as api from './lib/api';
  import { buildAggregateSql } from './lib/aggregate-sql';
  import { buildJoinSql } from './lib/join-sql';
  import { buildCellEditSql, buildColumnReplacementSql, buildMutationSql, hasVolatileRowOrder, nextDuplicateColumnName, quoteIdentifier } from './lib/mutation-sql';
  import { LEGACY_STORAGE_KEY, LEGACY_VERSIONING_STORAGE_KEY, VERSIONING_STORAGE_KEY, activateVersion, createSourceHistory, createView, finalizeVersion, matchColumnsByRegex, migrateDatasetHistories, migrateSavedQueries, rebindLegacyHistories, stageVersionChange, versionDiff, versionLabel as formatVersionLabel } from './lib/versioning';
  import type { AggregateCount, AggregateMetric, BaseViewInfo, CategoryValue, ColumnInfo, ColumnStats, DatasetVersionHistory, DistributionMode, ExportFormat, ExportOption, FilterCondition, FilterOperator, JoinWorkspaceRequest, JoinWorkspaceResponse, NodeInfo, ProjectInfo, QueryResponse, RowDensity, SerializableValue, SortCondition, SourceSummary, Version, VersionChange, VersionDiff, ViewHistory, WorkbookPreview } from './lib/types';

  import Button from './components/atoms/Button.svelte';
  import TitleBar from './components/organisms/TitleBar.svelte';
  import SourceRail from './components/organisms/SourceRail.svelte';
  import SourceDisclosure from './components/organisms/SourceDisclosure.svelte';
  import WelcomeScreen from './components/organisms/WelcomeScreen.svelte';
  import DatasetHead from './components/organisms/DatasetHead.svelte';
  import DatasetTabsBar from './components/organisms/DatasetTabsBar.svelte';
  import VersionsViewsPane from './components/organisms/VersionsViewsPane.svelte';
  import VersionDiffDialog from './components/organisms/VersionDiffDialog.svelte';
  import QueryConditionBar from './components/organisms/QueryConditionBar.svelte';
  import ColumnsMenuPopover from './components/organisms/ColumnsMenuPopover.svelte';
  import JoinMenuPopover from './components/organisms/JoinMenuPopover.svelte';
  import AggregateMenuPopover from './components/organisms/AggregateMenuPopover.svelte';
  import DedupeMenuPopover from './components/organisms/DedupeMenuPopover.svelte';
  import SqlEditorPanel from './components/organisms/SqlEditorPanel.svelte';
  import DataGridTable from './components/organisms/DataGridTable.svelte';
  import PaginationFooter from './components/organisms/PaginationFooter.svelte';
  import InspectorPanel from './components/organisms/InspectorPanel.svelte';
  import FilterInspector from './components/organisms/FilterInspector.svelte';
  import ProfileInspector from './components/organisms/ProfileInspector.svelte';
  import WorkbookDialog from './components/organisms/WorkbookDialog.svelte';
  import FormulaMenu from './components/organisms/FormulaMenu.svelte';
  import ExportMenu from './components/organisms/ExportMenu.svelte';
  import ProjectsScreen from './components/organisms/ProjectsScreen.svelte';
  import AppShell from './components/templates/AppShell.svelte';

  const pageSizes = [50, 100, 250, 500, 1000];

  const baseOperators: { value: FilterOperator; label: string }[] = [{ value: '=', label: 'equals' }, { value: '!=', label: 'not equal' }, { value: 'is_null', label: 'is null' }, { value: 'not_null', label: 'is not null' }];
  const textOperators: { value: FilterOperator; label: string }[] = [{ value: 'contains', label: 'contains' }, { value: 'starts_with', label: 'starts with' }, { value: 'ends_with', label: 'ends with' }];
  const orderedOperators: { value: FilterOperator; label: string }[] = [{ value: '>', label: 'greater than' }, { value: '>=', label: 'at least' }, { value: '<', label: 'less than' }, { value: '<=', label: 'at most' }];
  const aggregateMetricOptions: { value: AggregateMetric; label: string; numeric?: true; ordered?: true }[] = [{ value: 'count', label: 'Count' }, { value: 'distinct', label: 'Distinct' }, { value: 'min', label: 'Min', ordered: true }, { value: 'max', label: 'Max', ordered: true }, { value: 'sum', label: 'Sum', numeric: true }, { value: 'avg', label: 'Average', numeric: true }, { value: 'median', label: 'Median', numeric: true }, { value: 'stddev', label: 'Std. dev.', numeric: true }];
  type CellMove = 'up' | 'down' | 'left' | 'right' | 'rowStart' | 'rowEnd' | 'pageUp' | 'pageDown' | 'gridStart' | 'gridEnd';


  let projects = $state.raw<ProjectInfo[]>([]);
  let activeProject = $state.raw<ProjectInfo | null>(null);
  let projectName = $state('');
  let loadingProjects = $state(true);
  let creatingProject = $state(false);
  let projectError = $state('');
  let nodes = $state.raw<SourceSummary[]>([]);
  let datasets = $state.raw<BaseViewInfo[]>([]);
  let loadedSourceIds = $state.raw<string[]>([]);
  let loadingSourceId = $state('');
  let highlightToken = $state(0);
  let selectedNodeId = $state('');
  let selectedDataset = $state('');
  let result = $state.raw<QueryResponse | null>(null);
  let filters = $state<FilterCondition[]>([]);
  let sorts = $state<SortCondition[]>([]);
  let dedupeColumns = $state<string[]>([]);
  let dedupeDraft = $state<string[]>([]);
  let aggregateColumnSearch = $state('');
  let aggregateColumns = $state<string[]>([]);
  let aggregateFieldMetrics = $state<Record<string, AggregateMetric[]>>({});
  let focusedAggregateColumn = $state('');
  let aggregateSourceSql = $state('');
  let aggregateSourceColumns = $state.raw<ColumnInfo[]>([]);
  let joinLeftViewId = $state('');
  let joinRightViewId = $state('');
  let joinLeftKeys = $state<string[]>([]);
  let joinRightKeys = $state<string[]>([]);
  let joinLeftColumns = $state<string[]>([]);
  let joinRightColumns = $state<string[]>([]);
  let joinPreview = $state.raw<JoinWorkspaceResponse | null>(null);
  let joinPreviewLoading = $state(false);
  let joinPreviewError = $state('');
  let hiddenColumns = $state<string[]>([]);
  let columnOrder = $state<string[]>([]);
  let reorderOrigin = $state.raw<string[] | null>(null);
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
  let selectedCell = $state<{ row: number; column: string; expanded?: boolean } | null>(null);
  let editingCell = $state<{ row: number; column: string; value: string; original: string } | null>(null);
  let cellEditSaving = $state(false);
  let cellEditError = $state('');
  let columnMutationError = $state('');
  let renamingColumn = $state<{ original: string; value: string } | null>(null);
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
  let sourceRequestId = 0;
  let joinPreviewRequestId = 0;
  let categoryRequestId = 0;
  let statsRequestId = 0;
  let replayRequestId = 0;
  let workbookDialog = $state<HTMLDialogElement | null>(null);
  let workbookPreview = $state<WorkbookPreview | null>(null);
  let workbookSheets = $state<string[]>([]);
  let confirmingWorkbook = $state(false);
  let mutationDialog = $state<HTMLDialogElement | null>(null);
  let mutationTarget = $state<{ kind: 'insert'; insertIndex: number; left: string; right: string | null; trigger: HTMLButtonElement } | { kind: 'modify'; column: ColumnInfo } | null>(null);
  let mutationApplying = $state(false);
  let mutationError = $state('');
  let exportOpen = $state(false);
  let exportFormat = $state<ExportFormat>('csv');
  let exportOptions = $state.raw<ExportOption[]>([]);
  let exportSelectedKeys = $state<string[]>(['current']);
  let exportLoading = $state(false);
  let exporting = $state(false);
  let exportError = $state('');
  let exportTrigger = $state<HTMLButtonElement | null>(null);
  let exportRequestId = 0;
  let workspaceTab = $state<'data' | 'history'>('data');

  let queryMode = $state<'builder' | 'sql'>('builder');
  let sqlOpen = $state(false);
  let sqlText = $state('');
  let sqlBase = $state('');
  let activeSql = $state('');
  let activeSqlNodeId = $state('');
  let sqlError = $state('');
  let storageError = $state('');
  let recordingNotice = $state('');
  let versionHistories = $state.raw<ViewHistory[]>([]);
  let activeJoin = $state.raw<JoinWorkspaceRequest | undefined>(undefined);
  let openDiff = $state.raw<VersionDiff | null>(null);
  let diffDialog = $state<HTMLDialogElement | null>(null);
  let diffReturnFocus: HTMLElement | null = null;
  let editorHost = $state<HTMLDivElement | null>(null);
  let editorView: EditorView | null = null;
  let queryMenuOpen = $state<'columns' | 'joins' | 'aggregate' | 'dedupe' | null>(null);

  let currentHistory = $derived(versionHistories.find((history) => history.id === selectedDataset));
  let selectedNode = $derived(nodes.find((node) => node.id === currentHistory?.sourceId));
  let selectedSourceId = $derived(currentHistory?.sourceId ?? '');
  let currentDataset = $derived(datasets.find((dataset) => dataset.id === selectedDataset));
  let activeProjectId = $derived(activeProject?.id ?? '');
  let projectViews = $derived(activeProjectId ? versionHistories.filter((history) => history.projectId === activeProjectId) : []);
  let activeVersion = $derived(currentHistory?.versions.find((version) => version.id === currentHistory.activeVersionId));
  let activeVersionChildren = $derived(activeVersion ? currentHistory?.versions.filter((version) => version.parentId === activeVersion.id) ?? [] : []);
  let versionLabel = $derived(workspaceTab === 'data' && activeVersion ? `${formatVersionLabel(activeVersion)} · ${currentHistory?.versions.length ?? 0} saved` : '');
  let canPreviousVersion = $derived(workspaceTab === 'data' && !loadingData && !!activeVersion?.parentId);
  let canNextVersion = $derived(workspaceTab === 'data' && !loadingData && activeVersionChildren.length === 1);
  let currentExportOption = $derived(result ? {
    key: 'current',
    node_id: activeSqlNodeId || activeProject?.node_id || selectedNodeId,
    name: currentHistory?.name ?? 'Current View',
    source: currentHistory?.kind === 'derived' ? 'Derived Views' : selectedNode?.name ?? 'Source Views',
    sql: result.sql
  } : null);
  let joinLeftView = $derived(projectViews.find((view) => view.id === joinLeftViewId));
  let joinRightView = $derived(projectViews.find((view) => view.id === joinRightViewId));
  let joinLeftVersion = $derived(joinLeftView?.versions.find((version) => version.id === joinLeftView?.activeVersionId));
  let joinRightVersion = $derived(joinRightView?.versions.find((version) => version.id === joinRightView?.activeVersionId));
  let joinKeyPairs = $derived(joinLeftKeys.map((left, index) => ({ left, right: joinRightKeys[index] ?? '' })));
  let joinKeysValid = $derived(joinLeftKeys.length > 0 && joinLeftKeys.length === joinRightKeys.length);
  let canPreviewJoin = $derived(!!activeProject && !!joinLeftVersion && !!joinRightVersion);
  let canRunJoin = $derived(canPreviewJoin && joinKeysValid && (joinLeftColumns.length > 0 || joinRightColumns.length > 0));
  let totalPages = $derived(pageLimit(result?.total_pages ?? 0));
  let orderedColumns = $derived.by(() => {
    const columns = result?.columns ?? [];
    const byName = new Map(columns.map((column) => [column.name, column]));
    const names = [...columnOrder, ...columns.map((column) => column.name)];
    return names.filter((name, index) => byName.has(name) && names.indexOf(name) === index).map((name) => byName.get(name)!);
  });
  let visibleColumns = $derived(orderedColumns.filter((column) => !hiddenColumns.includes(column.name)));
  let rowColumns = $derived.by(() => {
    if (!reorderOrigin) return visibleColumns;
    const byName = new Map((result?.columns ?? []).map((column) => [column.name, column]));
    return reorderOrigin.map((name) => byName.get(name)).filter((column): column is ColumnInfo => !!column && !hiddenColumns.includes(column.name));
  });
  let aggregateFieldOptions = $derived((aggregateSourceColumns.length ? aggregateSourceColumns : result?.columns ?? []).filter((column) => column.profile_kind !== null));
  let aggregateColumnMatches = $derived.by(() => { const query = aggregateColumnSearch.trim().toLowerCase(); return query ? aggregateFieldOptions.filter((column) => column.name.toLowerCase().includes(query)) : aggregateFieldOptions; });
  let aggregateFields = $derived(aggregateColumns.filter(isAggregateField));
  let aggregateIndexes = $derived(aggregateColumns.filter((column) => !isAggregateField(column)));
  let aggregateMetrics = $derived(focusedAggregateColumn ? aggregateFieldMetrics[focusedAggregateColumn] ?? [] : []);
  let selectedAggregateColumn = $derived(aggregateFieldOptions.find((column) => column.name === focusedAggregateColumn));
  let availableAggregateMetrics = $derived(aggregateMetricOptions.filter((metric) => (!metric.numeric || selectedAggregateColumn?.numeric) && (!metric.ordered || selectedAggregateColumn?.numeric || selectedAggregateColumn?.profile_kind === 'date')));
  let columnMatches = $derived.by(() => { const query = columnSearch.trim().toLowerCase(); return query ? visibleColumns.filter((column) => column.name.toLowerCase().includes(query)) : []; });
  let columnMenuItems = $derived.by(() => { const query = columnMenuSearch.trim().toLowerCase(); return query ? orderedColumns.filter((column) => column.name.toLowerCase().includes(query)) : orderedColumns; });
  let columnTypes = $derived([...new Set((result?.columns ?? []).map((column) => column.type))]);
  let columnTypeCounts = $derived.by(() => { const counts: Record<string, number> = Object.create(null); for (const column of result?.columns ?? []) counts[column.type] = (counts[column.type] ?? 0) + 1; return counts; });
  let aggregateRowTones = $derived.by(() => {
    const rows = result?.rows ?? [];
    const majorIndex = queryMode === 'sql' && aggregateSourceSql ? aggregateIndexes[0] : '';
    if (!majorIndex) return [];
    let alternate = false;
    return rows.map((row, index) => { if (index && !Object.is(row[majorIndex], rows[index - 1][majorIndex])) alternate = !alternate; return alternate; });
  });
  let operators = $derived.by(() => filterColumn ? [...baseOperators, ...(!filterColumn.numeric && isTextType(filterColumn.type) ? textOperators : []), ...(filterColumn.numeric || isOrderedType(filterColumn.type) ? orderedOperators : [])] : baseOperators);
  let maxBin = $derived(stats && stats.kind !== 'categorical' && stats.histogram.length ? Math.max(...stats.histogram.map((bin) => Number(bin.count)), 1) : 1);
  let querySummary = $derived(result ? queryMode === 'sql' ? `${count(result.total_rows)} SQL result rows, page ${result.page} of ${count(result.total_pages)}.` : `${count(result.total_rows)} rows, page ${result.page} of ${count(result.total_pages)}, ${filters.length} filters, ${sorts.length} sorts, and ${dedupeColumns.length} dedupe keys.` : '');

  onMount(() => { loadVersioning(); void loadProjects(); return () => editorView?.destroy(); });

  function message(reason: unknown): string { return reason instanceof Error ? reason.message : 'Something went wrong'; }
  function isOrderedType(type: string): boolean { return /VARCHAR|CHAR|TEXT|DATE|TIME|INT|DECIMAL|NUMERIC|REAL|FLOAT|DOUBLE/i.test(type); }
  function isTextType(type: string): boolean { return /VARCHAR|CHAR|TEXT/i.test(type); }
  function isBooleanType(type: string): boolean { return type.toLowerCase() === 'boolean'; }
  function isAggregateField(column: string): boolean { return Object.prototype.hasOwnProperty.call(aggregateFieldMetrics, column); }
  function filterSummary(filter: FilterCondition): string {
    const labels: Record<FilterOperator, string> = { '=': 'equals', '!=': 'does not equal', in: 'is one of', is_null: 'is null', not_null: "isn't null", contains: 'contains', starts_with: 'starts with', ends_with: 'ends with', '>': 'is greater than', '>=': 'is at least', '<': 'is less than', '<=': 'is at most' };
    if (filter.operator === 'is_null' || filter.operator === 'not_null') return `${filter.column} ${labels[filter.operator]}`;
    const values = Array.isArray(filter.value) ? filter.value : [filter.value];
    const text = `${values.slice(0, 3).map(String).join(', ')}${values.length > 3 ? `, +${values.length - 3} more` : ''}`;
    return `${filter.column} ${labels[filter.operator]} ${text.length > 48 ? `${text.slice(0, 47)}…` : text}`;
  }
  function syncQueryMenu(menu: 'columns' | 'joins' | 'aggregate' | 'dedupe', event: Event) {
    const open = (event.currentTarget as HTMLDetailsElement).open;
    queryMenuOpen = open ? menu : queryMenuOpen === menu ? null : queryMenuOpen;
  }
  function clearAggregateDraft() { aggregateColumnSearch = ''; aggregateColumns = []; aggregateFieldMetrics = {}; focusedAggregateColumn = ''; aggregateSourceSql = ''; aggregateSourceColumns = []; if (queryMenuOpen === 'aggregate') queryMenuOpen = null; }
  function clearJoinPreview() { joinPreviewRequestId++; joinPreview = null; joinPreviewLoading = false; joinPreviewError = ''; }
  function clearJoinDraft() {
    clearJoinPreview();
    joinLeftViewId = currentHistory?.id ?? '';
    joinRightViewId = '';
    joinLeftKeys = [];
    joinRightKeys = [];
    joinLeftColumns = [...(activeVersion?.columns ?? [])];
    joinRightColumns = [];
    if (queryMenuOpen === 'joins') queryMenuOpen = null;
  }
  function selectJoinView(side: 'left' | 'right', id: string) {
    clearJoinPreview();
    const view = projectViews.find((item) => item.id === id);
    const columns = view?.versions.find((version) => version.id === view.activeVersionId)?.columns ?? [];
    if (side === 'left') { joinLeftViewId = id; joinLeftColumns = [...columns]; }
    else { joinRightViewId = id; joinRightColumns = [...columns]; }
    const left = side === 'left' ? columns : joinLeftVersion?.columns ?? [];
    const right = side === 'right' ? columns : joinRightVersion?.columns ?? [];
    const common = left.find((column) => right.includes(column)) ?? '';
    joinLeftKeys = common ? [common] : [];
    joinRightKeys = common ? [common] : [];
  }
  function setJoinKeys(side: 'left' | 'right', columns: string[]) { clearJoinPreview(); if (side === 'left') joinLeftKeys = columns; else joinRightKeys = columns; }
  function toggleJoinColumn(side: 'left' | 'right', column: string, checked: boolean) {
    clearJoinPreview();
    if (side === 'left') joinLeftColumns = checked ? [...joinLeftColumns, column] : joinLeftColumns.filter((item) => item !== column);
    else joinRightColumns = checked ? [...joinRightColumns, column] : joinRightColumns.filter((item) => item !== column);
  }
  function selectJoinColumns(side: 'left' | 'right', columns: string[]) { clearJoinPreview(); if (side === 'left') joinLeftColumns = columns; else joinRightColumns = columns; }
  function joinRequest(): JoinWorkspaceRequest | null {
    if (!activeProject || !joinLeftView || !joinRightView || !joinLeftVersion || !joinRightVersion) return null;
    return {
      left: { node_id: activeProject.node_id, sql: joinLeftVersion.sql, name: joinLeftView.name },
      right: { node_id: activeProject.node_id, sql: joinRightVersion.sql, name: joinRightView.name },
      left_keys: [...joinLeftKeys],
      right_keys: [...joinRightKeys]
    };
  }
  async function checkJoin(): Promise<JoinWorkspaceResponse | null> {
    const views = [joinLeftView, joinRightView];
    if (activeProject && views.some((view) => view?.kind === 'derived') && loadedSourceIds.length < nodes.length) {
      // ponytail: arbitrary derived SQL has no source dependency manifest.
      if (!await loadAllProjectSources(activeProject)) return null;
    } else for (const view of views) {
      if (view?.kind === 'source' && view.sourceId && !loadedSourceIds.includes(view.sourceId)) {
        if (!await loadProjectSource(view.sourceId, '', false)) return null;
      }
    }
    const request = joinRequest();
    if (!request) { joinPreviewError = 'Choose two Views to check the join.'; return null; }
    const id = ++joinPreviewRequestId;
    joinPreview = null;
    joinPreviewLoading = true;
    joinPreviewError = '';
    try {
      const next = await api.previewJoinWorkspace(request);
      if (id !== joinPreviewRequestId) return null;
      joinPreview = next;
      return next;
    } catch (reason) { if (id === joinPreviewRequestId) joinPreviewError = message(reason); return null; }
    finally { if (id === joinPreviewRequestId) joinPreviewLoading = false; }
  }
  async function runJoin() {
    const request = joinRequest();
    if (!canRunJoin || !request || !joinLeftView || !joinRightView || !joinLeftVersion || !joinRightVersion) return;
    const preview = joinPreview ?? await checkJoin();
    if (!preview) return;
    const query = buildJoinSql(
      { name: joinLeftView.name, sql: joinLeftVersion.sql },
      { name: joinRightView.name, sql: joinRightVersion.sql },
      joinKeyPairs, joinLeftColumns, joinRightColumns
    );
    if (!query) return;
    page = 1;
    pageInput = '1';
    sqlText = query;
    queryMenuOpen = null;
    closeSql(false);
    if (await runSql(query, false, false, preview.node_id)) {
      activeJoin = request;
      addView(result?.sql ?? query, request, `${joinLeftView.name} + ${joinRightView.name}`);
    }
  }
  function toggleAggregateColumn(column: string, checked: boolean) {
    if (checked) {
      if (!column || aggregateColumns.includes(column)) return;
      aggregateColumns = [...aggregateColumns, column];
      if (aggregateColumns.length === 1) {
        aggregateFieldMetrics = { ...aggregateFieldMetrics, [column]: ['count'] };
        focusedAggregateColumn = column;
      }
      return;
    }
    removeAggregateColumn(column);
  }
  function removeAggregateColumn(column: string) {
    aggregateColumns = aggregateColumns.filter((item) => item !== column);
    const metrics = { ...aggregateFieldMetrics };
    delete metrics[column];
    aggregateFieldMetrics = metrics;
    if (focusedAggregateColumn === column) focusedAggregateColumn = aggregateColumns.find(isAggregateField) ?? '';
  }
  function focusAggregate(column: string) { if (isAggregateField(column)) focusedAggregateColumn = column; }
  function toggleAggregateRole(column: string) {
    if (!aggregateColumns.includes(column)) return;
    if (isAggregateField(column)) {
      const metrics = { ...aggregateFieldMetrics };
      delete metrics[column];
      aggregateFieldMetrics = metrics;
      if (focusedAggregateColumn === column) focusedAggregateColumn = aggregateColumns.find((item) => Object.prototype.hasOwnProperty.call(metrics, item)) ?? '';
      return;
    }
    aggregateFieldMetrics = { ...aggregateFieldMetrics, [column]: ['count'] };
    focusedAggregateColumn = column;
  }
  function toggleAggregateMetric(metric: AggregateMetric, checked: boolean) {
    if (!focusedAggregateColumn || !isAggregateField(focusedAggregateColumn)) return;
    const metrics = aggregateFieldMetrics[focusedAggregateColumn];
    aggregateFieldMetrics = { ...aggregateFieldMetrics, [focusedAggregateColumn]: checked ? [...metrics, metric] : metrics.filter((item) => item !== metric) };
  }
  function isWorkbookPreview(node: NodeInfo | WorkbookPreview): node is WorkbookPreview { return node.kind === 'workbook' && 'sheets' in node && Array.isArray(node.sheets); }
  function toggleWorkbookSheet(sheet: string, checked: boolean) { workbookSheets = checked ? [...workbookSheets, sheet] : workbookSheets.filter((item) => item !== sheet); }

  async function openMutation(left: ColumnInfo, right: ColumnInfo | null, trigger: HTMLButtonElement) {
    if (!result || loadingData) return;
    const insertIndex = right ? result.columns.findIndex((column) => column.name === right.name) : result.columns.length;
    if (insertIndex < 0) return;
    mutationTarget = { kind: 'insert', insertIndex, left: left.name, right: right?.name ?? null, trigger };
    mutationError = '';
    mutationApplying = false;
    await tick();
    mutationDialog?.showModal();
    mutationDialog?.querySelector<HTMLInputElement>('input')?.focus();
  }

  function finishMutationClose() {
    const trigger = mutationTarget?.kind === 'insert' ? mutationTarget.trigger : null;
    const restoreTable = mutationTarget?.kind === 'modify';
    mutationTarget = null;
    mutationError = '';
    mutationApplying = false;
    tick().then(() => { if (trigger) trigger.focus(); else if (restoreTable) tableScroll?.focus(); });
  }

  async function applyColumnQuery(query: string, closeDialog: boolean, change: VersionChange): Promise<boolean> {
    const current = result;
    const targetNodeId = queryMode === 'sql' ? activeSqlNodeId || selectedNodeId : selectedNodeId;
    if (!current || !targetNodeId || mutationApplying) return false;
    if (!query) { if (mutationTarget) mutationError = 'Could not build the column query.'; else columnMutationError = 'Could not build the column query.'; return false; }
    const id = ++requestId;
    mutationApplying = true;
    loadingData = true;
    mutationError = '';
    columnMutationError = '';
    error = '';
    try {
      const next = await api.querySql(targetNodeId, { sql: query, page: 1, page_size: pageSize, filters: [], sorts: [], dedupe_columns: [] });
      if (id !== requestId) return false;
      closeSql(false);
      clearAggregateDraft();
      filters = [];
      sorts = [];
      dedupeColumns = [];
      dedupeDraft = [];
      const preferredOrder = mutationColumnOrder(current, next, change);
      result = next;
      reconcileColumns(next, preferredOrder);
      queryMode = 'sql';
      sqlText = query;
      sqlBase = query;
      activeSql = next.sql;
      activeSqlNodeId = targetNodeId;
      selectedCell = null;
      editingCell = null;
      cellEditError = '';
      page = next.page;
      pageInput = String(next.page);
      stageChange(change, current.sql);
      if (closeDialog) mutationDialog?.close();
      return true;
    } catch (reason) {
      if (id === requestId) { if (mutationTarget) mutationError = message(reason); else columnMutationError = message(reason); }
      return false;
    } finally { if (id === requestId) loadingData = false; mutationApplying = false; }
  }

  async function applyMutation(name: string, expression: string) {
    const target = mutationTarget;
    const current = result;
    if (!target || !current) return;
    const columns = current.columns.map((column) => column.name);
    const query = target.kind === 'insert'
      ? buildMutationSql(current.sql, columns, target.insertIndex, expression, name)
      : buildColumnReplacementSql(current.sql, columns, target.column.name, expression, name);
    const change: VersionChange = target.kind === 'insert'
      ? { kind: 'add', summary: `Add ${name}`, details: { column: name, expression, after: target.left } }
      : { kind: 'modify', summary: `Modify ${target.column.name}`, details: { column: target.column.name, expression, output: name } };
    await applyColumnQuery(query, true, change);
  }

  async function modifyColumn(column: ColumnInfo) {
    if (!result || loadingData || mutationApplying) return;
    mutationTarget = { kind: 'modify', column };
    mutationError = '';
    await tick();
    mutationDialog?.showModal();
    mutationDialog?.querySelector<HTMLTextAreaElement>('textarea')?.focus();
  }

  async function duplicateColumn(column: ColumnInfo) {
    const current = result;
    if (!current || loadingData || mutationApplying) return;
    const columns = current.columns.map((item) => item.name);
    const index = columns.indexOf(column.name);
    const name = nextDuplicateColumnName(column.name, columns);
    await applyColumnQuery(
      buildMutationSql(current.sql, columns, index + 1, quoteIdentifier(column.name), name),
      false,
      { kind: 'duplicate', summary: `Duplicate ${column.name}`, details: { column: column.name, copy: name } }
    );
    await tick();
    tableScroll?.focus();
  }

  async function renameColumn(column: ColumnInfo) {
    if (!result || loadingData || mutationApplying) return;
    try {
      const requested = window.prompt('Rename column', column.name);
      if (requested === null) return;
      await renameColumnTo(column, requested);
    } finally {
      await tick();
      tableScroll?.focus();
    }
  }

  async function renameColumnTo(column: ColumnInfo, requested: string): Promise<boolean> {
    const current = result;
    const name = requested.trim();
    if (!current || !name) { columnMutationError = 'Column name cannot be blank.'; return false; }
    if (name === column.name) return true;
    const columns = current.columns.map((item) => item.name);
    if (columns.some((item) => item !== column.name && item.toLocaleLowerCase() === name.toLocaleLowerCase())) { columnMutationError = 'Column names must be unique.'; return false; }
    return applyColumnQuery(
      buildColumnReplacementSql(current.sql, columns, column.name, quoteIdentifier(column.name), name),
      false,
      { kind: 'rename', summary: `Rename ${column.name} to ${name}`, details: { from: column.name, to: name } }
    );
  }

  function startColumnRename(column: ColumnInfo) {
    if (loadingData || mutationApplying) return;
    renamingColumn = { original: column.name, value: column.name };
    columnMutationError = '';
  }
  function setColumnRenameValue(value: string) { if (renamingColumn) renamingColumn = { ...renamingColumn, value }; }
  function cancelColumnRename() { renamingColumn = null; columnMutationError = ''; void tick().then(() => tableScroll?.focus()); }
  async function commitColumnRename() {
    const edit = renamingColumn;
    const column = result?.columns.find((item) => item.name === edit?.original);
    if (!edit || !column || loadingData || mutationApplying) return;
    if (await renameColumnTo(column, edit.value)) {
      renamingColumn = null;
      await tick();
      tableScroll?.focus();
    } else {
      await tick();
      tableScroll?.querySelector<HTMLInputElement>('th input[aria-label^="Rename column"]')?.focus();
    }
  }

  async function openExport(trigger: HTMLButtonElement) {
    if (!currentExportOption) return;
    if (exportOpen) { closeExport(); return; }
    exportTrigger = trigger;
    exportOpen = true;
    exportFormat = 'csv';
    exportSelectedKeys = ['current'];
    exportOptions = [];
    exportError = '';
    exportLoading = false;
    exportRequestId++;
    await tick();
    exportOptions = projectViews.filter((view) => view.id !== currentHistory?.id).reduce<ExportOption[]>((options, view) => {
      const version = view.versions.find((item) => item.id === view.activeVersionId);
      if (!version) return options;
      options.push({
        key: view.id,
        node_id: activeProject?.node_id ?? view.nodeId,
        name: view.name,
        source: view.kind === 'derived' ? 'Derived Views' : nodes.find((node) => node.id === view.sourceId)?.name ?? 'Source Views',
        sql: version.sql
      });
      return options;
    }, []);
  }

  function closeExport() {
    if (exporting) return;
    const trigger = exportTrigger;
    exportRequestId++;
    exportOpen = false;
    exportOptions = [];
    exportSelectedKeys = ['current'];
    exportError = '';
    exportLoading = false;
    exportTrigger = null;
    tick().then(() => trigger?.focus());
  }

  function setExportFormat(format: ExportFormat) { exportFormat = format; if (format === 'csv') exportSelectedKeys = ['current']; }
  function toggleExportOption(key: string, checked: boolean) { exportSelectedKeys = checked ? [...new Set([...exportSelectedKeys, key])] : exportSelectedKeys.filter((item) => item !== key); }

  async function runExport() {
    const current = currentExportOption;
    if (!current || exporting || exportLoading) return;
    const choices = [current, ...exportOptions];
    const selected = exportFormat === 'csv' ? [current] : choices.filter((option) => exportSelectedKeys.includes(option.key));
    if (!selected.length) return;
    exporting = true;
    exportError = '';
    try {
      const download = await api.exportData({
        format: exportFormat,
        filename: exportFormat === 'csv' ? current.name : 'quark-export',
        sheets: selected.map(({ node_id, name, sql }) => ({ node_id, name, sql }))
      });
      const url = URL.createObjectURL(download.blob);
      const anchor = document.createElement('a');
      anchor.href = url;
      anchor.download = download.filename;
      document.body.append(anchor);
      anchor.click();
      anchor.remove();
      setTimeout(() => URL.revokeObjectURL(url), 0);
      exporting = false;
      closeExport();
    } catch (reason) { exportError = message(reason); }
    finally { exporting = false; }
  }

  function isRecord(value: unknown): value is Record<string, unknown> {
    return typeof value === 'object' && value !== null && !Array.isArray(value);
  }

  function stringArray(value: unknown): value is string[] {
    return Array.isArray(value) && value.every((item) => typeof item === 'string');
  }

  function cleanJoin(value: unknown): JoinWorkspaceRequest | undefined {
    if (!isRecord(value) || !isRecord(value.left) || !isRecord(value.right)
      || typeof value.left.node_id !== 'string' || typeof value.right.node_id !== 'string'
      || !stringArray(value.left_keys) || !stringArray(value.right_keys)) return undefined;
    const side = (item: Record<string, unknown>): JoinWorkspaceRequest['left'] | null => typeof item.sql === 'string'
      ? { node_id: item.node_id as string, sql: item.sql, ...(typeof item.name === 'string' ? { name: item.name } : {}) }
      : typeof item.dataset === 'string' ? { node_id: item.node_id as string, dataset: item.dataset } : null;
    const left = side(value.left);
    const right = side(value.right);
    if (!left || !right) return undefined;
    return { left, right, left_keys: [...value.left_keys], right_keys: [...value.right_keys] };
  }

  function cleanChange(value: unknown): VersionChange | null {
    if (!isRecord(value) || typeof value.kind !== 'string' || typeof value.summary !== 'string') return null;
    if (value.details !== undefined && !isRecord(value.details)) return null;
    return {
      kind: value.kind,
      summary: value.summary,
      ...(value.details ? { details: structuredClone(value.details) as Record<string, SerializableValue> } : {})
    };
  }

  function cleanVersion(value: unknown): Version | null {
    if (!isRecord(value) || typeof value.id !== 'string' || !Number.isInteger(value.number) || Number(value.number) < 1
      || value.fork !== undefined && (!Number.isInteger(value.fork) || Number(value.fork) < 2)
      || typeof value.nodeId !== 'string' || typeof value.dataset !== 'string' || typeof value.sql !== 'string'
      || typeof value.timestamp !== 'string' || !stringArray(value.columns) || !stringArray(value.hiddenColumns)
      || !Array.isArray(value.changes) || value.parentId !== undefined && typeof value.parentId !== 'string') return null;
    const changes = value.changes.map(cleanChange).filter((change): change is VersionChange => change !== null);
    const join = value.join === undefined ? undefined : cleanJoin(value.join);
    if (changes.length !== value.changes.length || value.join !== undefined && !join) return null;
    return {
      id: value.id,
      ...(value.parentId ? { parentId: value.parentId } : {}),
      number: Number(value.number),
      ...(value.fork !== undefined ? { fork: Number(value.fork) } : {}),
      nodeId: value.nodeId,
      dataset: value.dataset,
      sql: value.sql,
      columns: [...value.columns],
      hiddenColumns: [...value.hiddenColumns],
      timestamp: value.timestamp,
      changes,
      ...(join ? { join } : {})
    };
  }

  function cleanHistory(value: unknown): { history: ViewHistory; pendingCount: number } | null {
    if (!isRecord(value) || typeof value.id !== 'string' || typeof value.projectId !== 'string'
      || typeof value.name !== 'string' || value.kind !== 'source' && value.kind !== 'derived'
      || value.sourceId !== undefined && typeof value.sourceId !== 'string'
      || typeof value.nodeId !== 'string' || typeof value.dataset !== 'string'
      || typeof value.activeVersionId !== 'string' || !Array.isArray(value.versions) || !value.versions.length
      || !Array.isArray(value.pendingChanges) || value.pendingParentId !== null && typeof value.pendingParentId !== 'string') return null;
    const versions = value.versions.map(cleanVersion).filter((version): version is Version => version !== null);
    const pending = value.pendingChanges.map(cleanChange).filter((change): change is VersionChange => change !== null);
    const versionIds = new Set(versions.map((version) => version.id));
    if (versions.length !== value.versions.length || pending.length !== value.pendingChanges.length || versionIds.size !== versions.length
      || versions.some((version) => version.nodeId !== value.nodeId || version.dataset !== value.dataset)
      || versions.some((version, index) => {
        const parent = version.parentId ? versions.slice(0, index).find((item) => item.id === version.parentId) : undefined;
        return new Set(version.columns).size !== version.columns.length
        || version.hiddenColumns.some((column) => !version.columns.includes(column))
        || (index === 0 ? version.parentId !== undefined || version.number !== 1 || version.fork !== undefined : !parent || version.number !== parent.number + 1);
      })) return null;
    const activeVersionId = versions.some((version) => version.id === value.activeVersionId) ? value.activeVersionId : versions[versions.length - 1].id;
    return {
      pendingCount: pending.length,
      history: {
        id: value.id,
        projectId: value.projectId,
        name: value.name,
        kind: value.kind,
        ...(typeof value.sourceId === 'string' ? { sourceId: value.sourceId } : {}),
        nodeId: value.nodeId,
        dataset: value.dataset,
        versions,
        activeVersionId,
        pendingParentId: null,
        pendingChanges: []
      }
    };
  }

  function persistHistories(next: ViewHistory[]): boolean {
    try {
      localStorage.setItem(VERSIONING_STORAGE_KEY, JSON.stringify(next));
      versionHistories = next;
      storageError = '';
      return true;
    } catch {
      storageError = 'Versions and Views could not be stored in this browser.';
      return false;
    }
  }

  function loadVersioning() {
    let values: unknown[] = [];
    let pendingCount = 0;
    let readError = '';
    const flat = localStorage.getItem(VERSIONING_STORAGE_KEY);
    try {
      if (flat !== null) {
        const parsed: unknown = JSON.parse(flat);
        if (!Array.isArray(parsed)) throw new Error('Invalid version history');
        values = parsed;
      } else {
        const parsed: unknown = JSON.parse(localStorage.getItem(LEGACY_VERSIONING_STORAGE_KEY) ?? '[]');
        if (!Array.isArray(parsed)) throw new Error('Invalid legacy version history');
        for (const value of parsed) {
          try { values.push(...migrateDatasetHistories([value as DatasetVersionHistory])); }
          catch { readError = 'Some browser version history was invalid and was ignored.'; }
        }
      }
      const legacy: unknown = JSON.parse(localStorage.getItem(LEGACY_STORAGE_KEY) ?? '[]');
      if (!Array.isArray(legacy)) throw new Error('Invalid legacy Views');
      values.push(...migrateSavedQueries(legacy, new Date().toISOString()));
    } catch {
      readError = 'Version history could not be read from this browser.';
    }

    const histories: ViewHistory[] = [];
    const seen = new Set<string>();
    for (const value of values) {
      const cleaned = cleanHistory(value);
      if (!cleaned || seen.has(cleaned.history.id)) { readError = 'Some browser version history was invalid and was ignored.'; continue; }
      seen.add(cleaned.history.id);
      histories.push(cleaned.history);
      pendingCount += cleaned.pendingCount;
    }
    if (persistHistories(histories)) {
      try {
        localStorage.removeItem(LEGACY_STORAGE_KEY);
        localStorage.removeItem(LEGACY_VERSIONING_STORAGE_KEY);
      } catch { storageError = 'Migrated legacy history could not be removed from this browser.'; }
    }
    if (readError && !storageError) storageError = readError;
    if (pendingCount) recordingNotice = `Cleared ${pendingCount} pending change${pendingCount === 1 ? '' : 's'} because working SQL is not stored.`;
  }

  function replaceHistory(nextHistory: ViewHistory, retainOnFailure = false): boolean {
    const index = versionHistories.findIndex((history) => history.id === nextHistory.id);
    const next = index < 0 ? [...versionHistories, nextHistory] : versionHistories.map((history, itemIndex) => itemIndex === index ? nextHistory : history);
    const saved = persistHistories(next);
    if (!saved && retainOnFailure) versionHistories = next;
    return saved;
  }

  function stageChange(change: VersionChange, _viewSql = result?.sql ?? activeSql) {
    if (!currentHistory) return;
    recordingNotice = '';
    replaceHistory(stageVersionChange(currentHistory, change), true);
  }

  function blockViewExecutionWhileRecording(): boolean {
    if (!currentHistory?.pendingChanges.length) return false;
    recordingNotice = 'Stop recording before creating a View.';
    return true;
  }

  function addView(value: string, join = activeJoin, displayName = '') {
    const query = value.trim();
    if (!activeProject || !query) return;
    const firstLine = query.split(/\r?\n/, 1)[0].replace(/\s+/g, ' ').trim();
    const name = displayName.trim().slice(0, 64) || firstLine.slice(0, 64) || `View ${projectViews.length + 1}`;
    const history = createView({
      id: crypto.randomUUID(),
      projectId: activeProject.id,
      name,
      nodeId: activeProject.node_id,
      dataset: name,
      sql: query,
      columns: result?.columns.map((column) => column.name) ?? [...columnOrder],
      hiddenColumns: [...hiddenColumns],
      timestamp: new Date().toISOString(),
      ...(join ? { join } : {})
    });
    if (replaceHistory(history)) {
      selectedDataset = history.id;
      joinLeftViewId = history.id;
      joinRightViewId = '';
      joinLeftKeys = [];
      joinRightKeys = [];
      joinLeftColumns = [...history.versions[0].columns];
      joinRightColumns = [];
      clearJoinPreview();
      queryMenuOpen = null;
    }
  }

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
            void runSqlAndAddView().then(() => editorView?.focus());
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

  function closeSql(_restoreFocus = true) { editorView?.destroy(); editorView = null; sqlOpen = false; }
  function toggleTableExpanded() { if (!tableExpanded) { if (sqlOpen) closeSql(false); queryMenuOpen = null; railOpen = false; } tableExpanded = !tableExpanded; }
  function resetSql(dataset: BaseViewInfo | undefined) { closeSql(); queryMode = 'builder'; sqlText = dataset?.sql ?? ''; sqlBase = ''; activeSql = ''; activeSqlNodeId = ''; sqlError = ''; }

  function discardPending(): boolean {
    const history = currentHistory;
    if (!history?.pendingChanges.length) return true;
    if (!window.confirm(`Discard ${history.pendingChanges.length} pending change${history.pendingChanges.length === 1 ? '' : 's'}?`)) return false;
    replaceHistory({ ...history, pendingParentId: null, pendingChanges: [] });
    recordingNotice = 'Pending changes discarded.';
    return true;
  }

  async function replayStored(sql: string, nodeId: string, join?: JoinWorkspaceRequest, columns?: string[], hidden: string[] = []): Promise<boolean> {
    const replayId = ++replayRequestId;
    page = 1;
    pageInput = '1';
    let targetNodeId = nodeId;
    try {
      if (join) targetNodeId = (await api.previewJoinWorkspace(join)).node_id;
    } catch (reason) {
      sqlError = message(reason);
      return false;
    }
    if (replayId !== replayRequestId) return false;
    if (!await runSql(sql, false, false, targetNodeId)) return false;
    activeJoin = join;
    if (result) reconcileColumns(result, columns);
    hiddenColumns = hidden.filter((column) => columnOrder.includes(column));
    shownColumnTypes = [];
    return true;
  }

  async function replayVersionSnapshot(version: Version): Promise<boolean> {
    workspaceTab = 'data';
    filters = [];
    sorts = [];
    dedupeColumns = [];
    dedupeDraft = [];
    page = 1;
    pageInput = '1';
    return replayStored(version.sql, version.nodeId, version.join, version.columns, version.hiddenColumns);
  }

  async function restoreVersion(version: Version) {
    if (!discardPending()) return;
    if (!await replayVersionSnapshot(version)) return;
    if (currentHistory) replaceHistory(activateVersion(currentHistory, version.id));
  }

  function previousVersion() { const version = currentHistory?.versions.find((item) => item.id === activeVersion?.parentId); if (canPreviousVersion && version) void restoreVersion(version); }
  function nextVersion() { const version = activeVersionChildren[0]; if (canNextVersion && version) void restoreVersion(version); }

  async function showDiff(history: ViewHistory, version: Version, returnFocus: HTMLElement | null = null) {
    const diff = versionDiff(history, version.id);
    if (!diff) return;
    diffReturnFocus = returnFocus;
    openDiff = diff;
    await tick();
    diffDialog?.showModal();
  }

  function closeDiff() {
    const target = diffReturnFocus;
    diffReturnFocus = null;
    openDiff = null;
    void tick().then(() => target?.focus());
  }

  async function stopRecording() {
    const history = currentHistory;
    if (!history || !result || !history.pendingChanges.length) return;
    const next = finalizeVersion(history, {
      sql: result.sql,
      columns: [...columnOrder],
      hiddenColumns: [...hiddenColumns],
      timestamp: new Date().toISOString(),
      ...(activeJoin ? { join: activeJoin } : {})
    });
    if (!replaceHistory(next)) return;
    const version = next.versions.find((item) => item.id === next.activeVersionId);
    if (version) await showDiff(next, version, tableScroll);
  }


  async function discardWorkbook() {
    const preview = workbookPreview;
    workbookPreview = null;
    workbookSheets = [];
    if (!preview || !activeProject) return;
    try { await api.discardWorkbook(activeProject.id, preview.id); }
    catch (reason) { error = message(reason); }
  }

  async function confirmWorkbook() {
    const preview = workbookPreview;
    const project = activeProject;
    if (!preview || !project || !workbookSheets.length || confirmingWorkbook) return;
    confirmingWorkbook = true;
    error = '';
    try {
      await api.confirmWorkbook(project.id, preview.id, workbookSheets);
      workbookPreview = null;
      workbookSheets = [];
      workbookDialog?.close();
      await refreshAfterSourceMutation(project);
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
    if (!activeProject) return;
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

  function clearWorkspaceState() {
    replayRequestId++;
    requestId++;
    sourceRequestId++;
    closeInspector();
    resetSql(undefined);
    workspaceTab = 'data';
    selectedNodeId = '';
    selectedDataset = '';
    datasets = [];
    nodes = [];
    loadedSourceIds = [];
    loadingSourceId = '';
    result = null;
    filters = [];
    sorts = [];
    dedupeColumns = [];
    dedupeDraft = [];
    clearAggregateDraft();
    clearJoinPreview();
    joinLeftViewId = '';
    joinRightViewId = '';
    joinLeftKeys = [];
    joinRightKeys = [];
    joinLeftColumns = [];
    joinRightColumns = [];
    hiddenColumns = [];
    columnOrder = [];
    activeJoin = undefined;
    lastHiddenColumn = null;
    shownColumnTypes = [];
    error = '';
    recordingNotice = '';
    queryMenuOpen = null;
    tableExpanded = false;
    selectedCell = null;
    editingCell = null;
    railOpen = false;
    sourceOpen = false;
  }

  async function loadProjects() {
    loadingProjects = true;
    projectError = '';
    try { projects = await api.listProjects(); }
    catch (reason) { projectError = message(reason); }
    finally { loadingProjects = false; }
  }

  async function createProject(event: SubmitEvent) {
    event.preventDefault();
    const name = projectName.trim();
    if (!name || creatingProject) return;
    creatingProject = true;
    projectError = '';
    try {
      const project = await api.createProject(name);
      projects = [...projects.filter((item) => item.id !== project.id), project];
      projectName = '';
      await openProject(project.id);
    } catch (reason) { projectError = message(reason); }
    finally { creatingProject = false; }
  }

  function mergeBaseViews(project: ProjectInfo, baseViews: BaseViewInfo[]): ViewHistory[] {
    const sourceIds = new Set(baseViews.map((view) => view.source_id));
    datasets = [...datasets.filter((view) => !sourceIds.has(view.source_id)), ...baseViews];
    const rebound = rebindLegacyHistories(versionHistories, datasets, project);
    const existing = new Set(rebound.map((history) => history.id));
    const created = baseViews.filter((view) => !existing.has(view.id)).map((view) => createSourceHistory({
      id: view.id,
      projectId: project.id,
      sourceId: view.source_id,
      name: view.name,
      nodeId: view.node_id,
      dataset: view.name,
      sql: view.sql,
      columns: [...view.columns],
      hiddenColumns: [],
      timestamp: new Date().toISOString()
    }));
    const next = created.length ? [...rebound, ...created] : rebound;
    if (next !== versionHistories) persistHistories(next);
    return next;
  }

  async function loadProjectContents(project: ProjectInfo) {
    loadingNodes = true;
    error = '';
    try {
      const sources = await api.listProjectSources(project.id);
      if (activeProject?.id !== project.id) return;
      const available = new Set(sources.map((source) => source.id));
      nodes = sources;
      loadedSourceIds = loadedSourceIds.filter((id) => available.has(id));
      datasets = datasets.filter((view) => available.has(view.source_id));
    } catch (reason) { if (activeProject?.id === project.id) error = message(reason); }
    finally { if (activeProject?.id === project.id) loadingNodes = false; }
  }

  async function loadProjectSource(sourceId: string, preferredViewId = '', selectAfterLoad = true): Promise<boolean> {
    const project = activeProject;
    if (!project) return false;
    if (loadedSourceIds.includes(sourceId)) {
      sourceRequestId++;
      loadingSourceId = '';
      if (!selectAfterLoad) return true;
      const available = versionHistories.filter((history) => history.projectId === project.id && history.kind === 'source' && history.sourceId === sourceId);
      const next = available.find((history) => history.id === preferredViewId) ?? available[0];
      if (next) await selectView(next.id, true);
      return true;
    }
    const id = ++sourceRequestId;
    loadingSourceId = sourceId;
    error = '';
    try {
      const source = await api.getProjectSource(project.id, sourceId);
      if (id !== sourceRequestId || activeProject?.id !== project.id) return false;
      const histories = mergeBaseViews(project, source.views);
      loadedSourceIds = [...loadedSourceIds, sourceId];
      if (!selectAfterLoad) return true;
      const available = histories.filter((history) => history.projectId === project.id && history.kind === 'source' && history.sourceId === sourceId);
      const next = available.find((history) => history.id === preferredViewId) ?? available[0];
      if (next) await selectView(next.id, true);
      return true;
    } catch (reason) {
      if (id === sourceRequestId && activeProject?.id === project.id) error = message(reason);
      return false;
    } finally {
      if (id === sourceRequestId) loadingSourceId = '';
    }
  }

  async function loadAllProjectSources(project: ProjectInfo): Promise<boolean> {
    const id = ++sourceRequestId;
    loadingSourceId = '*';
    error = '';
    try {
      const baseViews = await api.listProjectViews(project.id);
      if (id !== sourceRequestId || activeProject?.id !== project.id) return false;
      mergeBaseViews(project, baseViews);
      loadedSourceIds = nodes.map((source) => source.id);
      return true;
    } catch (reason) {
      if (id === sourceRequestId && activeProject?.id === project.id) error = message(reason);
      return false;
    } finally {
      if (id === sourceRequestId) loadingSourceId = '';
    }
  }

  async function refreshAfterSourceMutation(project: ProjectInfo) {
    clearWorkspaceState();
    selectedNodeId = project.node_id;
    await loadProjectContents(project);
  }

  async function openProject(id: string) {
    const project = projects.find((item) => item.id === id);
    if (!project || !discardPending()) return;
    clearWorkspaceState();
    activeProject = project;
    selectedNodeId = project.node_id;
    await loadProjectContents(project);
  }

  function exitProject() {
    if (!discardPending()) return;
    clearWorkspaceState();
    activeProject = null;
    void loadProjects();
  }

  async function selectView(id: string, sourceReady = false) {
    let history = versionHistories.find((item) => item.id === id && item.projectId === activeProject?.id);
    if (!history) return;
    if (!sourceReady) {
      sourceRequestId++;
      loadingSourceId = '';
    }
    if (!sourceReady && history.kind === 'source' && history.sourceId && !loadedSourceIds.includes(history.sourceId)) {
      await loadProjectSource(history.sourceId, id);
      return;
    }
    if (history.kind === 'derived' && activeProject && loadedSourceIds.length < nodes.length) {
      // ponytail: historic arbitrary SQL has no dependency manifest, so explicit derived-View replay loads all source metadata.
      if (!await loadAllProjectSources(activeProject)) return;
      history = versionHistories.find((item) => item.id === id && item.projectId === activeProject?.id);
      if (!history) return;
    }
    if (id === selectedDataset && result) { workspaceTab = 'data'; railOpen = false; return; }
    if (!discardPending()) return;
    replayRequestId++;
    closeInspector();
    resetSql(datasets.find((view) => view.id === id));
    selectedNodeId = activeProject?.node_id ?? history.nodeId;
    selectedDataset = id;
    workspaceTab = 'data';
    filters = [];
    sorts = [];
    dedupeColumns = [];
    dedupeDraft = [];
    clearAggregateDraft();
    clearJoinPreview();
    joinLeftViewId = id;
    joinRightViewId = '';
    joinLeftKeys = [];
    joinRightKeys = [];
    joinLeftColumns = [];
    joinRightColumns = [];
    hiddenColumns = [];
    columnOrder = [];
    activeJoin = undefined;
    lastHiddenColumn = null;
    shownColumnTypes = [];
    page = 1;
    pageInput = '1';
    railOpen = false;
    const version = history.versions.find((item) => item.id === history.activeVersionId) ?? history.versions[history.versions.length - 1];
    if (version) await replayVersionSnapshot(version);
  }

  async function loadData(): Promise<boolean> {
    const sql = sqlBase || activeSql || activeVersion?.sql;
    return sql ? runSql(sql, true, true, activeProject?.node_id ?? currentHistory?.nodeId) : false;
  }

  async function createAggregateView() {
    if (blockViewExecutionWhileRecording()) return;
    const source = queryMode === 'builder' ? result?.sql : aggregateSourceSql || sqlBase || activeSql;
    const columns = queryMode === 'builder' ? result?.columns ?? [] : aggregateSourceColumns.length ? aggregateSourceColumns : result?.columns ?? [];
    const aggregates = aggregateFields.map((column) => ({ column, metrics: aggregateFieldMetrics[column] }));
    if (!source || !aggregates.some(({ metrics }) => metrics.length)) return;
    const query = buildAggregateSql(source, aggregateIndexes, aggregates);
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
    if (await runSql(query, false, true)) addView(result?.sql ?? query, undefined, `Aggregate of ${currentHistory?.name ?? 'View'}`);
  }

  function reconcileColumns(next: QueryResponse, preferred = columnOrder.length ? columnOrder : next.columns.map((column) => column.name)) {
    const names = next.columns.map((column) => column.name);
    columnOrder = [...preferred.filter((column, index) => names.includes(column) && preferred.indexOf(column) === index), ...names.filter((column) => !preferred.includes(column))];
    hiddenColumns = hiddenColumns.filter((column) => names.includes(column));
  }

  function mutationColumnOrder(current: QueryResponse, next: QueryResponse, change: VersionChange): string[] {
    const before = current.columns.map((column) => column.name);
    const after = next.columns.map((column) => column.name);
    const removed = before.filter((name) => !after.includes(name));
    const added = after.filter((name) => !before.includes(name));
    let preferred = columnOrder.length ? [...columnOrder] : [...before];
    if (removed.length === 1 && added.length === 1) {
      preferred = preferred.map((name) => name === removed[0] ? added[0] : name);
      hiddenColumns = hiddenColumns.map((name) => name === removed[0] ? added[0] : name);
      if (lastHiddenColumn === removed[0]) lastHiddenColumn = added[0];
    } else if (added.length) {
      const detail = change.details?.after ?? change.details?.column;
      const anchor = typeof detail === 'string' ? preferred.indexOf(detail) : -1;
      preferred.splice(anchor < 0 ? preferred.length : anchor + 1, 0, ...added);
    }
    return preferred;
  }

  function beginColumnReorder() {
    if (reorderOrigin) return;
    const names = orderedColumns.map((column) => column.name);
    reorderOrigin = names;
    columnOrder = names;
  }

  function previewColumnReorder(dragged: string, target: string, placement: 'before' | 'after') {
    if (!reorderOrigin || dragged === target) return;
    const next = columnOrder.filter((name) => name !== dragged);
    const targetIndex = next.indexOf(target);
    if (targetIndex < 0) return;
    next.splice(targetIndex + (placement === 'after' ? 1 : 0), 0, dragged);
    if (!next.every((name, index) => name === columnOrder[index])) columnOrder = next;
  }

  function commitColumnReorder() {
    const before = reorderOrigin ? [...reorderOrigin] : null;
    if (!before) return;
    const after = [...columnOrder];
    reorderOrigin = null;
    if (before.length === after.length && before.every((name, index) => name === after[index])) return;
    stageChange({ kind: 'reorder', summary: 'Reorder columns', details: { before, after } });
  }

  function cancelColumnReorder() {
    if (!reorderOrigin) return;
    columnOrder = [...reorderOrigin];
    reorderOrigin = null;
  }

  function moveColumnOneStep(name: string, direction: -1 | 1) {
    const names = orderedColumns.map((column) => column.name);
    const index = names.indexOf(name);
    const target = names[index + direction];
    if (index < 0 || !target) return;
    beginColumnReorder();
    previewColumnReorder(name, target, direction < 0 ? 'before' : 'after');
    commitColumnReorder();
  }

  async function runSql(value = sqlText, keepSqlBase = false, keepAggregateBuilder = false, nodeId?: string): Promise<boolean> {
    const query = value.trim();
    const targetNodeId = nodeId || activeSqlNodeId || selectedNodeId;
    if (!targetNodeId || !query) { sqlError = 'Enter SQL to run.'; return false; }
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
      const next = await api.querySql(targetNodeId, { sql: source, page, page_size: pageSize, filters, sorts, dedupe_columns: dedupeColumns });
      if (id !== requestId) return false;
      result = next;
      reconcileColumns(next, keepSqlBase && columnOrder.length ? columnOrder : next.columns.map((column) => column.name));
      sqlBase = source;
      activeSql = next.sql;
      activeSqlNodeId = targetNodeId;
      queryMode = 'sql';
      selectedCell = null;
      editingCell = null;
      cellEditSaving = false;
      cellEditError = '';
      if (!keepSqlBase) {
        hiddenColumns = [];
        shownColumnTypes = [];
      }
      page = next.page;
      pageInput = String(next.page);
      if (sqlOpen) { await tick(); createSqlEditor(); }
      return true;
    } catch (reason) { if (id === requestId) sqlError = message(reason); return false; }
    finally { if (id === requestId) loadingData = false; }
  }

  async function runSqlAndAddView(): Promise<boolean> {
    if (blockViewExecutionWhileRecording()) return false;
    page = 1;
    pageInput = '1';
    const query = sqlText;
    const ran = await runSql(query);
    if (ran) addView(result?.sql ?? query);
    return ran;
  }

  async function loadActiveData() { if (queryMode === 'sql') await runSql(sqlBase || activeSql, true); else await loadData(); }

  async function upload(event: Event) {
    const input = event.currentTarget as HTMLInputElement;
    const file = input.files?.[0];
    const project = activeProject;
    if (!file || !project) return;
    mutating = true;
    error = '';
    try {
      const node = await api.uploadNode(project.id, file);
      if (isWorkbookPreview(node)) {
        workbookPreview = node;
        workbookSheets = [...node.sheets];
        await tick();
        workbookDialog?.showModal();
      } else {
        await refreshAfterSourceMutation(project);
      }
    } catch (reason) { error = message(reason); }
    finally { mutating = false; input.value = ''; }
  }

  async function attach() {
    const path = attachPath.trim();
    const project = activeProject;
    if (!path || !project) return;
    mutating = true;
    error = '';
    try {
      await api.attachNode(project.id, path);
      attachPath = '';
      await refreshAfterSourceMutation(project);
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
        ? await api.getSqlCategoryValues(activeSqlNodeId || selectedNodeId, filterColumn.name, { sql: sqlBase || activeSql }, { search: categorySearch.trim(), offset })
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

  async function applyFilterChange(next: FilterCondition[], change: VersionChange) {
    filters = next;
    page = 1;
    pageInput = '1';
    if (await loadData()) stageChange(change);
  }

  async function addCategoryFilter() {
    if (!filterColumn || selectedCategories.length === 0) return;
    const values = [...selectedCategories];
    const filter: FilterCondition = { column: filterColumn.name, operator: 'in', value: values, ...(filters.length ? { connector: 'and' as const } : {}) };
    closeInspector();
    await applyFilterChange([...filters, filter], { kind: 'filter', summary: `Filter ${filterSummary(filter)}`, details: { column: filter.column, operator: filter.operator, value: values } });
  }

  async function addFilter() {
    if (!filterColumn) return;
    const noValue = filterOperator === 'is_null' || filterOperator === 'not_null';
    if (!noValue && filterValue === '') return;
    const numericValue = filterColumn.numeric ? normalizedNumber(filterValue) : filterValue;
    if (!noValue && numericValue === null) return;
    if (filterColumn.numeric && numericValue !== null) filterValue = formattedNumber(numericValue);
    const value = noValue ? undefined : filterColumn.numeric ? numericValue! : isBooleanType(filterColumn.type) ? filterValue === 'true' : filterValue;
    const filter: FilterCondition = { column: filterColumn.name, operator: filterOperator, ...(value === undefined ? {} : { value }), ...(filters.length ? { connector: 'and' as const } : {}) };
    closeInspector();
    await applyFilterChange([...filters, filter], { kind: 'filter', summary: `Filter ${filterSummary(filter)}`, details: { column: filter.column, operator: filter.operator, ...(value === undefined ? {} : { value }) } });
  }

  function addNullFilter(operator: 'is_null' | 'not_null') { filterOperator = operator; void addFilter(); }

  async function toggleFilterConnector(index: number) {
    const filter = filters[index];
    if (!filter || index === 0) return;
    const connector = filter.connector === 'or' ? 'and' : 'or';
    await applyFilterChange(filters.map((item, itemIndex) => itemIndex === index ? { ...item, connector } : item), { kind: 'filter-connector', summary: `Use ${connector.toUpperCase()} before ${filter.column}`, details: { index, column: filter.column, connector } });
  }

  async function removeFilter(index: number) {
    const filter = filters[index];
    if (!filter) return;
    const value = Array.isArray(filter.value) ? [...filter.value] : filter.value;
    await applyFilterChange(filters.filter((_, itemIndex) => itemIndex !== index), { kind: 'filter-remove', summary: `Remove filter ${filterSummary(filter)}`, details: { column: filter.column, operator: filter.operator, ...(value === undefined ? {} : { value }) } });
  }
  async function cycleSort(column: ColumnInfo) {
    const existing = sorts.find((sort) => sort.column === column.name);
    sorts = existing?.direction === 'asc' ? sorts.map((sort) => sort.column === column.name ? { ...sort, direction: 'desc' } : sort) : existing ? sorts.filter((sort) => sort.column !== column.name) : [...sorts, { column: column.name, direction: 'asc' }];
    page = 1;
    await loadData();
  }
  async function removeSort(column: string) { sorts = sorts.filter((sort) => sort.column !== column); page = 1; await loadData(); }
  async function clearQuery() {
    const count = filters.length;
    sorts = [];
    dedupeColumns = [];
    dedupeDraft = [];
    await applyFilterChange([], { kind: 'filter-clear', summary: 'Clear conditions', details: { count } });
  }
  async function backToBuilder() {
    if (!discardPending()) return;
    const version = currentHistory?.versions.find((item) => item.id === currentHistory.activeVersionId);
    if (!version) return;
    clearAggregateDraft();
    clearJoinDraft();
    await replayVersionSnapshot(version);
  }
  function toggleDedupe(column: string, checked: boolean) { dedupeDraft = checked ? [...dedupeDraft, column] : dedupeDraft.filter((item) => item !== column); }
  async function applyDedupe() { dedupeColumns = [...dedupeDraft]; page = 1; await loadData(); }
  async function clearDedupe() { dedupeColumns = []; dedupeDraft = []; page = 1; await loadData(); }
  function isColumnProtected(column: string): boolean { return [...dedupeColumns, ...dedupeDraft].includes(column); }
  function setHidden(next: string[], change: VersionChange): boolean {
    const columns = new Set((result?.columns ?? []).map((column) => column.name));
    const normalized = [...new Set(next)].filter((column) => columns.has(column));
    if (normalized.length === hiddenColumns.length && normalized.every((column, index) => column === hiddenColumns[index])) return false;
    hiddenColumns = normalized;
    stageChange(change);
    return true;
  }
  function hideColumn(column: string) {
    if (!isColumnProtected(column) && visibleColumns.length > 1 && setHidden([...hiddenColumns, column], { kind: 'hide', summary: `Hide ${column}`, details: { columns: [column] } })) lastHiddenColumn = column;
  }
  function hideColumnsAtNullFraction(fraction: number) {
    const columns = visibleColumns.filter((column) => !isColumnProtected(column.name) && column.null_fraction >= fraction).slice(0, visibleColumns.length - 1);
    if (columns.length && setHidden([...hiddenColumns, ...columns.map((column) => column.name)], { kind: 'hide', summary: `Hide ${columns.length} column${columns.length === 1 ? '' : 's'}`, details: { columns: columns.map((column) => column.name) } })) lastHiddenColumn = columns[columns.length - 1].name;
  }
  function restoreColumn(column: string) {
    if (setHidden(hiddenColumns.filter((item) => item !== column), { kind: 'show', summary: `Show ${column}`, details: { columns: [column] } }) && lastHiddenColumn === column) lastHiddenColumn = null;
  }
  function showAllColumns() {
    if (setHidden([], { kind: 'show', summary: 'Show all columns', details: { columns: [...hiddenColumns] } })) lastHiddenColumn = null;
    shownColumnTypes = [];
  }
  function isTypeShown(type: string): boolean { return shownColumnTypes.length === 0 || shownColumnTypes.includes(type); }
  function showColumnsOfTypes(types: string[]) {
    const columns = result?.columns ?? [];
    if (!columns.some((column) => types.includes(column.type) || isColumnProtected(column.name))) return;
    const next = columns.filter((column) => !types.includes(column.type) && !isColumnProtected(column.name)).map((column) => column.name);
    if (setHidden(next, { kind: 'visibility', summary: `Show ${types.join(', ')} columns`, details: { types: [...types] } })) lastHiddenColumn = null;
  }
  function toggleShownType(type: string, checked: boolean) {
    const selected = shownColumnTypes.length ? shownColumnTypes : [...columnTypes];
    const next = checked ? [...new Set([...selected, type])] : selected.filter((item) => item !== type);
    if (!next.length) return;
    shownColumnTypes = next.length === columnTypes.length ? [] : next;
    showColumnsOfTypes(next);
  }
  function toggleColumn(column: string, checked: boolean) { if (checked) restoreColumn(column); else hideColumn(column); }
  function applyRegexVisibility(pattern: string, invert: boolean, action: 'show' | 'hide'): string {
    const names = [...columnOrder];
    const result = matchColumnsByRegex(columnOrder, pattern);
    if (result.error) return result.error;
    const matches = new Set(result.matches);
    const selected = names.filter((name) => invert !== matches.has(name));
    const columns = action === 'show'
      ? selected.filter((name) => hiddenColumns.includes(name))
      : selected.filter((name) => !hiddenColumns.includes(name) && !isColumnProtected(name)).slice(0, Math.max(0, visibleColumns.length - 1));
    if (!columns.length) return '';
    const chosen = new Set(columns);
    const next = action === 'show' ? hiddenColumns.filter((name) => !chosen.has(name)) : [...hiddenColumns, ...columns];
    if (setHidden(next, {
      kind: 'visibility',
      summary: `${action === 'show' ? 'Show' : 'Hide'} ${columns.length} regex-selected column${columns.length === 1 ? '' : 's'}`,
      details: { pattern, invert, action, columns }
    })) lastHiddenColumn = action === 'hide' ? columns[columns.length - 1] : null;
    return '';
  }
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
        ? await api.getSqlColumnStats(activeSqlNodeId || selectedNodeId, column.name, { sql: sqlBase || activeSql, page, page_size: pageSize, filters, sorts, dedupe_columns: dedupeColumns })
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
  function selectCell(event: MouseEvent, row: number, column: string) {
    selectedCell = { row, column, expanded: false };
    (event.currentTarget as HTMLTableCellElement).focus();
  }
  function expandCell(event: MouseEvent, row: number, column: string) {
    const cell = event.currentTarget as HTMLTableCellElement;
    selectedCell = { row, column, expanded: cell.scrollWidth > cell.clientWidth };
  }
  async function filterCategoricalCell(column: ColumnInfo, value: unknown) {
    if (column.profile_kind !== 'categorical' || (typeof value !== 'string' && typeof value !== 'boolean') || filters.some((filter) => filter.column === column.name && filter.operator === '=' && filter.value === value)) return;
    const filter: FilterCondition = { column: column.name, operator: '=', value, ...(filters.length ? { connector: 'and' as const } : {}) };
    await applyFilterChange([...filters, filter], { kind: 'filter', summary: `Filter ${filterSummary(filter)}`, details: { column: filter.column, operator: filter.operator, value } });
  }
  function cellTitle(column: ColumnInfo, value: unknown): string { const text = display(value); return column.profile_kind === 'categorical' && (typeof value === 'string' || typeof value === 'boolean') ? `${text} — Double-click to filter by this value` : text; }
  function cellEditText(value: unknown): string { return value == null ? '' : display(value); }
  function startCellEdit(row: number, column: string, initial?: string) {
    if (!result || loadingData || cellEditSaving) return;
    const original = cellEditText(result.rows[row]?.[column]);
    selectedCell = { row, column, expanded: false };
    editingCell = { row, column, value: initial ?? original, original };
    cellEditError = '';
  }
  async function focusCell(row: number, column: string) {
    await tick();
    const cell = [...(tableScroll?.querySelectorAll<HTMLTableCellElement>('td[data-row][data-column]') ?? [])].find(
      (element) => Number(element.dataset.row) === row && element.dataset.column === column,
    );
    cell?.focus();
  }
  // One screen of rows, used by PageUp/PageDown. The grid is a single tab stop, so these
  // are the only way a keyboard user crosses a long page without holding an arrow key.
  function cellPageSize(): number {
    const rowHeight = tableScroll?.querySelector('tbody tr:not(.spacer)')?.getBoundingClientRect().height || 34;
    return Math.max(1, Math.floor((tableScroll?.clientHeight ?? rowHeight * 10) / rowHeight) - 1);
  }
  function moveCell(row: number, column: string, move: CellMove) {
    if (!result || !visibleColumns.length || !result.rows.length) return;
    const lastRow = result.rows.length - 1;
    const lastColumn = visibleColumns.length - 1;
    const columnIndex = Math.max(0, visibleColumns.findIndex((item) => item.name === column));
    const page = cellPageSize();
    const rowDelta = move === 'down' ? 1 : move === 'up' ? -1 : move === 'pageDown' ? page : move === 'pageUp' ? -page : 0;
    const nextRow = move === 'gridStart' ? 0 : move === 'gridEnd' ? lastRow : Math.min(lastRow, Math.max(0, row + rowDelta));
    const nextColumnIndex = move === 'rowStart' || move === 'gridStart'
      ? 0
      : move === 'rowEnd' || move === 'gridEnd'
        ? lastColumn
        : Math.min(lastColumn, Math.max(0, columnIndex + (move === 'right' ? 1 : move === 'left' ? -1 : 0)));
    const nextColumn = visibleColumns[nextColumnIndex].name;
    selectedCell = { row: nextRow, column: nextColumn, expanded: false };
    void focusCell(nextRow, nextColumn);
  }
  function handleCellKeydown(event: KeyboardEvent, row: number, column: string) {
    if (editingCell || loadingData || cellEditSaving) return;
    const moves: Record<string, CellMove> = { ArrowUp: 'up', ArrowDown: 'down', ArrowLeft: 'left', ArrowRight: 'right', PageUp: 'pageUp', PageDown: 'pageDown' };
    if (moves[event.key]) { event.preventDefault(); moveCell(row, column, moves[event.key]); return; }
    if (event.key === 'Home' || event.key === 'End') {
      event.preventDefault();
      const move: CellMove = event.ctrlKey || event.metaKey ? (event.key === 'Home' ? 'gridStart' : 'gridEnd') : (event.key === 'Home' ? 'rowStart' : 'rowEnd');
      moveCell(row, column, move);
      return;
    }
    if (event.key === 'Enter') { event.preventDefault(); startCellEdit(row, column); return; }
    if (event.key === 'Backspace' || event.key === 'Delete') { event.preventDefault(); startCellEdit(row, column, ''); return; }
    if (event.key.length === 1 && !event.altKey && !event.ctrlKey && !event.metaKey) { event.preventDefault(); startCellEdit(row, column, event.key); }
  }
  function setCellEditValue(value: string) { if (editingCell) editingCell = { ...editingCell, value }; }
  function cancelCellEdit() {
    const edit = editingCell;
    editingCell = null;
    cellEditError = '';
    if (edit) void focusCell(edit.row, edit.column);
  }
  async function commitCellEdit(move?: CellMove) {
    const edit = editingCell;
    const current = result;
    const targetNodeId = queryMode === 'sql' ? activeSqlNodeId || selectedNodeId : selectedNodeId;
    if (!edit || !current || !targetNodeId || cellEditSaving) return;
    if (edit.value === edit.original) {
      editingCell = null;
      cellEditError = '';
      if (move) moveCell(edit.row, edit.column, move);
      return;
    }
    if (hasVolatileRowOrder(current.sql)) {
      cellEditError = 'Cell editing is unavailable for queries with randomized row order.';
      return;
    }
    // ponytail: row-position edits stay View-local; switch to key-based patches when sources expose primary keys.
    const rowNumber = (current.page - 1) * current.page_size + edit.row + 1;
    const query = buildCellEditSql(current.sql, current.columns.map((column) => column.name), rowNumber, edit.column, edit.value);
    if (!query) { cellEditError = 'Could not build the cell edit query.'; return; }
    const id = ++requestId;
    cellEditSaving = true;
    cellEditError = '';
    try {
      const next = await api.querySql(targetNodeId, { sql: query, page: current.page, page_size: current.page_size, filters: [], sorts: [], dedupe_columns: [] });
      if (id !== requestId) return;
      closeSql(false);
      clearAggregateDraft();
      filters = [];
      sorts = [];
      dedupeColumns = [];
      dedupeDraft = [];
      result = next;
      reconcileColumns(next);
      queryMode = 'sql';
      sqlText = query;
      sqlBase = query;
      activeSql = next.sql;
      activeSqlNodeId = targetNodeId;
      editingCell = null;
      page = next.page;
      pageInput = String(next.page);
      stageChange({ kind: 'cell', summary: `Edit row ${rowNumber}, ${edit.column}`, details: { row: rowNumber, column: edit.column } }, current.sql);
      if (move) moveCell(edit.row, edit.column, move);
      else void focusCell(selectedCell?.row ?? edit.row, selectedCell?.column ?? edit.column);
    } catch (reason) {
      if (id === requestId) {
        cellEditError = message(reason);
        cellEditSaving = false;
        await tick();
        tableScroll?.querySelector<HTMLInputElement>('td.editing-cell input')?.focus();
      }
    } finally { cellEditSaving = false; }
  }
  function collapseCell(row: number, column: string) { if (selectedCell?.row === row && selectedCell.column === column) selectedCell = { row, column, expanded: false }; }
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

{#if !activeProject}
  <ProjectsScreen
    {projects} {projectName} loading={loadingProjects} creating={creatingProject} error={projectError}
    setProjectName={(value) => projectName = value} onCreate={createProject} onOpen={openProject}
  />
{:else}
<AppShell liveSummary={querySummary}>
  {#snippet titlebar()}
    <TitleBar
      project={activeProject!} currentView={currentHistory} {railCollapsed}
      inert={!!inspectorMode || tableExpanded}
      onProjects={exitProject}
      onToggleRailCollapsed={() => railCollapsed = !railCollapsed}
      onOpenRail={() => railOpen = true}
    />
  {/snippet}

  {#snippet rail()}
    <SourceRail
      {nodes} views={projectViews} selectedViewId={selectedDataset} {selectedSourceId} {loadedSourceIds} {loadingSourceId} {loadingNodes} {railOpen} collapsed={railCollapsed} {sourceOpen} {highlightToken}
      inert={!!inspectorMode || tableExpanded}
      onSelectSource={(id) => { void loadProjectSource(id); }}
      onSelectView={(id) => { void selectView(id); }}
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
      {#if !selectedDataset}
        <WelcomeScreen
          {error} {mutating} {nodes} {loadedSourceIds} {loadingSourceId}
          onSelectSource={(id) => { void loadProjectSource(id); }}
          onShowAllSources={() => { railCollapsed = false; railOpen = true; highlightToken += 1; }}
          onUpload={upload} onRetry={() => loadProjectContents(activeProject!)}
        >
          {#snippet attachForm()}
            <SourceDisclosure {mutating} {attachPath} onUpload={upload} onAttach={(event) => { event.preventDefault(); attach(); }} setAttachPath={(value) => attachPath = value} idPrefix="onboarding-database-path" showUpload={false} />
          {/snippet}
        </WelcomeScreen>
      {:else}
        <section class="workspace" inert={cellEditSaving}>
          <DatasetHead
            title={workspaceTab === 'history' ? 'Versions' : (currentHistory?.name ?? '')}
            {versionLabel} {canPreviousVersion} {canNextVersion}
            onPreviousVersion={previousVersion} onNextVersion={nextVersion}
            showMeta={workspaceTab === 'data' && !!result}
            rows={result ? count(result.total_rows) : ''}
            ms={result ? compact(result.elapsed_ms) : ''}
            showRefresh={workspaceTab === 'data'}
            onRefresh={loadActiveData}
            onExport={openExport}
            {loadingData} canExport={!!result} {exporting}
            pendingCount={currentHistory?.pendingChanges.length ?? 0}
            onStopRecording={stopRecording}
            {exportOpen}
            inert={!!inspectorMode || tableExpanded}
          >
            {#snippet exportMenu()}
              <ExportMenu
                open={exportOpen} current={currentExportOption} options={exportOptions}
                selectedKeys={exportSelectedKeys} loading={exportLoading} {exporting} error={exportError}
                setFormat={setExportFormat} onToggle={toggleExportOption}
                onExport={runExport} onClose={closeExport}
              />
            {/snippet}
          </DatasetHead>
          <DatasetTabsBar
            {workspaceTab} {tableExpanded} {rowDensity}
            historyCount={currentHistory?.versions.length ?? 0}
            onSelectData={() => workspaceTab = 'data'}
            onSelectHistory={() => { closeSql(); workspaceTab = 'history'; }}
            setRowDensity={(density) => rowDensity = density}
            onToggleExpanded={toggleTableExpanded}
          />
          {#if workspaceTab === 'history'}
            <VersionsViewsPane
              history={currentHistory} {storageError}
              onRestore={restoreVersion}
              onDiff={(version, trigger) => currentHistory && showDiff(currentHistory, version, trigger)}
            />
          {:else}
            {#if recordingNotice}<div class="banner" role="status">{recordingNotice}</div>{/if}
            {#if error}
              <div class="banner error-banner" role="alert" inert={tableExpanded}><div><strong>Request failed</strong><p>{error}</p></div><button onclick={() => loadData()}>Retry</button></div>
            {/if}
            {#if selectedDataset}
              <QueryConditionBar
                inert={!!inspectorMode || tableExpanded || loadingData}
                showBuilder={canQuery}
                {filters} {sorts} {dedupeColumns}
                {activeSql} {filterSummary}
                onToggleFilterConnector={toggleFilterConnector} onRemoveFilter={removeFilter} onRemoveSort={removeSort} onClearDedupe={clearDedupe}
                onSaveView={() => addView(result?.sql ?? activeSql)} canSaveView={!!result?.sql}
                onClearConditions={clearQuery}
                isSqlMode={false}
                onBackToFullTable={backToBuilder}
                onBackToBuilder={backToBuilder}
                {columnSearch} setColumnSearch={(value) => columnSearch = value}
                onFindColumn={findColumn} onColumnSearchKeydown={cycleColumnMatch}
                columnMatchCount={columnMatches.length}
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
                    orderedColumnNames={orderedColumns.map((column) => column.name)}
                    onBeginReorder={beginColumnReorder} onPreviewReorder={previewColumnReorder}
                    onCommitReorder={commitColumnReorder} onCancelReorder={cancelColumnReorder}
                    onMoveColumn={moveColumnOneStep} onRegexVisibility={applyRegexVisibility}
                  />
                {/snippet}
                {#snippet joinMenu()}
                  <JoinMenuPopover
                    open={queryMenuOpen === 'joins'} ontoggle={(event) => syncQueryMenu('joins', event)}
                    views={projectViews} {joinLeftViewId} {joinRightViewId} onSelectView={selectJoinView}
                    {joinLeftKeys} {joinRightKeys} onSetKeys={setJoinKeys}
                    {joinLeftColumns} {joinRightColumns} onToggleColumn={toggleJoinColumn}
                    onSelectAll={(side) => selectJoinColumns(side, [...(side === 'left' ? joinLeftVersion?.columns ?? [] : joinRightVersion?.columns ?? [])])}
                    onSelectNone={(side) => selectJoinColumns(side, [])}
                    {joinPreview} previewLoading={joinPreviewLoading} previewError={joinPreviewError}
                    onCheck={checkJoin} canCheck={canPreviewJoin} {count}
                    onRun={runJoin} canRun={canRunJoin} running={loadingData}
                  />
                {/snippet}
                {#snippet aggregateMenu()}
                  <AggregateMenuPopover
                    open={queryMenuOpen === 'aggregate'} ontoggle={(event) => syncQueryMenu('aggregate', event)}
                    label={aggregateMenuLabel}
                    {aggregateColumnSearch} setAggregateColumnSearch={(value) => aggregateColumnSearch = value}
                    {aggregateColumnMatches} {aggregateColumns} {aggregateFields} {focusedAggregateColumn}
                    onToggleColumn={toggleAggregateColumn} onRemoveColumn={removeAggregateColumn}
                    onFocusAggregate={focusAggregate} onToggleRole={toggleAggregateRole}
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
                  onRun={runSqlAndAddView}
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
                <section class="table-pane {rowDensity}" aria-label="View rows" inert={!!inspectorMode}>
                  <div class="table-card" class:recording={!!currentHistory?.pendingChanges.length} aria-busy={loadingData}>
                    {#if loadingData && !result}
                      <div class="table-state"><span class="spinner"></span>Loading rows…</div>
                    {:else if result && result.rows.length === 0}
                      <div class="table-state"><strong>No matching rows</strong><span>{queryMode === 'sql' ? 'The SQL query returned no rows.' : 'Change or remove filters to see more data.'}</span></div>
                    {:else if result}
                      <DataGridTable
                        columns={visibleColumns} bodyColumns={rowColumns} rows={result.rows}
                        caption={`Rows from ${currentHistory?.name ?? selectedDataset}`}
                        {rowDensity}
                        {canQuery} canInsert={!loadingData} canEdit={!loadingData} {sorts} {filters} {columnLabelParts} {isColumnProtected}
                        onSort={cycleSort}
                        onFilter={(column, trigger) => openFilter(column, trigger)}
                        onProfile={(column, trigger) => openStats(column, trigger)}
                        onHide={hideColumn} {display} {cellTitle}
                        {selectedCell} {editingCell} editSaving={cellEditSaving}
                        onSelectCell={selectCell} onExpandCell={expandCell} onFilterCategoricalCell={filterCategoricalCell}
                        onCellKeydown={handleCellKeydown} onCollapseCell={collapseCell}
                        onEditValue={setCellEditValue} onCommitEdit={commitCellEdit} onCancelEdit={cancelCellEdit}
                        {aggregateRowTones} setTableScroll={(el) => tableScroll = el} onInsert={openMutation}
                        onModify={modifyColumn} onDuplicate={duplicateColumn} onRename={renameColumn}
                        {renamingColumn} onStartRename={startColumnRename} onRenameValue={setColumnRenameValue}
                        onCommitRename={commitColumnRename} onCancelRename={cancelColumnRename}
                        onBeginReorder={beginColumnReorder} onPreviewReorder={previewColumnReorder}
                        onCommitReorder={commitColumnReorder} onCancelReorder={cancelColumnReorder}
                      />
                      {#if cellEditError || columnMutationError}<div class="cell-edit-error" role="alert">{cellEditError || columnMutationError}</div>{/if}
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
                          onAddNullFilter={addNullFilter}
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
{/if}

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

{#if mutationTarget}
  <FormulaMenu
    columns={result?.columns ?? []}
    targetColumn={mutationTarget.kind === 'modify' ? mutationTarget.column : null}
    boundaryLabel={mutationTarget.kind === 'modify' ? mutationTarget.column.name : mutationTarget.right ? `Between ${mutationTarget.left} and ${mutationTarget.right}` : `After ${mutationTarget.left}`}
    applying={mutationApplying} error={mutationError}
    setDialog={(element) => mutationDialog = element}
    onClose={finishMutationClose}
    onCancelAttempt={(event) => { if (mutationApplying) event.preventDefault(); }}
    onApply={applyMutation}
  />
{/if}

{#if openDiff}
  <VersionDiffDialog diff={openDiff} setDialog={(element) => diffDialog = element} onClose={closeDiff} />
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
  .table-card.recording::before, .table-card.recording::after { content: ''; position: absolute; z-index: 6; top: 0; bottom: 0; width: 2px; pointer-events: none; background: var(--action); }
  .table-card.recording::before { left: 0; }
  .table-card.recording::after { right: 0; }
  .table-state { display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 8px; height: 100%; padding: 40px; text-align: center; color: var(--muted); font-family: var(--font-ui); font-size: 13px; }
  .loading-overlay { position: absolute; inset: 0; z-index: 5; display: flex; align-items: center; justify-content: center; gap: 8px; background: rgba(255, 255, 255, 0.7); font-size: 12.5px; color: var(--muted); }
  .cell-edit-error { position: absolute; right: 10px; bottom: 10px; z-index: 7; max-width: min(460px, calc(100% - 20px)); padding: 8px 10px; border: 1px solid var(--error); border-radius: var(--radius-md); background: var(--surface); box-shadow: var(--shadow-popover); color: var(--error); font-size: 12px; }
  .banner { margin: 14px 20px; padding: 12px 14px; border-radius: var(--radius-lg); border: 1px solid var(--line); background: var(--surface-inset); font-size: 12.5px; color: var(--muted); }
  .banner strong { display: block; margin-bottom: 4px; color: var(--ink); font-size: 13px; }
  .banner p { margin: 0; }
  .error-banner { display: flex; align-items: center; justify-content: space-between; gap: 12px; border-color: var(--error); }
  .error-banner strong { color: var(--error); }
  .error-banner button { flex: none; height: 28px; padding: 0 12px; border-radius: var(--radius-md); border: 1px solid var(--control-border); background: var(--surface); }
</style>
