<script lang="ts">
  import { concatRows, type TableRows } from './lib/table-rows';
  import { onMount, onDestroy, tick } from 'svelte';
  import { basicSetup, EditorView } from 'codemirror';
  import { autocompletion, completionStatus, startCompletion, type CompletionSource } from '@codemirror/autocomplete';
  import { keywordCompletionSource, schemaCompletionSource, sql, StandardSQL, type SQLConfig, type SQLNamespace } from '@codemirror/lang-sql';
  import { keymap } from '@codemirror/view';
  import * as api from './lib/api';
  import { buildAggregateSql } from './lib/aggregate-sql';
  import { buildJoinSql } from './lib/join-sql';
  import { buildCellEditSql, buildColumnReplacementSql, buildMutationSql, hasVolatileRowOrder, nextDuplicateColumnName, quoteIdentifier } from './lib/mutation-sql';
  import { absoluteRowToPage, clampAbsoluteRow, safeTotalRows } from './lib/row-scrollbar';
  import { criticalPages, latencyEma, pagesForRange, snapshotWindow } from './lib/scroll-prefetch';
  import { LEGACY_STORAGE_KEY, LEGACY_VERSIONING_STORAGE_KEY, VERSIONING_STORAGE_KEY, activateVersion, createSourceHistory, createView, finalizeVersion, matchColumnsByRegex, migrateDatasetHistories, migrateSavedQueries, rebindLegacyHistories, stageVersionChange, versionDiff, versionLabel as formatVersionLabel } from './lib/versioning';
  import { applyRoles, chartRoles, classifyColumn, filtersFromMark, suggestCharts } from './lib/visualize';
  import { chartTitle, nextDashboardName, clampPlacement, composeFilters, DASHBOARD_STORAGE_KEY, DEFAULT_TILE, emptyDashboardDataset, filtersForChart, readDashboards, selectionChartIdAtFilterIndex, updateActiveDashboardTab, updatePlacedChart, withoutSelections } from './lib/dashboard';
  import { chartThemeCssVariables, defaultChartThemePreferences, isChartPalette, readChartThemePreferences, serializeChartThemePreferences, type ChartPalette, type ChartThemePreferences } from './lib/chartThemes';
  import type { AggregateCount, AggregateMetric, AggregateRecipeItem, BaseViewInfo, CategoryValue, ChartMark, ChartSpec, ChartType, ColumnInfo, ColumnStats, DashboardDataset, DashboardPlacement, BarLayout, DashboardSelection, DatasetVersionHistory, EncodingRole, DistributionMode, ExportFormat, ExportOption, FilterCondition, FilterOperator, JoinWorkspaceRequest, JoinWorkspaceResponse, JsonLayout, NodeInfo, ProjectInfo, QueryResponse, RowDensity, SerializableValue, SortCondition, SourceSummary, Version, VersionChange, VersionDiff, ViewHistory, VisualizeResponse, WorkbookPreview } from './lib/types';

  import { editorHighlight, editorTheme } from './lib/editorTheme';
  import { readThemePreference, resolveScheme, themeStorageKey, type ColorScheme, type ThemePreference } from './lib/theme';
  import { commandFor, combinationTimeoutMs, operationsFor, shortcuts, toolbarStorageKey, actionMenuStorageKey, chartThemeStorageKey, type ActionMenuMode, type ChartTheme, type CommandPrefix, type ToolbarVisibility } from './lib/commands';
  import CommandHint from './components/molecules/CommandHint.svelte';
  import CommandDialog from './components/organisms/CommandDialog.svelte';
  import SettingsPage from './components/organisms/SettingsPage.svelte';
  import CellFinder from './components/organisms/CellFinder.svelte';
  import Button from './components/atoms/Button.svelte';
  import TitleBar from './components/organisms/TitleBar.svelte';
  import SourceRail from './components/organisms/SourceRail.svelte';
  import SourceDisclosure from './components/organisms/SourceDisclosure.svelte';
  import WelcomeScreen from './components/organisms/WelcomeScreen.svelte';
  import DatasetHead from './components/organisms/DatasetHead.svelte';
  import VersionMenu from './components/organisms/VersionMenu.svelte';
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
  import VisualizeStage from './components/organisms/VisualizeStage.svelte';
  import DashboardStage from './components/organisms/DashboardStage.svelte';
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
  type JoinStep = 0 | 1 | 2;


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
  let aggregateRecipe = $state.raw<AggregateRecipeItem[]>([]);
  let focusedAggregateItemId = $state<number | null>(null);
  let nextAggregateRecipeId = 0;
  let aggregateSourceSql = $state('');
  let aggregateSourceColumns = $state.raw<ColumnInfo[]>([]);
  let joinLeftViewId = $state('');
  let joinRightViewId = $state('');
  let joinLeftSourceId = $state('');
  let joinRightSourceId = $state('');
  let joinLeftKeys = $state<string[]>([]);
  let joinRightKeys = $state<string[]>([]);
  let joinLeftColumns = $state<string[]>([]);
  let joinRightColumns = $state<string[]>([]);
  let joinStep = $state<JoinStep>(0);
  let joinStepDirection = $state<-1 | 1>(1);
  let joinSourceSide = $state<'left' | 'right'>('right');
  let joinPreview = $state.raw<JoinWorkspaceResponse | null>(null);
  let joinPreviewLoading = $state(false);
  let joinPreviewError = $state('');
  let hiddenColumns = $state<string[]>([]);
  let columnOrder = $state<string[]>([]);
  let reorderOrigin = $state.raw<string[] | null>(null);
  let lastHiddenColumn = $state<string | null>(null);
  let railCollapsed = $state(false);
  let fitColumnsToContent = $state(false);
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
  let columnMenuRegex = $state(false);
  let nullThreshold = $state(100);
  let activeColumnMatch = $state(0);
  let selectedCell = $state<{ row: number; column: string; expanded?: boolean } | null>(null);
  let pinnedColumns = $state<string[]>([]);
  let editingCell = $state<{ row: number; column: string; value: string; original: string } | null>(null);
  let cellEditSaving = $state(false);
  let cellEditError = $state('');
  let columnMutationError = $state('');
  let renamingColumn = $state<{ original: string; value: string } | null>(null);
  let tableScroll = $state<HTMLDivElement | null>(null);
  let gridApi = $state<{ scrollToAbsoluteRow: (absolute: number, onlyIfOutside?: boolean) => void; requestPin: (column: ColumnInfo) => void } | null>(null);
  type CachedPage = { page: number; rows: TableRows };
  // One previous page, at most two incoming pages, and one viewport preview.
  let neighborCache = $state.raw<CachedPage | null>(null);
  let aheadCache = $state.raw<CachedPage[]>([]);
  let snapshotCache = $state.raw<{ start: number; rows: TableRows } | null>(null);
  let pendingSelect: { absRow: number; column: string } | null = null;
  let renderedQueryKey = $state('');
  let lastDir: 1 | -1 = 1;
  let dragHeld = false;
  let latencyMs = 300;
  type ViewportReport = { firstRow: number; lastRow: number; velocity: number };
  let latestViewport: ViewportReport | null = null;
  const pendingPages = new Map<number, AbortController>();
  let snapshotRequest: AbortController | null = null;
  const fetchFailures = new Map<number, { at: number; message: string }>();
  let gridLoadError = $state('');
  const FETCH_FAIL_COOLDOWN_MS = 5000;

  function currentQueryKey(): string {
    return JSON.stringify({
      node: activeSqlNodeId || selectedNodeId,
      sql: sqlBase || activeSql || activeVersion?.sql || '',
      filters, sorts, dedupe: dedupeColumns, size: pageSize,
    });
  }
  function cancelSnapshot() {
    snapshotRequest?.abort();
    snapshotRequest = null;
  }
  function resetBackground() {
    for (const request of pendingPages.values()) request.abort();
    pendingPages.clear();
    cancelSnapshot();
    neighborCache = null;
    aheadCache = [];
    snapshotCache = null;
    pendingSelect = null;
    latestViewport = null;
    fetchFailures.clear();
    gridLoadError = '';
    dragHeld = false;
  }
  onDestroy(resetBackground);
  $effect(() => {
    const key = currentQueryKey();
    if (key !== renderedQueryKey) {
      resetBackground();
      renderedQueryKey = key;
    }
  });
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
  let exportJsonLayout = $state<JsonLayout>('rows');
  let exportOptions = $state.raw<ExportOption[]>([]);
  let exportSelectedKeys = $state<string[]>(['current']);
  let exportLoading = $state(false);
  let exporting = $state(false);
  let exportError = $state('');
  let exportTrigger = $state<HTMLButtonElement | null>(null);
  let exportRequestId = 0;
  let versionsOpen = $state(false);

  let canvasMode = $state<'rows' | 'chart' | 'dashboard'>('rows');
  let visualizeColumns = $state.raw<ColumnInfo[]>([]);
  let visualizeChart = $state<ChartType | null>(null);
  let visualizeMetric = $state<AggregateMetric | null>(null);
  let visualizeRoles = $state<Record<string, EncodingRole>>({});
  let visualizeLayout = $state<BarLayout | null>(null);
  let visualizeSearch = $state('');
  let visualizeData = $state.raw<VisualizeResponse | null>(null);
  let visualizeLoading = $state(false);
  let visualizeError = $state('');
  let visualizeRequestId = 0;
  let dashboardDocuments = $state.raw<DashboardDataset[]>([]);
  let dashboardSelections = $state.raw<DashboardSelection[]>([]);
  let dashboardChartStates = $state.raw<Record<string, { data: VisualizeResponse | null; loading: boolean; error: string }>>({});
  let dashboardRequestId = 0;
  const implementedCharts = new Set<ChartType>(['bar', 'histogram', 'box', 'scatter']);

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
  type UndoPoint = {
    historyId: string;
    sqlBase: string; activeSql: string; activeSqlNodeId: string; sqlText: string;
    columnOrder: string[]; hiddenColumns: string[];
    filters: FilterCondition[]; sorts: SortCondition[]; dedupeColumns: string[];
    dashboardSelections: DashboardSelection[];
    join: JoinWorkspaceRequest | undefined; page: number;
  };
  // One entry per staged pending change, kept aligned by index with pendingChanges.
  let undoStack = $state.raw<UndoPoint[]>([]);
  let openDiff = $state.raw<VersionDiff | null>(null);
  let diffDialog = $state<HTMLDialogElement | null>(null);
  let diffReturnFocus: HTMLElement | null = null;
  let editorHost = $state<HTMLDivElement | null>(null);
  let editorView: EditorView | null = null;
  let queryMenuOpen = $state<'columns' | 'joins' | 'aggregate' | 'dedupe' | null>(null);

  let themePreference = $state<ThemePreference>('system');
  let colorScheme = $state<ColorScheme>('light');
  let darkQuery: MediaQueryList | undefined;
  let toolbarVisibility = $state<ToolbarVisibility>('show');
  let actionMenuMode = $state<ActionMenuMode>('simple');
  let chartTheme = $state(defaultChartThemePreferences.mode);
  let chartPalettes = $state<ChartThemePreferences['palettes']>({ ...defaultChartThemePreferences.palettes });
  let chartPalette = $derived(chartPalettes[chartTheme]);
  let chartThemeKey = $derived(`${chartTheme}:${chartPalette}`);
  let settingsOpen = $state(false);
  let densityMenuOpen = $state(false);
  let settingsError = $state('');
  let commandPrefix = $state<CommandPrefix>(null);
  let combinationTimer: ReturnType<typeof setTimeout> | undefined;
  let combinationId = $state(0);
  let commandMode = $state<'wheel' | 'find-column' | 'help' | 'operation' | 'sort' | null>(null);
  let commandQuery = $state('');
  let columnTarget = $state('');
  let pendingColumnAction = '';
  let commandReturnFocus: HTMLElement | null = null;
  let sourcePicking = $state(false);
  let sourceDigits = $state('');
  let sourceNumberTimer: ReturnType<typeof setTimeout> | undefined;
  let cellFinderOpen = $state(false);
  let cellSearchTerm = $state('');
  let cellSearchNotice = $state('');
  let cellSearching = $state(false);
  let cellMatch: api.CellMatch | null = null;
  let cellSearchKey = '';
  let cellSearchRequest: AbortController | null = null;
  let redoStack = $state.raw<{ point: UndoPoint; history: ViewHistory }[]>([]);
  let versionRedo = $state.raw<{ historyId: string; versionId: string }[]>([]);
  let historyBusy = $state(false);


  /* The scheme is applied by the resolved preference rather than by a bare media query, so
     an explicit Light or Dark keeps winning when the operating system changes under it. */
  function applyColorScheme() {
    colorScheme = resolveScheme(themePreference, darkQuery?.matches ?? false);
    document.documentElement.dataset.theme = colorScheme;
    applyChartThemeColors();
  }
  function setThemePreference(value: ThemePreference) {
    themePreference = value;
    /* One authored moment: the workspace already on screen crossfades into the other
       scheme instead of blinking. Browsers without view transitions, and anyone who asked
       for less motion, get the switch immediately. */
    const reduced = window.matchMedia('(prefers-reduced-motion: reduce)').matches;
    if (!reduced && typeof document.startViewTransition === 'function') document.startViewTransition(() => applyColorScheme());
    else applyColorScheme();
    try { localStorage.setItem(themeStorageKey, value); settingsError = ''; }
    catch { settingsError = 'This preference could not be saved. It will last until you close Quark.'; }
  }
  function setToolbarVisibility(value: ToolbarVisibility) {
    toolbarVisibility = value;
    try { localStorage.setItem(toolbarStorageKey, value); settingsError = ''; }
    catch { settingsError = 'This preference could not be saved. It will last until you close Quark.'; }
  }
  function setActionMenuMode(value: ActionMenuMode) {
    actionMenuMode = value;
    try { localStorage.setItem(actionMenuStorageKey, value); settingsError = ''; }
    catch { settingsError = 'This preference could not be saved. It will last until you close Quark.'; }
  }
  function applyChartThemeColors() {
    const root = document.documentElement;
    const preferences: ChartThemePreferences = { mode: chartTheme, palettes: { ...chartPalettes } };
    const legacyTheme = chartTheme === 'single' ? 'primary' : chartTheme === 'multicolor' ? 'rich' : 'monotone';
    root.dataset.chartTheme = legacyTheme;
    root.dataset.chartMode = chartTheme;
    root.dataset.chartPalette = chartPalettes[chartTheme];
    for (const [name, value] of Object.entries(chartThemeCssVariables(preferences, colorScheme))) root.style.setProperty(name, value);
  }
  function persistChartThemePreferences() {
    const preferences: ChartThemePreferences = { mode: chartTheme, palettes: { ...chartPalettes } };
    try { localStorage.setItem(chartThemeStorageKey, serializeChartThemePreferences(preferences)); settingsError = ''; }
    catch { settingsError = 'This preference could not be saved. It will last until you close Quark.'; }
  }
  function setChartTheme(value: ChartTheme) {
    chartTheme = value;
    applyChartThemeColors();
    persistChartThemePreferences();
  }
  function setChartPalette(value: ChartPalette) {
    if (!isChartPalette(chartTheme, value)) return;
    chartPalettes = { ...chartPalettes, [chartTheme]: value };
    applyChartThemeColors();
    persistChartThemePreferences();
  }
  function clearCommandSequence() {
    commandPrefix = null; sourcePicking = false; sourceDigits = '';
    clearTimeout(sourceNumberTimer);
    clearTimeout(combinationTimer);
  }
  function startCombinationTimer() {
    clearTimeout(combinationTimer);
    combinationId++;
    combinationTimer = setTimeout(clearCommandSequence, combinationTimeoutMs);
  }
  function closeCommands(restoreFocus = true) {
    commandMode = null; queryMenuOpen = null; densityMenuOpen = false; pendingColumnAction = '';
    if (restoreFocus) void tick().then(() => commandReturnFocus?.isConnected && commandReturnFocus.focus());
  }
  function showCommands(mode: typeof commandMode) {
    if (!commandMode) commandReturnFocus = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    clearCommandSequence();
    queryMenuOpen = null; densityMenuOpen = false; versionsOpen = false;
    commandQuery = ''; commandMode = mode;
  }
  function openQueryMenu(menu: NonNullable<typeof queryMenuOpen>) {
    queryMenuOpen = menu;
    if (menu === 'joins') {
      joinStep = 0; joinStepDirection = 1;
      joinSourceSide = joinLeftViewId || joinLeftSourceId ? 'right' : 'left';
      railCollapsed = false; prepareJoinPicker();
    }
  }
  async function selectCommandColumn(name: string) {
    const followup = pendingColumnAction;
    closeCommands(false);
    columnTarget = name; selectedCell = null;
    await scrollToColumn(name);
    const header = [...(tableScroll?.querySelectorAll<HTMLElement>('th[data-column]') ?? [])].find((item) => item.dataset.column === name);
    header?.focus({ preventScroll: true });
    if (followup) await executeCommand(followup);
  }
  async function chooseCommand(id: string) {
    if (commandMode === 'find-column') { await selectCommandColumn(id); return; }
    if (commandMode === 'sort') {
      const column = commandColumn;
      closeCommands();
      if (column) await setColumnSort(column, id as 'asc' | 'desc' | 'none');
      return;
    }
    await executeCommand(id);
  }
  async function executeCommand(id: string) {
    if (id === 'wheel') { commandMode === 'wheel' ? closeCommands() : showCommands('wheel'); return; }
    if (id === 'settings') { closeCommands(false); clearCommandSequence(); settingsOpen = !settingsOpen; return; }
    if (id === 'sidebar') { closeCommands(false); clearCommandSequence(); railCollapsed = !railCollapsed; return; }
    if (id === 'sources') { closeCommands(false); clearCommandSequence(); railCollapsed = false; sourcePicking = true; startCombinationTimer(); highlightToken++; return; }
    if (id === 'help') { showCommands('help'); return; }
    if (!result || loadingData || cellEditSaving || historyBusy) return;
    if (['columns', 'joins', 'aggregate', 'dedupe'].includes(id)) {
      showCommands('operation'); openQueryMenu(id as NonNullable<typeof queryMenuOpen>);
      await tick();
      document.querySelector<HTMLElement>('.popover-host[open] .popover input, .popover-host[open] .popover button, .detached-menus .popover input, .detached-menus .popover button')?.focus();
      return;
    }
    if (id === 'find-column') { showCommands('find-column'); return; }
    if (id === 'find-values') { closeCommands(false); cellFinderOpen = true; await tick(); document.querySelector<HTMLInputElement>('.finder input')?.focus(); return; }
    if (id === 'density') {
      closeCommands(false); densityMenuOpen = true;
      await tick(); document.querySelector<HTMLElement>('.density-menu button')?.focus(); return;
    }
    if (id === 'filter' || id === 'sort' || id === 'hide' || id === 'pin') {
      const column = commandColumn;
      if (!column) { showCommands('find-column'); pendingColumnAction = id; return; }
      if (id === 'sort') { showCommands('sort'); return; }
      closeCommands(false);
      if (id === 'filter') await openFilter(column);
      if (id === 'hide') { hideColumn(column.name); columnTarget = ''; selectedCell = null; tableScroll?.focus(); }
      if (id === 'pin') gridApi?.requestPin(column);
      return;
    }
    closeCommands(false);
    if (id === 'fit') fitColumnsToContent = !fitColumnsToContent;
    if (id === 'sql') await openSql();
    if (id === 'versions') toggleVersions();
    if (id === 'refresh') await loadActiveData();
    if (id === 'save') await stopRecording();
    if (id === 'undo') await undoCommand();
    if (id === 'redo') await redoCommand();
  }
  function captureCommands(event: KeyboardEvent) {
    if (!activeProject || event.defaultPrevented || event.isComposing) return;
    const target = event.target instanceof HTMLElement ? event.target : null;
    const editable = isEditableElement(target);
    if (target?.closest('dialog') || inspectorMode || settingsOpen) {
      if (settingsOpen && event.key === 'Escape') { event.preventDefault(); event.stopImmediatePropagation(); settingsOpen = false; void tick().then(() => tableScroll?.focus()); }
      // Dialogs and editors own their keys. The wheel still has its toggle.
      if (commandMode && !editable && (event.metaKey || event.ctrlKey) && event.key.toLowerCase() === 'a') { event.preventDefault(); event.stopImmediatePropagation(); closeCommands(); }
      return;
    }
    if (event.key === 'Escape' && queryMenuOpen) {
      event.preventDefault(); event.stopImmediatePropagation();
      if (commandMode === 'operation') showCommands('wheel'); else { queryMenuOpen = null; tableScroll?.focus(); }
      return;
    }
    if (sourcePicking && !editable && !event.altKey && ((!event.metaKey && !event.ctrlKey) || /^[0-9]$/.test(event.key))) {
      event.preventDefault(); event.stopImmediatePropagation();
      if (/^[0-9]$/.test(event.key)) {
        startCombinationTimer();
        sourceDigits += event.key;
        clearTimeout(sourceNumberTimer);
        const index = Number(sourceDigits) - 1;
        const commit = () => { const node = nodes[index]; clearCommandSequence(); if (node) void loadProjectSource(node.id); };
        if (nodes.some((_, i) => String(i + 1).startsWith(sourceDigits) && String(i + 1) !== sourceDigits)) sourceNumberTimer = setTimeout(commit, 900);
        else commit();
      } else if (event.key === 'Enter' && sourceDigits) { const node = nodes[Number(sourceDigits) - 1]; clearCommandSequence(); if (node) void loadProjectSource(node.id); }
      else if (event.key === 'Escape') clearCommandSequence();
      return;
    }
    const action = commandFor(event, commandPrefix, editable);
    if (!action) return;
    event.preventDefault(); event.stopImmediatePropagation();
    if (event.repeat) return;
    clearCommandSequence();
    if (action === 'prefix-find') commandPrefix = 'find';
    else if (action === 'prefix-column') commandPrefix = 'column';
    else if (action === 'prefix-sidebar') commandPrefix = 'sidebar';
    else if (action !== 'cancel') void executeCommand(action);
    if (commandPrefix) startCombinationTimer();
  }
  function clearSequenceOnEdit(event: FocusEvent) { if (isEditableElement(event.target as Element)) clearCommandSequence(); }
  onMount(() => {
    try {
      themePreference = readThemePreference(localStorage.getItem(themeStorageKey));
      const value = localStorage.getItem(toolbarStorageKey); if (value === 'show' || value === 'hover' || value === 'hide') toolbarVisibility = value;
      const menu = localStorage.getItem(actionMenuStorageKey); if (menu === 'simple' || menu === 'comprehensive') actionMenuMode = menu;
      const chartPreferences = readChartThemePreferences(localStorage.getItem(chartThemeStorageKey));
      chartTheme = chartPreferences.mode;
      chartPalettes = { ...chartPreferences.palettes };
    }
    catch { settingsError = 'Preferences are unavailable in this browser.'; }
    darkQuery = window.matchMedia('(prefers-color-scheme: dark)');
    darkQuery.addEventListener('change', applyColorScheme);
    applyColorScheme();
    window.addEventListener('keydown', captureCommands, true);
    window.addEventListener('blur', clearCommandSequence);
    window.addEventListener('focusin', clearSequenceOnEdit);
    window.addEventListener('pointerdown', clearCommandSequence);
    return () => { darkQuery?.removeEventListener('change', applyColorScheme); window.removeEventListener('keydown', captureCommands, true); window.removeEventListener('blur', clearCommandSequence); window.removeEventListener('focusin', clearSequenceOnEdit); window.removeEventListener('pointerdown', clearCommandSequence); clearCommandSequence(); cellSearchRequest?.abort(); };
  });
  function resetCellSearch() { cellSearchRequest?.abort(); cellSearching = false; cellMatch = null; cellSearchNotice = ''; }
  async function findCellValue(direction: 'next' | 'previous') {
    if (!result || !cellSearchTerm || cellSearching || !visibleColumns.length) return;
    cellSearchRequest?.abort();
    const controller = new AbortController(); cellSearchRequest = controller;
    const key = JSON.stringify([currentQueryKey(), visibleColumns.map((column) => column.name), cellSearchTerm]);
    if (key !== cellSearchKey) cellMatch = null;
    cellSearchKey = key;
    cellSearching = true; cellSearchNotice = '';
    try {
      const { match } = await api.findCell(activeSqlNodeId || selectedNodeId, {
        sql: result.sql, term: cellSearchTerm, columns: visibleColumns.map((column) => column.name),
        after_row: cellMatch?.row ?? -1, after_column: cellMatch?.column_index ?? -1, direction,
      }, controller.signal);
      if (controller.signal.aborted || key !== JSON.stringify([currentQueryKey(), visibleColumns.map((column) => column.name), cellSearchTerm])) return;
      cellMatch = match;
      if (!match) { cellSearchNotice = 'No matching values in this View.'; return; }
      const desiredPage = Math.floor(match.row / pageSize) + 1;
      if (page !== desiredPage) { page = desiredPage; if (!await loadData()) throw new Error(sqlError || 'Could not load the matching row.'); }
      if (controller.signal.aborted) return;
      selectedCell = { row: match.row % pageSize, column: match.column };
      columnTarget = match.column;
      await scrollToColumn(match.column);
      gridApi?.scrollToAbsoluteRow(match.row);
      cellSearchNotice = `Row ${match.row + 1} · ${match.column}`;
    } catch (reason) { if (!controller.signal.aborted) cellSearchNotice = message(reason); }
    finally { if (cellSearchRequest === controller) cellSearching = false; }
  }

  let currentHistory = $derived(versionHistories.find((history) => history.id === selectedDataset));
  let selectedNode = $derived(nodes.find((node) => node.id === currentHistory?.sourceId));
  let selectedSourceId = $derived(currentHistory?.sourceId ?? '');
  let currentDataset = $derived(datasets.find((dataset) => dataset.id === selectedDataset));
  let activeProjectId = $derived(activeProject?.id ?? '');
  let projectViews = $derived(activeProjectId ? versionHistories.filter((history) => history.projectId === activeProjectId) : []);
  let activeVersion = $derived(currentHistory?.versions.find((version) => version.id === currentHistory.activeVersionId));
  let activeVersionChildren = $derived(activeVersion ? currentHistory?.versions.filter((version) => version.parentId === activeVersion.id) ?? [] : []);
  let versionLabel = $derived(activeVersion ? `${formatVersionLabel(activeVersion)} · ${currentHistory?.versions.length ?? 0} saved` : '');
  let canPreviousVersion = $derived(!loadingData && !!activeVersion?.parentId);
  let canNextVersion = $derived(!loadingData && activeVersionChildren.length === 1);
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
  let commandColumn = $derived(visibleColumns.find((column) => column.name === (selectedCell?.column || columnTarget)));
  let commandItems = $derived(commandMode === 'wheel' ? operationsFor(actionMenuMode).map((item) => ({ ...item, shortcut: item.key.toUpperCase(), disabled: !result || loadingData || historyBusy }))
    : commandMode === 'find-column' ? visibleColumns.filter((column) => commandQuery.trim() && column.name.toLowerCase().includes(commandQuery.trim().toLowerCase())).map((column) => ({ id: column.name, label: column.name, detail: column.type }))
    : commandMode === 'sort' ? [{ id: 'asc', label: 'Ascending' }, { id: 'desc', label: 'Descending' }, { id: 'none', label: 'Remove sort' }]
    : shortcuts.filter(([, label]) => label.toLowerCase().includes(commandQuery.toLowerCase())).map(([shortcut, label, id]) => ({ id, label, shortcut })));
  // A pin only survives while its column is still on screen.
  let livePins = $derived(pinnedColumns.filter((name) => visibleColumns.some((column) => column.name === name)));
  let rowColumns = $derived.by(() => {
    if (!reorderOrigin) return visibleColumns;
    const byName = new Map((result?.columns ?? []).map((column) => [column.name, column]));
    return reorderOrigin.map((name) => byName.get(name)).filter((column): column is ColumnInfo => !!column && !hiddenColumns.includes(column.name));
  });
  let aggregateFieldOptions = $derived((aggregateSourceColumns.length ? aggregateSourceColumns : result?.columns ?? []).filter((column) => column.profile_kind !== null));
  let aggregateColumnMatches = $derived.by(() => { const query = aggregateColumnSearch.trim().toLowerCase(); return query ? aggregateFieldOptions.filter((column) => column.name.toLowerCase().includes(query)) : aggregateFieldOptions; });
  let aggregateFields = $derived(aggregateRecipe.filter((item) => item.metrics !== null));
  let aggregateIndexes = $derived(aggregateRecipe.filter((item) => item.metrics === null).map((item) => item.column));
  let focusedAggregateItem = $derived(aggregateRecipe.find((item) => item.id === focusedAggregateItemId));
  let aggregateMetrics = $derived(focusedAggregateItem?.metrics ?? []);
  let selectedAggregateColumn = $derived(aggregateFieldOptions.find((column) => column.name === focusedAggregateItem?.column));
  let availableAggregateMetrics = $derived(aggregateMetricOptions.filter((metric) => (!metric.numeric || selectedAggregateColumn?.numeric) && (!metric.ordered || selectedAggregateColumn?.numeric || selectedAggregateColumn?.profile_kind === 'date')));
  let canCreateAggregate = $derived(aggregateRecipe.length > 0 && aggregateFields.some((item) => (item.metrics?.length ?? 0) > 0));
  let visualizeFieldOptions = $derived((result?.columns ?? []).filter((column) => classifyColumn(column) !== null));
  let visualizeSuggestions = $derived(suggestCharts(visualizeColumns).filter((item) => implementedCharts.has(item.chart)));
  let visualizeSpec = $derived.by((): ChartSpec | null => {
    const suggestions = visualizeSuggestions;
    if (!suggestions.length) return null;
    const pick = (visualizeChart && suggestions.find((item) => item.chart === visualizeChart)) || suggestions[0];
    const encodings = applyRoles(pick.encodings, pick.chart, visualizeRoles, visualizeColumns);
    const metric = pick.chart === 'bar' && encodings.value
      ? (visualizeMetric ?? pick.metric ?? 'avg')
      : pick.metric;
    const layout = pick.chart === 'bar' && encodings.group ? (visualizeLayout ?? 'grouped') : undefined;
    const base: ChartSpec = metric
      ? { chart: pick.chart, encodings, metric }
      : { chart: pick.chart, encodings };
    return layout ? { ...base, layout } : base;
  });
  let currentDashboard = $derived(dashboardDocuments.find((document) => document.datasetId === selectedDataset));
  let columnMatches = $derived.by(() => { const query = columnSearch.trim().toLowerCase(); return query ? visibleColumns.filter((column) => column.name.toLowerCase().includes(query)) : []; });
  let columnMenuRegexResult = $derived(matchColumnsByRegex(columnOrder, columnMenuSearch.trim()));
  let columnMenuItems = $derived.by(() => {
    const query = columnMenuSearch.trim();
    if (!query) return orderedColumns;
    if (!columnMenuRegex) return orderedColumns.filter((column) => column.name.toLowerCase().includes(query.toLowerCase()));
    if (columnMenuRegexResult.error) return [];
    const matches = new Set(columnMenuRegexResult.matches);
    return orderedColumns.filter((column) => matches.has(column.name));
  });
  let columnTypes = $derived([...new Set((result?.columns ?? []).map((column) => column.type))]);
  let columnTypeCounts = $derived.by(() => { const counts: Record<string, number> = Object.create(null); for (const column of result?.columns ?? []) counts[column.type] = (counts[column.type] ?? 0) + 1; return counts; });
  let aggregateRowTones = $derived.by(() => {
    const rows = result?.rows;
    const majorIndex = queryMode === 'sql' && aggregateSourceSql ? aggregateIndexes[0] : '';
    if (!majorIndex || !rows) return [];
    let alternate = false;
    return Array.from({ length: rows.length }, (_, index) => { if (index && !Object.is(rows.cell(index, majorIndex), rows.cell(index - 1, majorIndex))) alternate = !alternate; return alternate; });
  });
  let operators = $derived.by(() => filterColumn ? [...baseOperators, ...(!filterColumn.numeric && isTextType(filterColumn.type) ? textOperators : []), ...(filterColumn.numeric || isOrderedType(filterColumn.type) ? orderedOperators : [])] : baseOperators);
  let maxBin = $derived(stats && stats.kind !== 'categorical' && stats.histogram.length ? Math.max(...stats.histogram.map((bin) => Number(bin.count)), 1) : 1);
  let querySummary = $derived(result ? queryMode === 'sql' ? `${count(result.total_rows)} SQL result rows, page ${result.page} of ${count(result.total_pages)}.` : `${count(result.total_rows)} rows, page ${result.page} of ${count(result.total_pages)}, ${filters.length} filters, ${sorts.length} sorts, and ${dedupeColumns.length} dedupe keys.` : '');

  onMount(() => { loadVersioning(); loadDashboards(); void loadProjects(); return () => editorView?.destroy(); });

  function message(reason: unknown): string { return reason instanceof Error ? reason.message : 'Something went wrong'; }
  function isOrderedType(type: string): boolean { return /VARCHAR|CHAR|TEXT|DATE|TIME|INT|DECIMAL|NUMERIC|REAL|FLOAT|DOUBLE/i.test(type); }
  function isTextType(type: string): boolean { return /VARCHAR|CHAR|TEXT/i.test(type); }
  function isBooleanType(type: string): boolean { return type.toLowerCase() === 'boolean'; }
  function resetVisualizeDraft() {
    visualizeColumns = [];
    visualizeChart = null;
    visualizeMetric = null;
    visualizeRoles = {};
    visualizeLayout = null;
    visualizeSearch = '';
    visualizeData = null;
    visualizeError = '';
    dashboardRequestId++;
    dashboardSelections = [];
    dashboardChartStates = {};
    canvasMode = 'rows';
  }

  function setCanvasMode(mode: 'rows' | 'chart' | 'dashboard') {
    canvasMode = mode;
    if (mode === 'chart') void loadVisualize();
    if (mode === 'dashboard') void loadDashboardCharts();
  }

  function toggleVisualizeColumn(name: string) {
    if (visualizeColumns.some((column) => column.name === name)) {
      visualizeColumns = visualizeColumns.filter((column) => column.name !== name);
    } else {
      const column = result?.columns.find((item) => item.name === name);
      if (!column) return;
      visualizeColumns = [...visualizeColumns, column];
    }
    if (visualizeRoles[name] && !visualizeColumns.some((column) => column.name === name)) {
      const { [name]: _dropped, ...rest } = visualizeRoles;
      visualizeRoles = rest;
    }
    visualizeSearch = '';
    if (visualizeChart && !suggestCharts(visualizeColumns).some((item) => item.chart === visualizeChart && implementedCharts.has(item.chart))) {
      visualizeChart = null;
    }
    void loadVisualize();
  }

  function selectVisualizeLayout(layout: BarLayout) {
    visualizeLayout = layout;
    void loadVisualize();
  }

  function setVisualizeRole(name: string, role: EncodingRole) {
    visualizeRoles = { ...visualizeRoles, [name]: role };
    void loadVisualize();
  }

  function visualizeRoleOf(name: string): EncodingRole | null {
    const encodings = visualizeSpec?.encodings;
    if (!encodings) return null;
    const entry = (Object.entries(encodings) as [EncodingRole, string][]).find(([, value]) => value === name);
    return entry ? entry[0] : null;
  }

  function selectVisualizeChart(chart: ChartType) {
    visualizeChart = chart;
    void loadVisualize();
  }

  function selectVisualizeMetric(metric: AggregateMetric) {
    visualizeMetric = metric;
    void loadVisualize();
  }

  async function loadVisualize() {
    const spec = visualizeSpec;
    if (canvasMode !== 'chart') return;
    if (!spec || !implementedCharts.has(spec.chart)) {
      visualizeData = null;
      visualizeError = '';
      visualizeLoading = false;
      return;
    }
    const id = ++visualizeRequestId;
    visualizeLoading = true;
    visualizeError = '';
    try {
      const body = { spec, page, page_size: pageSize, filters, sorts, dedupe_columns: dedupeColumns };
      const next = queryMode === 'sql'
        ? await api.visualizeSql(activeSqlNodeId || selectedNodeId, { ...body, sql: sqlBase || activeSql })
        : await api.visualizeDataset(selectedNodeId, selectedDataset, body);
      if (id !== visualizeRequestId) return;
      visualizeData = next;
    } catch (reason) {
      if (id === visualizeRequestId) {
        visualizeError = message(reason);
        visualizeData = null;
      }
    } finally {
      if (id === visualizeRequestId) visualizeLoading = false;
    }
  }

  async function applyChartMark(mark: Parameters<typeof filtersFromMark>[1]) {
    if (!visualizeSpec) return;
    const base = withoutSelections(filters, dashboardSelections);
    const added = filtersFromMark(visualizeSpec, mark, base.length);
    if (!added.length) return;
    const summary = added.map(filterSummary).join(' and ');
    await applyFilterChange(composeFilters([...base, ...added], dashboardSelections), { kind: 'filter', summary: `Filter ${summary}`, details: { source: 'chart' } });
  }

  function loadDashboards() {
    try { dashboardDocuments = readDashboards(localStorage.getItem(DASHBOARD_STORAGE_KEY)); }
    catch { dashboardDocuments = []; }
  }

  function persistDashboards(next: DashboardDataset[]): boolean {
    try {
      localStorage.setItem(DASHBOARD_STORAGE_KEY, JSON.stringify(next));
      dashboardDocuments = next;
      storageError = '';
      return true;
    } catch {
      storageError = 'Dashboards could not be stored in this browser.';
      return false;
    }
  }

  function updateCurrentDashboard(update: (document: DashboardDataset) => DashboardDataset): DashboardDataset | null {
    if (!selectedDataset) return null;
    const document = currentDashboard ?? emptyDashboardDataset(selectedDataset);
    const next = update(document);
    const documents = currentDashboard
      ? dashboardDocuments.map((item) => item.datasetId === selectedDataset ? next : item)
      : [...dashboardDocuments, next];
    return persistDashboards(documents) ? next : null;
  }

  function createDashboardTab(): string | null {
    const id = crypto.randomUUID();
    return updateCurrentDashboard((document) => ({
      ...document,
      activeTabId: id,
      tabs: [...document.tabs, { id, name: nextDashboardName(document.tabs.map((tab) => tab.name)), scrollTop: 0, placements: [] }]
    })) ? id : null;
  }

  function selectDashboardTab(id: string) {
    updateCurrentDashboard((document) => document.tabs.some((tab) => tab.id === id) ? { ...document, activeTabId: id } : document);
  }

  function addChartToDashboard(): boolean {
    const tab = currentDashboard?.tabs.find((item) => item.id === currentDashboard.activeTabId);
    return placeCurrentChart({ x: 20,
      y: Math.max(20, ...(tab?.placements.map((item) => item.y + item.height + 20) ?? [0])),
      ...DEFAULT_TILE
    });
  }

  function placeCurrentChart(rect: Pick<DashboardPlacement, 'x' | 'y' | 'width' | 'height'>) {
    const spec = visualizeSpec;
    if (!spec || !selectedDataset) return false;
    const next = updateCurrentDashboard((document) => {
      let tab = document.tabs.find((item) => item.id === document.activeTabId);
      if (!tab) tab = { id: crypto.randomUUID(), name: nextDashboardName(document.tabs.map((tab) => tab.name)), scrollTop: 0, placements: [] };
      const saved = document.charts.find((chart) => JSON.stringify(chart.spec) === JSON.stringify(spec));
      const chart = saved ?? { id: crypto.randomUUID(), title: chartTitle(spec), spec: { ...spec, encodings: { ...spec.encodings } } };
      const placement = clampPlacement({ ...rect, id: crypto.randomUUID(), chartId: chart.id });
      return {
        ...document,
        activeTabId: tab.id,
        charts: saved ? document.charts : [...document.charts, chart],
        tabs: document.tabs.some((item) => item.id === tab!.id)
          ? document.tabs.map((item) => item.id === tab!.id ? { ...item, placements: [...item.placements, placement] } : item)
          : [...document.tabs, { ...tab, placements: [placement] }]
      };
    });
    if (next) void loadDashboardCharts();
    return !!next;
  }

  function renameDashboardTab(id: string, name: string) {
    const clean = name.trim().slice(0, 40);
    if (!clean) return;
    updateCurrentDashboard((document) => ({ ...document,
      tabs: document.tabs.map((tab) => tab.id === id ? { ...tab, name: clean } : tab)
    }));
  }

  function renameDashboardChart(id: string, title: string) {
    const clean = title.trim().slice(0, 120);
    if (!clean) return;
    updateCurrentDashboard((document) => ({ ...document,
      charts: document.charts.map((chart) => chart.id === id ? { ...chart, title: clean } : chart)
    }));
  }

  function editDashboardChart(placementId: string, spec: ChartSpec) {
    const previous = currentDashboard;
    const next = updateCurrentDashboard((document) => updatePlacedChart(document, placementId, spec));
    if (!next || next === previous) return;
    const chartId = next.tabs.find((tab) => tab.id === next.activeTabId)?.placements.find((item) => item.id === placementId)?.chartId;
    if (!chartId) return;
    const selectionIndex = filters.findIndex((_, index) => selectionChartIdAtFilterIndex(filters, dashboardSelections, index) === chartId);
    if (selectionIndex >= 0) void removeFilter(selectionIndex).finally(() => loadDashboardCharts());
    else void loadDashboardCharts();
  }

  function moveDashboardPlacement(placement: DashboardPlacement) {
    updateCurrentDashboard((document) => updateActiveDashboardTab(document, (tab) => ({
      ...tab, placements: tab.placements.map((item) => item.id === placement.id ? clampPlacement(placement) : item)
    })));
  }

  function removeDashboardPlacement(id: string) {
    updateCurrentDashboard((document) => updateActiveDashboardTab(document, (tab) => ({
      ...tab, placements: tab.placements.filter((item) => item.id !== id)
    })));
  }

  function saveDashboardScroll(scrollTop: number) {
    updateCurrentDashboard((document) => updateActiveDashboardTab(document, (tab) => ({ ...tab, scrollTop })));
  }

  async function fetchDashboardChart(chart: DashboardDataset['charts'][number], id: number) {
    try {
      const body = { spec: chart.spec, page, page_size: pageSize, filters: filtersForChart(filters, dashboardSelections, chart.id), sorts, dedupe_columns: dedupeColumns };
      const data = queryMode === 'sql'
        ? await api.visualizeSql(activeSqlNodeId || selectedNodeId, { ...body, sql: sqlBase || activeSql })
        : await api.visualizeDataset(selectedNodeId, selectedDataset, body);
      if (id === dashboardRequestId) dashboardChartStates = { ...dashboardChartStates, [chart.id]: { data, loading: false, error: '' } };
    } catch (reason) {
      if (id === dashboardRequestId) dashboardChartStates = { ...dashboardChartStates, [chart.id]: { data: dashboardChartStates[chart.id]?.data ?? null, loading: false, error: message(reason) } };
    }
  }

  async function loadDashboardCharts() {
    const document = currentDashboard;
    if (canvasMode !== 'dashboard' || !document) return;
    const id = ++dashboardRequestId;
    dashboardChartStates = Object.fromEntries(document.charts.map((chart) => [chart.id, {
      data: dashboardChartStates[chart.id]?.data ?? null, loading: true, error: ''
    }]));
    await Promise.all(document.charts.map((chart) => fetchDashboardChart(chart, id)));
  }

  async function retryDashboardChart(chartId: string) {
    const chart = currentDashboard?.charts.find((item) => item.id === chartId);
    if (!chart) return;
    const id = dashboardRequestId;
    dashboardChartStates = { ...dashboardChartStates, [chart.id]: { data: dashboardChartStates[chart.id]?.data ?? null, loading: true, error: '' } };
    await fetchDashboardChart(chart, id);
  }

  async function applyDashboardMark(chartId: string, mark: ChartMark) {
    const chart = currentDashboard?.charts.find((item) => item.id === chartId);
    if (!chart) return;
    const before = undoPoint();
    const beforeSelections = dashboardSelections;
    const base = withoutSelections(filters, beforeSelections);
    const selected = filtersFromMark(chart.spec, mark, base.length);
    if (!selected.length) return;
    const nextSelections = [...beforeSelections.filter((item) => item.chartId !== chartId), { chartId, filters: selected }];
    dashboardSelections = nextSelections;
    const summary = selected.map(filterSummary).join(' and ');
    if (!await applyFilterChange(composeFilters(base, nextSelections), { kind: 'filter', summary: `Filter ${summary}`, details: { source: 'dashboard', chartId } }, before) && dashboardSelections === nextSelections) {
      dashboardSelections = beforeSelections;
    }
  }

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
    if (open) openQueryMenu(menu);
  }

  function clearAggregateDraft() { aggregateColumnSearch = ''; aggregateRecipe = []; focusedAggregateItemId = null; aggregateSourceSql = ''; aggregateSourceColumns = []; if (queryMenuOpen === 'aggregate') queryMenuOpen = null; }
  function clearJoinPreview() { joinPreviewRequestId++; joinPreview = null; joinPreviewLoading = false; joinPreviewError = ''; }
  function clearJoinDraft() {
    clearJoinPreview();
    joinLeftViewId = currentHistory?.id ?? '';
    joinRightViewId = '';
    joinLeftSourceId = currentHistory?.sourceId ?? '';
    joinRightSourceId = '';
    joinLeftKeys = [];
    joinRightKeys = [];
    joinLeftColumns = [...(activeVersion?.columns ?? [])];
    joinRightColumns = [];
    joinStep = 0;
    joinStepDirection = 1;
    joinSourceSide = 'right';
    if (queryMenuOpen === 'joins') queryMenuOpen = null;
  }
  function prepareJoinPicker() {
    if (joinLeftView) joinLeftSourceId = joinLeftView.sourceId ?? '';
    if (joinRightView) joinRightSourceId = joinRightView.sourceId ?? '';
    if (joinLeftViewId && !joinRightViewId && !joinLeftKeys.length && !joinRightKeys.length && !joinLeftColumns.length && !joinRightColumns.length) selectJoinView('left', joinLeftViewId);
  }
  async function resolveJoinView(side: 'left' | 'right'): Promise<boolean> {
    const viewId = side === 'left' ? joinLeftViewId : joinRightViewId;
    let view = versionHistories.find((item) => item.id === viewId && item.projectId === activeProject?.id);
    const sourceId = view?.sourceId ?? (side === 'left' ? joinLeftSourceId : joinRightSourceId);
    if (sourceId && !loadedSourceIds.includes(sourceId) && !await loadProjectSource(sourceId, '', false)) return false;
    // ponytail: source-row picks use the first View; add a second-stage View choice if multi-View sources need disambiguation.
    view = versionHistories.find((item) => item.id === viewId && item.projectId === activeProject?.id)
      ?? versionHistories.find((item) => item.projectId === activeProject?.id && item.kind === 'source' && item.sourceId === sourceId);
    if (!view) return false;
    selectJoinView(side, view.id);
    return true;
  }
  async function setJoinStep(next: JoinStep): Promise<boolean> {
    if (next === 1 && joinStep === 0) {
      joinPreviewError = '';
      if (!await resolveJoinView('left') || !await resolveJoinView('right')) {
        joinPreviewError = error || 'The selected source could not be loaded.';
        return false;
      }
    }
    joinStepDirection = next < joinStep ? -1 : 1;
    joinStep = next;
    if (next === 0) railCollapsed = false;
    return true;
  }
  function pickJoinSource(id: string) {
    clearJoinPreview();
    if (joinSourceSide === 'left') {
      joinLeftSourceId = id;
      joinLeftViewId = '';
      joinLeftColumns = [];
      joinSourceSide = 'right';
    } else {
      joinRightSourceId = id;
      joinRightViewId = '';
      joinRightColumns = [];
    }
    joinLeftKeys = [];
    joinRightKeys = [];
  }
  function pickJoinView(id: string) {
    selectJoinView(joinSourceSide, id);
    if (joinSourceSide === 'left') joinSourceSide = 'right';
  }
  function selectJoinView(side: 'left' | 'right', id: string) {
    clearJoinPreview();
    const view = versionHistories.find((item) => item.id === id && item.projectId === activeProject?.id);
    const columns = view?.versions.find((version) => version.id === view.activeVersionId)?.columns ?? [];
    if (side === 'left') { joinLeftViewId = id; joinLeftSourceId = view?.sourceId ?? ''; joinLeftColumns = [...columns]; }
    else { joinRightViewId = id; joinRightSourceId = view?.sourceId ?? ''; joinRightColumns = [...columns]; }
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
  function addAggregateColumn(column: string) {
    if (!column) return;
    const item: AggregateRecipeItem = {
      id: ++nextAggregateRecipeId,
      column,
      metrics: aggregateRecipe.some((candidate) => candidate.column === column && candidate.metrics === null) ? [] : null
    };
    aggregateRecipe = [...aggregateRecipe, item];
    if (item.metrics !== null) focusedAggregateItemId = item.id;
  }
  function removeAggregateColumn(id: number) {
    aggregateRecipe = aggregateRecipe.filter((item) => item.id !== id);
    if (focusedAggregateItemId === id) focusedAggregateItemId = aggregateRecipe.find((item) => item.metrics !== null)?.id ?? null;
  }
  function focusAggregate(id: number) {
    if (aggregateRecipe.some((item) => item.id === id && item.metrics !== null)) focusedAggregateItemId = id;
  }
  function toggleAggregateRole(id: number) {
    const item = aggregateRecipe.find((candidate) => candidate.id === id);
    if (!item) return;
    if (item.metrics === null) {
      aggregateRecipe = aggregateRecipe.map((candidate) => candidate.id === id ? { ...candidate, metrics: ['count'] } : candidate);
      focusedAggregateItemId = id;
      return;
    }
    if (aggregateRecipe.some((candidate) => candidate.id !== id && candidate.column === item.column && candidate.metrics === null)) return;
    aggregateRecipe = aggregateRecipe.map((candidate) => candidate.id === id ? { ...candidate, metrics: null } : candidate);
    if (focusedAggregateItemId === id) focusedAggregateItemId = aggregateRecipe.find((candidate) => candidate.metrics !== null)?.id ?? null;
  }
  function toggleAggregateMetric(metric: AggregateMetric, checked: boolean) {
    const item = aggregateRecipe.find((candidate) => candidate.id === focusedAggregateItemId);
    if (!item || item.metrics === null) return;
    const metrics = checked ? [...item.metrics, metric] : item.metrics.filter((candidateMetric) => candidateMetric !== metric);
    aggregateRecipe = aggregateRecipe.map((candidate) => candidate.id === item.id ? { ...candidate, metrics } : candidate);
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
    const before = undoPoint();
    const current = result;
    const targetNodeId = queryMode === 'sql' ? activeSqlNodeId || selectedNodeId : selectedNodeId;
    if (!current || !targetNodeId || mutationApplying) return false;
    if (!query) { if (mutationTarget) mutationError = 'Could not build the column query.'; else columnMutationError = 'Could not build the column query.'; return false; }
    const id = ++requestId;
    resetBackground();
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
      dashboardSelections = [];
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
      stageChange(change, before);
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
    versionsOpen = false;
    exportTrigger = trigger;
    exportOpen = true;
    exportFormat = 'csv';
    exportJsonLayout = 'rows';
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

  function toggleVersions() {
    if (versionsOpen) { versionsOpen = false; return; }
    if (!currentHistory) return;
    closeExport();
    versionsOpen = true;
  }

  function closeVersions() { versionsOpen = false; }

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

  function setExportFormat(format: ExportFormat) { exportFormat = format; if (format !== 'xlsx') exportSelectedKeys = ['current']; }
  function toggleExportOption(key: string, checked: boolean) { exportSelectedKeys = checked ? [...new Set([...exportSelectedKeys, key])] : exportSelectedKeys.filter((item) => item !== key); }

  async function runExport() {
    const current = currentExportOption;
    if (!current || exporting || exportLoading) return;
    const choices = [current, ...exportOptions];
    const selected = exportFormat === 'xlsx' ? choices.filter((option) => exportSelectedKeys.includes(option.key)) : [current];
    if (!selected.length) return;
    exporting = true;
    exportError = '';
    try {
      const download = await api.exportData({
        format: exportFormat,
        ...(exportFormat === 'json' ? { json_layout: exportJsonLayout } : {}),
        filename: exportFormat === 'xlsx' ? 'quark-export' : current.name,
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

  function undoPoint(): UndoPoint {
    return {
      historyId: currentHistory?.id ?? '',
      sqlBase, activeSql, activeSqlNodeId, sqlText,
      columnOrder: [...columnOrder], hiddenColumns: [...hiddenColumns],
      filters: filters.map((filter) => ({ ...filter, ...(Array.isArray(filter.value) ? { value: [...filter.value] } : {}) })),
      sorts: sorts.map((sort) => ({ ...sort })),
      dedupeColumns: [...dedupeColumns],
      dashboardSelections: dashboardSelections.map((selection) => ({ ...selection, filters: selection.filters.map((filter) => ({ ...filter })) })),
      join: activeJoin, page
    };
  }

  function stageChange(change: VersionChange, before?: UndoPoint) {
    if (!currentHistory) return;
    redoStack = []; versionRedo = [];
    recordingNotice = '';
    undoStack = [...undoStack, before ?? undoPoint()];
    replaceHistory(stageVersionChange(currentHistory, change), true);
  }

  function clearPending(history: ViewHistory) {
    undoStack = []; redoStack = []; versionRedo = [];
    replaceHistory({ ...history, pendingParentId: null, pendingChanges: [] });
  }

  async function restoreUndoPoint(point: UndoPoint, history: ViewHistory): Promise<boolean> {
    if (point.historyId !== currentHistory?.id) return false;
    historyBusy = true;
    try {
      // Fetch before changing history so a failed replay leaves both stacks intact.
      const next = await api.querySql(point.activeSqlNodeId || activeProject?.node_id || history.nodeId, {
        sql: point.sqlBase || point.activeSql, page: point.page, page_size: pageSize,
        filters: point.filters, sorts: point.sorts, dedupe_columns: point.dedupeColumns,
      });
      if (point.historyId !== currentHistory?.id) return false;
      if (!replaceHistory(history)) return false;
      closeSql(false); clearAggregateDraft(); resetBackground();
      filters = point.filters; sorts = point.sorts;
      dashboardSelections = point.dashboardSelections;
      dedupeColumns = [...point.dedupeColumns]; dedupeDraft = [...point.dedupeColumns];
      columnOrder = [...point.columnOrder]; hiddenColumns = [...point.hiddenColumns];
      activeJoin = point.join; sqlText = point.sqlText; sqlBase = point.sqlBase;
      activeSql = next.sql; activeSqlNodeId = point.activeSqlNodeId;
      result = next; page = next.page; pageInput = String(next.page);
      reorderOrigin = null; selectedCell = null; editingCell = null; recordingNotice = '';
      await tick(); gridApi?.scrollToAbsoluteRow((page - 1) * pageSize);
      if (canvasMode === 'dashboard') void loadDashboardCharts();
      return true;
    } catch (reason) { recordingNotice = `Could not restore change: ${message(reason)}`; return false; }
    finally { historyBusy = false; }
  }
  async function undoLastChange() {
    const history = currentHistory;
    const point = undoStack[undoStack.length - 1];
    if (!history?.pendingChanges.length || !point || loadingData || historyBusy) return;
    const redo = { point: undoPoint(), history };
    const pendingChanges = history.pendingChanges.slice(0, -1);
    if (!await restoreUndoPoint(point, { ...history, pendingChanges, pendingParentId: pendingChanges.length ? history.pendingParentId : null })) return;
    undoStack = undoStack.slice(0, -1);
    redoStack = [...redoStack, redo];
  }
  async function undoCommand() {
    if (currentHistory?.pendingChanges.length) { await undoLastChange(); return; }
    const history = currentHistory;
    const version = history?.versions.find((item) => item.id === activeVersion?.parentId);
    if (!history || !version || !activeVersion) return;
    const previous = { historyId: history.id, versionId: activeVersion.id };
    await restoreVersion(version, true);
    if (currentHistory?.activeVersionId === version.id) versionRedo = [...versionRedo, previous];
  }
  async function redoCommand() {
    if (versionRedo.length) {
      const target = versionRedo[versionRedo.length - 1];
      const version = target?.historyId === currentHistory?.id ? currentHistory?.versions.find((item) => item.id === target.versionId) : undefined;
      if (!version || currentHistory?.pendingChanges.length) return;
      await restoreVersion(version, true);
      if (currentHistory?.activeVersionId === version.id) versionRedo = versionRedo.slice(0, -1);
      return;
    }
    const entry = redoStack[redoStack.length - 1];
    if (entry && entry.point.historyId === currentHistory?.id) {
      const before = undoPoint();
      if (!await restoreUndoPoint(entry.point, entry.history)) return;
      undoStack = [...undoStack, before]; redoStack = redoStack.slice(0, -1);
      return;
    }
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
      dashboardSelections = [];
      dashboardChartStates = {};
      joinLeftViewId = history.id;
      joinRightViewId = '';
      joinLeftSourceId = '';
      joinRightSourceId = '';
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
        editorTheme,
        editorHighlight,
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
  function resetSql(dataset: BaseViewInfo | undefined) { closeSql(); queryMode = 'builder'; sqlText = dataset?.sql ?? ''; sqlBase = ''; activeSql = ''; activeSqlNodeId = ''; sqlError = ''; }

  function discardPending(): boolean {
    const history = currentHistory;
    if (!history?.pendingChanges.length) return true;
    if (!window.confirm(`Discard ${history.pendingChanges.length} pending change${history.pendingChanges.length === 1 ? '' : 's'}?`)) return false;
    clearPending(history);
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
    dashboardSelections = [];
    dashboardChartStates = {};
    filters = [];
    sorts = [];
    dedupeColumns = [];
    dedupeDraft = [];
    page = 1;
    pageInput = '1';
    return replayStored(version.sql, version.nodeId, version.join, version.columns, version.hiddenColumns);
  }

  async function restoreVersion(version: Version, preserveRedo = false) {
    if (!discardPending()) return;
    const before = undoPoint();
    if (!await replayVersionSnapshot(version)) {
      filters = before.filters; sorts = before.sorts; dedupeColumns = before.dedupeColumns;
      dashboardSelections = before.dashboardSelections;
      dedupeDraft = [...before.dedupeColumns]; page = before.page; pageInput = String(page);
      recordingNotice = sqlError || 'Could not open this Version.';
      return;
    }
    if (!preserveRedo) { redoStack = []; versionRedo = []; }
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
    undoStack = [];
    redoStack = []; versionRedo = [];
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
    closeCommands(false); clearCommandSequence(); resetCellSearch(); cellFinderOpen = false; settingsOpen = false; columnTarget = ""; redoStack = []; versionRedo = [];
    replayRequestId++;
    requestId++;
    resetBackground();
    sourceRequestId++;
    closeInspector();
    resetSql(undefined);
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
    joinLeftSourceId = '';
    joinRightSourceId = '';
    joinLeftKeys = [];
    joinRightKeys = [];
    joinLeftColumns = [];
    joinRightColumns = [];
    hiddenColumns = [];
    columnOrder = [];
    activeJoin = undefined;
    lastHiddenColumn = null;
    shownColumnTypes = [];
    resetVisualizeDraft();
    error = '';
    recordingNotice = '';
    queryMenuOpen = null;
    fitColumnsToContent = false;
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
    if (id === selectedDataset && result) { railOpen = false; return; }
    if (!discardPending()) return;
    replayRequestId++;
    closeInspector();
    resetSql(datasets.find((view) => view.id === id));
    selectedNodeId = activeProject?.node_id ?? history.nodeId;
    selectedDataset = id;
    columnTarget = ""; resetCellSearch(); redoStack = []; versionRedo = [];
    filters = [];
    sorts = [];
    dedupeColumns = [];
    dedupeDraft = [];
    clearAggregateDraft();
    clearJoinPreview();
    joinLeftViewId = id;
    joinRightViewId = '';
    joinLeftSourceId = history.sourceId ?? '';
    joinRightSourceId = '';
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
    resetVisualizeDraft();
    const version = history.versions.find((item) => item.id === history.activeVersionId) ?? history.versions[history.versions.length - 1];
    if (version) await replayVersionSnapshot(version);
  }

  async function loadData(): Promise<boolean> {
    const sql = sqlBase || activeSql || activeVersion?.sql;
    if (!sql) return false;
    const loaded = await runSql(sql, true, true, activeProject?.node_id ?? currentHistory?.nodeId);

    return loaded;
  }

  async function createAggregateView() {
    if (blockViewExecutionWhileRecording()) return;
    const source = queryMode === 'builder' ? result?.sql : aggregateSourceSql || sqlBase || activeSql;
    const columns = queryMode === 'builder' ? result?.columns ?? [] : aggregateSourceColumns.length ? aggregateSourceColumns : result?.columns ?? [];
    const aggregates = aggregateFields.map((item) => ({ column: item.column, metrics: item.metrics ?? [] }));
    if (!source || !canCreateAggregate) return;
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
    stageChange({ kind: 'reorder', summary: 'Reorder columns', details: { before, after } }, { ...undoPoint(), columnOrder: before });
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
      dashboardSelections = [];
      filters = [];
      sorts = [];
      dedupeColumns = [];
      dedupeDraft = [];
      if (!keepAggregateBuilder) clearAggregateDraft();
    }
    const id = ++requestId;
    resetBackground();
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
      await tick();
      gridApi?.scrollToAbsoluteRow((next.page - 1) * next.page_size);
      if (sqlOpen) createSqlEditor();
      if (canvasMode === 'chart') void loadVisualize();
      if (canvasMode === 'dashboard') void loadDashboardCharts();
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

  async function applyFilterChange(next: FilterCondition[], change: VersionChange, before = undoPoint()): Promise<boolean> {
    const previousRequestId = requestId;
    filters = next;
    page = 1;
    pageInput = '1';
    if (await loadData()) { stageChange(change, before); return true; }
    if (requestId > previousRequestId + 1) return false;
    filters = before.filters; sorts = before.sorts; dedupeColumns = before.dedupeColumns; page = before.page; pageInput = String(page);
    return false;
  }

  async function addCategoryFilter() {
    if (!filterColumn || selectedCategories.length === 0) return;
    const base = withoutSelections(filters, dashboardSelections);
    const values = [...selectedCategories];
    const filter: FilterCondition = { column: filterColumn.name, operator: 'in', value: values, ...(base.length ? { connector: 'and' as const } : {}) };
    closeInspector();
    await applyFilterChange(composeFilters([...base, filter], dashboardSelections), { kind: 'filter', summary: `Filter ${filterSummary(filter)}`, details: { column: filter.column, operator: filter.operator, value: values } });
  }

  async function addFilter() {
    if (!filterColumn) return;
    const base = withoutSelections(filters, dashboardSelections);
    const noValue = filterOperator === 'is_null' || filterOperator === 'not_null';
    if (!noValue && filterValue === '') return;
    const numericValue = filterColumn.numeric ? normalizedNumber(filterValue) : filterValue;
    if (!noValue && numericValue === null) return;
    if (filterColumn.numeric && numericValue !== null) filterValue = formattedNumber(numericValue);
    const value = noValue ? undefined : filterColumn.numeric ? numericValue! : isBooleanType(filterColumn.type) ? filterValue === 'true' : filterValue;
    const filter: FilterCondition = { column: filterColumn.name, operator: filterOperator, ...(value === undefined ? {} : { value }), ...(base.length ? { connector: 'and' as const } : {}) };
    closeInspector();
    await applyFilterChange(composeFilters([...base, filter], dashboardSelections), { kind: 'filter', summary: `Filter ${filterSummary(filter)}`, details: { column: filter.column, operator: filter.operator, ...(value === undefined ? {} : { value }) } });
  }

  function addNullFilter(operator: 'is_null' | 'not_null') { filterOperator = operator; void addFilter(); }

  async function toggleFilterConnector(index: number) {
    if (selectionChartIdAtFilterIndex(filters, dashboardSelections, index)) return;
    const base = withoutSelections(filters, dashboardSelections);
    const filter = base[index];
    if (!filter || index === 0) return;
    const connector = filter.connector === 'or' ? 'and' : 'or';
    await applyFilterChange(composeFilters(base.map((item, itemIndex) => itemIndex === index ? { ...item, connector } : item), dashboardSelections), { kind: 'filter-connector', summary: `Use ${connector.toUpperCase()} before ${filter.column}`, details: { index, column: filter.column, connector } });
  }

  async function removeFilter(index: number) {
    const filter = filters[index];
    if (!filter) return;
    const chartId = selectionChartIdAtFilterIndex(filters, dashboardSelections, index);
    if (chartId) {
      const before = undoPoint();
      const beforeSelections = dashboardSelections;
      const nextSelections = dashboardSelections.filter((selection) => selection.chartId !== chartId);
      dashboardSelections = nextSelections;
      if (!await applyFilterChange(composeFilters(withoutSelections(filters, beforeSelections), nextSelections), { kind: 'filter-remove', summary: `Clear ${currentDashboard?.charts.find((chart) => chart.id === chartId)?.title ?? 'chart'} selection`, details: { source: 'dashboard', chartId } }, before) && dashboardSelections === nextSelections) {
        dashboardSelections = beforeSelections;
      }
      return;
    }
    const base = withoutSelections(filters, dashboardSelections);
    const value = Array.isArray(filter.value) ? [...filter.value] : filter.value;
    await applyFilterChange(composeFilters(base.filter((_, itemIndex) => itemIndex !== index), dashboardSelections), { kind: 'filter-remove', summary: `Remove filter ${filterSummary(filter)}`, details: { column: filter.column, operator: filter.operator, ...(value === undefined ? {} : { value }) } });
  }
  async function setColumnSort(column: ColumnInfo, direction: 'asc' | 'desc' | 'none') {
    const before = undoPoint();
    const exists = sorts.some((sort) => sort.column === column.name);
    sorts = direction === 'none' ? sorts.filter((sort) => sort.column !== column.name) : exists
      ? sorts.map((sort) => sort.column === column.name ? { ...sort, direction } : sort)
      : [...sorts, { column: column.name, direction }];
    page = 1; pageInput = '1';
    if (await loadData()) stageChange({ kind: 'sort', summary: direction === 'none' ? `Remove sort ${column.name}` : `Sort ${column.name} ${direction}`, details: { column: column.name, direction } }, before);
    else { sorts = before.sorts; page = before.page; pageInput = String(page); }
  }
  async function cycleSort(column: ColumnInfo) {
    const existing = sorts.find((sort) => sort.column === column.name);
    await setColumnSort(column, existing?.direction === 'asc' ? 'desc' : existing ? 'none' : 'asc');
  }
  async function removeSort(column: string) { const item = result?.columns.find((item) => item.name === column); if (item) await setColumnSort(item, 'none'); }
  async function clearQuery() {
    const before = undoPoint();
    const count = filters.length;
    const beforeSelections = dashboardSelections;
    dashboardSelections = [];
    sorts = [];
    dedupeColumns = [];
    dedupeDraft = [];
    if (!await applyFilterChange([], { kind: 'filter-clear', summary: 'Clear conditions', details: { count } }, before) && !dashboardSelections.length) dashboardSelections = beforeSelections;
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
  async function setDedupe(columns: string[]) {
    const before = undoPoint();
    dedupeColumns = [...columns]; dedupeDraft = [...columns]; page = 1; pageInput = '1';
    if (await loadData()) stageChange({ kind: 'dedupe', summary: columns.length ? `Dedupe by ${columns.join(', ')}` : 'Clear dedupe', details: { columns } }, before);
    else { dedupeColumns = before.dedupeColumns; dedupeDraft = [...before.dedupeColumns]; page = before.page; pageInput = String(page); }
  }
  async function applyDedupe() { await setDedupe(dedupeDraft); }
  async function clearDedupe() { await setDedupe([]); }
  function isColumnProtected(column: string): boolean { return [...dedupeColumns, ...dedupeDraft].includes(column); }
  function setHidden(next: string[], change: VersionChange): boolean {
    const columns = new Set((result?.columns ?? []).map((column) => column.name));
    const normalized = [...new Set(next)].filter((column) => columns.has(column));
    if (normalized.length === hiddenColumns.length && normalized.every((column, index) => column === hiddenColumns[index])) return false;
    const before = undoPoint();
    hiddenColumns = normalized;
    stageChange(change, before);
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
  function hideAllColumns() {
    const columns = visibleColumns.filter((column) => !isColumnProtected(column.name)).map((column) => column.name);
    if (columns.length && setHidden([...hiddenColumns, ...columns], { kind: 'hide', summary: 'Hide all columns', details: { columns } })) lastHiddenColumn = columns[columns.length - 1];
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
  async function changePage(next: number) {
    if (next < 1 || next > totalPages || next === result?.page) return;
    await seekRow((next - 1) * Math.max(1, result?.page_size ?? pageSize));
  }
  async function seekRow(absoluteRow: number) {
    if (loadingData || !result) return;
    cancelSnapshot();
    const total = safeTotalRows(result.total_rows);
    if (total <= 0) return;
    const clamped = clampAbsoluteRow(absoluteRow, total);
    gridApi?.scrollToAbsoluteRow(clamped);
    const visible = cellPageSize();
    handleViewport({ firstRow: clamped, lastRow: Math.min(total - 1, clamped + visible), velocity: 0 });
    await consumePendingSelect();
  }
  async function previewSeek(absoluteRow: number, column: string) {
    if (!result || loadingData) return;
    pendingSelect = { absRow: Math.floor(absoluteRow), column };
    await seekRow(absoluteRow);
  }
  async function consumePendingSelect() {
    const pending = pendingSelect;
    if (!pending || !result) return;
    const start = (result.page - 1) * Math.max(1, result.page_size);
    const index = pending.absRow - start;
    if (index < 0 || index >= result.rows.length) return;
    const name = visibleColumns.some((column) => column.name === pending.column) ? pending.column : visibleColumns[0]?.name;
    pendingSelect = null;
    if (!name) return;
    selectedCell = { row: index, column: name, expanded: false };
    await focusCell(index, name);
  }
  function promotePage(cache: CachedPage) {
    if (!result || cache.page === result.page) return;
    const left = { page: result.page, rows: result.rows };
    result = { ...result, rows: cache.rows, page: cache.page };
    neighborCache = left;
    aheadCache = aheadCache.filter((entry) => entry.page !== cache.page);
    selectedCell = null;
    editingCell = null;
    page = cache.page;
    pageInput = String(cache.page);
    void consumePendingSelect();
  }
  function prefetchContext() {
    const targetNodeId = activeSqlNodeId || selectedNodeId;
    const sql = sqlBase || activeSql || activeVersion?.sql;
    if (!targetNodeId || !sql || !result) return null;
    return { targetNodeId, sql, filters, sorts, dedupe_columns: dedupeColumns, key: currentQueryKey(), generation: requestId };
  }
  async function fetchPageBackground(page: number) {
    const context = prefetchContext();
    if (!context || !result || pendingPages.has(page) || pendingPages.size >= 2) return;
    const failedAt = fetchFailures.get(page);
    if (failedAt !== undefined && performance.now() - failedAt.at < FETCH_FAIL_COOLDOWN_MS) return;
    const controller = new AbortController();
    pendingPages.set(page, controller);
    const started = performance.now();
    try {
      const next = await api.querySql(context.targetNodeId, { sql: context.sql, page, page_size: result.page_size, filters: context.filters, sorts: context.sorts, dedupe_columns: context.dedupe_columns }, controller.signal);
      if (controller.signal.aborted || context.generation !== requestId || context.key !== currentQueryKey() || !result) return;
      latencyMs = latencyEma(latencyMs, performance.now() - started);
      aheadCache = [...aheadCache.filter((entry) => entry.page !== next.page), { page: next.page, rows: next.rows }].slice(-2);
      fetchFailures.delete(page);
    } catch (reason) {
      if (!controller.signal.aborted && context.generation === requestId && context.key === currentQueryKey()) {
        fetchFailures.set(page, { at: performance.now(), message: message(reason) });
      }
    } finally {
      if (pendingPages.get(page) === controller) pendingPages.delete(page);
      if (!controller.signal.aborted && context.generation === requestId && latestViewport) handleViewport(latestViewport);
    }
  }
  async function fetchSnapshot(absoluteRow: number) {
    cancelSnapshot();
    const context = prefetchContext();
    if (!context || !result || !dragHeld) return;
    const total = safeTotalRows(result.total_rows);
    const clamped = clampAbsoluteRow(absoluteRow, total);
    const visible = cellPageSize() + 8;
    const end = Math.min(total - 1, clamped + visible);
    const window = snapshotWindow(clamped, visible);
    const size = Math.max(1, result.page_size);
    const covers = (start: number, length: number) => clamped >= start && end < start + length;
    if (covers((result.page - 1) * size, result.rows.length)) return;
    if (snapshotCache && covers(snapshotCache.start, snapshotCache.rows.length)) return;
    const controller = new AbortController();
    snapshotRequest = controller;
    try {
      // A viewport straddling the preview grid needs both small slices.
      const pages = pagesForRange(clamped, end, window.size, Math.ceil(total / window.size));
      const chunks = await Promise.all(pages.map((page) => api.querySql(context.targetNodeId, { sql: context.sql, page, page_size: window.size, filters: context.filters, sorts: context.sorts, dedupe_columns: context.dedupe_columns }, controller.signal)));
      if (controller.signal.aborted || !dragHeld || context.generation !== requestId || context.key !== currentQueryKey()) return;
      snapshotCache = { start: (pages[0] - 1) * window.size, rows: concatRows(chunks.map((chunk) => chunk.rows)) };
    } catch { /* Retain visible rows; releasing retries with a full page. */ }
    finally { if (snapshotRequest === controller) snapshotRequest = null; }
  }
  function thumbHeld(held: boolean) {
    dragHeld = held;
    cancelSnapshot();
    if (held) {
      for (const request of pendingPages.values()) request.abort();
      pendingPages.clear();
    }
  }
  function snapshotRest(absoluteRow: number) {
    if (!result || loadingData) return;
    void fetchSnapshot(absoluteRow);
  }
  function retryGridLoad() {
    fetchFailures.clear();
    gridLoadError = '';
    if (latestViewport) handleViewport(latestViewport);
  }
  function handleViewport(report: ViewportReport) {
    latestViewport = report;
    if (!result || loadingData || cellEditSaving || currentQueryKey() !== renderedQueryKey) return;
    if (dragHeld) return;
    const total = safeTotalRows(result.total_rows);
    if (total <= 0) return;
    const size = Math.max(1, result.page_size);
    if (report.velocity !== 0) lastDir = report.velocity > 0 ? 1 : -1;
    const target = absoluteRowToPage(clampAbsoluteRow(report.firstRow, total), size, Math.max(totalPages, 1)).page;
    gridLoadError = pagesForRange(report.firstRow, report.lastRow, size, totalPages).map((page) => fetchFailures.get(page)?.message).find(Boolean) ?? '';
    const cached = [neighborCache, ...aheadCache].find((entry) => entry?.page === target);
    if (cached) promotePage(cached);
    const wanted = criticalPages({ ...report, pageSize: size, totalPages: Math.max(totalPages, 1), currentPage: result.page, coveredPages: [result.page], latencyMs, lastDir });
    aheadCache = aheadCache.filter((entry) => wanted.includes(entry.page));
    for (const [page, controller] of pendingPages) {
      if (!wanted.includes(page)) { controller.abort(); pendingPages.delete(page); }
    }
    const covered = [result.page, ...(neighborCache ? [neighborCache.page] : []), ...aheadCache.map((entry) => entry.page)];
    for (const page of wanted) if (!covered.includes(page)) void fetchPageBackground(page);
    if (snapshotCache) {
      const loaded = [result, neighborCache, ...aheadCache].filter((entry) => entry !== null);
      const visiblePages = pagesForRange(report.firstRow, Math.min(total - 1, report.lastRow), size, totalPages);
      if (visiblePages.every((page) => loaded.some((entry) => entry.page === page))) snapshotCache = null;
    }
  }
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
    tableScroll.scrollTo({ left: Math.max(0, header.offsetLeft - (tableScroll.clientWidth - header.offsetWidth) / 2), behavior: matchMedia('(prefers-reduced-motion: reduce)').matches ? 'instant' : 'smooth' });
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
    const base = withoutSelections(filters, dashboardSelections);
    const filter: FilterCondition = { column: column.name, operator: '=', value, ...(base.length ? { connector: 'and' as const } : {}) };
    await applyFilterChange(composeFilters([...base, filter], dashboardSelections), { kind: 'filter', summary: `Filter ${filterSummary(filter)}`, details: { column: filter.column, operator: filter.operator, value } });
  }
  function cellTitle(column: ColumnInfo, value: unknown): string { const text = display(value); return column.profile_kind === 'categorical' && (typeof value === 'string' || typeof value === 'boolean') ? `${text} — Double-click to filter by this value` : text; }
  function cellEditText(value: unknown): string { return value == null ? '' : display(value); }
  function startCellEdit(row: number, column: string, initial?: string) {
    if (!result || loadingData || cellEditSaving) return;
    const original = cellEditText(result.rows.cell(row, column));
    selectedCell = { row, column, expanded: false };
    editingCell = { row, column, value: initial ?? original, original };
    cellEditError = '';
  }
  async function focusCell(row: number, column: string) {
    if (result) gridApi?.scrollToAbsoluteRow((result.page - 1) * result.page_size + row, true);
    await tick();
    const cell = [...(tableScroll?.querySelectorAll<HTMLTableCellElement>('td[data-row][data-column]') ?? [])].find(
      (element) => Number(element.dataset.row) === row && element.dataset.column === column,
    );
    cell?.focus({ preventScroll: true });
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
    const before = undoPoint();
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
    resetBackground();
    cellEditSaving = true;
    cellEditError = '';
    try {
      const next = await api.querySql(targetNodeId, { sql: query, page: current.page, page_size: current.page_size, filters: [], sorts: [], dedupe_columns: [] });
      if (id !== requestId) return;
      closeSql(false);
      clearAggregateDraft();
      dashboardSelections = [];
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
      stageChange({ kind: 'cell', summary: `Edit row ${rowNumber}, ${edit.column}`, details: { row: rowNumber, column: edit.column } }, before);
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
  // Pinning parks the column at the left of the grid, ahead of every unpinned one, and freezes
  // it there; the reorder rides the same staged path as a drag so it undoes like one.
  function togglePinColumn(column: ColumnInfo) {
    if (livePins.includes(column.name)) { pinnedColumns = livePins.filter((name) => name !== column.name); return; }
    const target = visibleColumns.filter((item) => !livePins.includes(item.name))[0];
    if (target && target.name !== column.name) {
      beginColumnReorder();
      previewColumnReorder(column.name, target.name, 'before');
      commitColumnReorder();
    }
    pinnedColumns = [...livePins, column.name];
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
  function typeToggleDisabled(type: string): boolean { return isTypeShown(type) && (columnTypes.length === 1 || (shownColumnTypes.length === 1 && shownColumnTypes[0] === type)); }
  function setCategorySearchLive(value: string) { categorySearch = value; loadCategoryValues(true); }
</script>

{#snippet operationMenus()}
                {#if toolbarVisibility !== 'hide' || queryMenuOpen === 'columns'}
                  <ColumnsMenuPopover embedded={toolbarVisibility === 'hide'}
                    open={queryMenuOpen === 'columns'} ontoggle={(event) => syncQueryMenu('columns', event)}
                    visibleCount={visibleColumns.length} totalCount={result?.columns.length ?? 0}
                    {columnMenuSearch} setColumnMenuSearch={(value) => columnMenuSearch = value}
                    {columnMenuRegex} setColumnMenuRegex={(value) => columnMenuRegex = value}
                    columnMenuRegexError={columnMenuRegex ? columnMenuRegexResult.error : ''}
                    {columnTypes} {columnTypeCounts} {isTypeShown} {toggleShownType} {typeToggleDisabled}
                    {nullThreshold} setNullThreshold={(value) => nullThreshold = value}
                    onApplyThreshold={() => hideColumnsAtNullFraction(Math.min(100, Math.max(0, nullThreshold)) / 100)}
                    onHideAll={hideAllColumns}
                    onShowAll={showAllColumns} hiddenCount={hiddenColumns.length}
                    {columnMenuItems} {hiddenColumns} {isColumnProtected}
                    visibleColumnsLength={visibleColumns.length} onToggleColumn={toggleColumn}
                    orderedColumnNames={orderedColumns.map((column) => column.name)}
                    onBeginReorder={beginColumnReorder} onPreviewReorder={previewColumnReorder}
                    onCommitReorder={commitColumnReorder} onCancelReorder={cancelColumnReorder}
                    onMoveColumn={moveColumnOneStep} onRegexVisibility={applyRegexVisibility}
                  />
                {/if}
                {#if toolbarVisibility !== 'hide' || queryMenuOpen === 'joins'}
                  <JoinMenuPopover embedded={toolbarVisibility === 'hide'}
                    open={queryMenuOpen === 'joins'} ontoggle={(event) => syncQueryMenu('joins', event)}
                    step={joinStep} direction={joinStepDirection} sourceSide={joinSourceSide}
                    onSetSourceSide={(side) => joinSourceSide = side} onStep={setJoinStep}
                    sources={nodes} views={projectViews} {joinLeftViewId} {joinRightViewId} {joinLeftSourceId} {joinRightSourceId}
                    {joinLeftKeys} {joinRightKeys} onSetKeys={setJoinKeys}
                    {joinLeftColumns} {joinRightColumns} onToggleColumn={toggleJoinColumn}
                    onSelectAll={(side) => selectJoinColumns(side, [...(side === 'left' ? joinLeftVersion?.columns ?? [] : joinRightVersion?.columns ?? [])])}
                    onSelectNone={(side) => selectJoinColumns(side, [])}
                    {joinPreview} previewLoading={joinPreviewLoading} previewError={joinPreviewError}
                    onCheck={checkJoin} {count}
                    onRun={async () => { await runJoin(); if (commandMode === 'operation' && queryMenuOpen !== 'joins') closeCommands(false); }} canRun={canRunJoin} running={loadingData} preparing={!!loadingSourceId}
                  />
                {/if}
                {#if toolbarVisibility !== 'hide' || queryMenuOpen === 'aggregate'}
                  <AggregateMenuPopover embedded={toolbarVisibility === 'hide'}
                    open={queryMenuOpen === 'aggregate'} ontoggle={(event) => syncQueryMenu('aggregate', event)}
                    label="Aggregate"
                    {aggregateColumnSearch} setAggregateColumnSearch={(value) => aggregateColumnSearch = value}
                    {aggregateColumnMatches} {aggregateRecipe} {focusedAggregateItemId}
                    onAddColumn={addAggregateColumn} onRemoveColumn={removeAggregateColumn}
                    onFocusAggregate={focusAggregate} onToggleRole={toggleAggregateRole}
                    {selectedAggregateColumn} availableMetrics={availableAggregateMetrics}
                    {aggregateMetrics} onToggleMetric={toggleAggregateMetric}
                    {canCreateAggregate}
                    onCreateView={async () => { await createAggregateView(); if (commandMode === 'operation' && queryMenuOpen !== 'aggregate') closeCommands(false); }} creating={loadingData}
                  />
                {/if}
                {#if toolbarVisibility !== 'hide' || queryMenuOpen === 'dedupe'}
                  <DedupeMenuPopover embedded={toolbarVisibility === 'hide'}
                    open={queryMenuOpen === 'dedupe'} ontoggle={(event) => syncQueryMenu('dedupe', event)}
                    label={`Dedupe${dedupeDraft.length ? ` (${dedupeDraft.length})` : ''}`}
                    columns={visibleColumns} {dedupeDraft} onToggle={toggleDedupe}
                    onApply={applyDedupe} onClear={clearDedupe} dedupeAppliedCount={dedupeColumns.length}
                  />
                {/if}
{/snippet}

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
      inert={!!inspectorMode}
      onSettings={() => executeCommand('settings')}
      onCommands={() => { settingsOpen = false; showCommands('wheel'); }}
      canCommand={!!result && !loadingData}
      onProjects={exitProject}
      onToggleRailCollapsed={() => railCollapsed = !railCollapsed}
      onOpenRail={() => railOpen = true}
    />
  {/snippet}

  {#snippet rail()}
    <SourceRail
      {nodes} views={projectViews} selectedViewId={selectedDataset} {selectedSourceId} {loadedSourceIds} {loadingSourceId} {loadingNodes} {railOpen} collapsed={railCollapsed} {sourceOpen} {highlightToken}
      numbered={sourcePicking}
      inert={!!inspectorMode}
      joinPicking={queryMenuOpen === 'joins' && joinStep === 0} {joinSourceSide} {joinLeftViewId} {joinRightViewId}
      {joinLeftSourceId} {joinRightSourceId}
      onSelectSource={(id) => { void loadProjectSource(id); }}
      onSelectView={(id) => { void selectView(id); }}
      onPickJoinSource={pickJoinSource}
      onPickJoinView={pickJoinView}
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
      {#if settingsOpen}<SettingsPage visibility={toolbarVisibility} {actionMenuMode} {chartTheme} {themePreference} {colorScheme} onThemePreference={setThemePreference} chartPalette={chartPalette} onActionMenuMode={setActionMenuMode} error={settingsError} onVisibility={setToolbarVisibility} onChartTheme={setChartTheme} onChartPalette={setChartPalette} onClose={() => { settingsOpen = false; void tick().then(() => tableScroll?.focus()); }} />{/if}
      <div class="workspace-content" hidden={settingsOpen}>
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
        <section class="workspace" inert={cellEditSaving || historyBusy}>
          <DatasetHead
            title={currentHistory?.name ?? ''}
            {versionLabel} {canPreviousVersion} {canNextVersion}
            onPreviousVersion={previousVersion} onNextVersion={nextVersion}
            versionOpen={versionsOpen} onToggleVersions={toggleVersions}
            showMeta={!!result}
            rows={result ? count(result.total_rows) : ''}
            ms={result ? compact(result.elapsed_ms) : ''}
            showRefresh
            onRefresh={loadActiveData}
            onExport={openExport}
            {loadingData} canExport={!!result} {exporting}
            pendingCount={currentHistory?.pendingChanges.length ?? 0}
            onStopRecording={stopRecording}
            canUndo={undoStack.length > 0 && undoStack[undoStack.length - 1].historyId === currentHistory?.id}
            onUndo={() => void undoLastChange()}
            {exportOpen}
            inert={!!inspectorMode}
            {canvasMode} onCanvasMode={setCanvasMode} canChart={!!result}
          >
            {#snippet versionMenu()}
              <VersionMenu
                open={versionsOpen} history={currentHistory} {storageError}
                onRestore={restoreVersion}
                onDiff={(version, trigger) => currentHistory && showDiff(currentHistory, version, trigger)}
                onClose={closeVersions}
              />
            {/snippet}
            {#snippet exportMenu()}
              <ExportMenu
                open={exportOpen} current={currentExportOption} options={exportOptions}
                selectedKeys={exportSelectedKeys} loading={exportLoading} {exporting} error={exportError}
                setFormat={setExportFormat} setJsonLayout={(layout) => exportJsonLayout = layout}
                onToggle={toggleExportOption}
                onExport={runExport} onClose={closeExport}
              />
            {/snippet}
          </DatasetHead>
          {#if recordingNotice}<div class="banner" role="status">{recordingNotice}</div>{/if}
            {#if error || gridLoadError}
              <div class="banner error-banner" role="alert"><div><strong>Request failed</strong><p>{error || gridLoadError}</p></div><button onclick={() => gridLoadError ? retryGridLoad() : loadData()}>Retry</button></div>
            {/if}
            {#if selectedDataset}
              <QueryConditionBar
                visibility={toolbarVisibility} menuOpen={queryMenuOpen !== null} bind:densityOpen={densityMenuOpen}
                inert={!!inspectorMode || loadingData}
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
                {rowDensity} setRowDensity={(density) => rowDensity = density}
                {fitColumnsToContent} onToggleFitColumns={() => fitColumnsToContent = !fitColumnsToContent}
              >
                {#snippet menus()}{@render operationMenus()}{/snippet}
              </QueryConditionBar>
              {#if cellFinderOpen}
                <CellFinder term={cellSearchTerm} busy={cellSearching} notice={cellSearchNotice} onTerm={(term) => { cellSearchTerm = term; resetCellSearch(); }} onFind={findCellValue} onClose={() => { cellFinderOpen = false; resetCellSearch(); tableScroll?.focus(); }} />
              {/if}
              {#if sqlOpen}
                <SqlEditorPanel
                  setEditorHost={(el) => editorHost = el}
                  hasError={!!sqlError} {sqlError}
                  onClose={() => closeSql()}
                  onRun={runSqlAndAddView}
                  canRun={!loadingData && !!sqlText.trim()} running={loadingData}
                />
              {/if}
              <div class="data-stage">
                {#if canvasMode === 'chart'}
                  <VisualizeStage
                    columnSearch={visualizeSearch} setColumnSearch={(value) => visualizeSearch = value}
                    columns={visualizeFieldOptions} selected={visualizeColumns}
                    onToggleColumn={toggleVisualizeColumn}
                    suggestions={visualizeSuggestions} spec={visualizeSpec} onSelectChart={selectVisualizeChart} onSelectMetric={selectVisualizeMetric} onSelectLayout={selectVisualizeLayout}
                    roles={visualizeSpec ? chartRoles(visualizeSpec.chart) : []}
                    roleOf={visualizeRoleOf} onSetRole={setVisualizeRole}
                    data={visualizeData} loading={visualizeLoading} error={visualizeError}
                    {count} {compact} chartTheme={chartThemeKey} {binLabel}
                    onAddToDashboard={addChartToDashboard}
                    onMark={(mark) => void applyChartMark(mark)}
                  />
                {:else if canvasMode === 'dashboard'}
                  <DashboardStage
                    tabs={currentDashboard?.tabs ?? []}
                    activeTabId={currentDashboard?.activeTabId ?? ''}
                    charts={currentDashboard?.charts ?? []}
                    chartStates={dashboardChartStates}
                    {count} {compact} chartTheme={chartThemeKey} {binLabel}
                    columnSearch={visualizeSearch} setColumnSearch={(value) => visualizeSearch = value}
                    columns={visualizeFieldOptions} selected={visualizeColumns}
                    onToggleColumn={toggleVisualizeColumn}
                    suggestions={visualizeSuggestions} spec={visualizeSpec} onSelectChart={selectVisualizeChart} onSelectMetric={selectVisualizeMetric} onSelectLayout={selectVisualizeLayout}
                    roles={visualizeSpec ? chartRoles(visualizeSpec.chart) : []}
                    roleOf={visualizeRoleOf} onSetRole={setVisualizeRole}
                    onSelectTab={selectDashboardTab}
                    onCreateTab={createDashboardTab}
                    onRenameTab={renameDashboardTab}
                    onRenameChart={renameDashboardChart}
                    onEditChart={editDashboardChart}
                    onPlaceCurrent={placeCurrentChart}
                    onMove={moveDashboardPlacement}
                    onRemove={removeDashboardPlacement}
                    onMark={(chartId, mark) => void applyDashboardMark(chartId, mark)}
                    onRetryChart={(chartId) => void retryDashboardChart(chartId)}
                    onScroll={saveDashboardScroll}
                  />
                {:else}
                <section class="table-pane {rowDensity}" aria-label="View rows" inert={!!inspectorMode}>
                  <div class="table-card" class:recording={!!currentHistory?.pendingChanges.length} aria-busy={loadingData}>
                    {#if loadingData && !result}
                      <div class="table-state"><span class="spinner"></span>Loading rows…</div>
                    {:else if result && result.rows.length === 0}
                      <div class="table-state"><strong>No matching rows</strong><span>{queryMode === 'sql' ? 'The SQL query returned no rows.' : 'Change or remove filters to see more data.'}</span></div>
                    {:else if result}
                      <DataGridTable
                        bind:this={gridApi}
                        columns={visibleColumns} bodyColumns={rowColumns} rows={result.rows}
                        rowOffset={(result.page - 1) * result.page_size}
                        pageSize={result.page_size}
                        neighbor={neighborCache} ahead={aheadCache} snapshot={snapshotCache}
                        onSnapshotRest={snapshotRest} onThumbHeld={thumbHeld} onThumbMove={cancelSnapshot} onViewportNeed={handleViewport}
                        onPreviewSelect={previewSeek}
                        pinnedColumns={livePins} onTogglePin={togglePinColumn}
                        caption={`Rows from ${currentHistory?.name ?? selectedDataset}`}
                        {rowDensity} {fitColumnsToContent}
                        {canQuery} canInsert={!loadingData} canEdit={!loadingData} {sorts} {filters} {columnLabelParts} {isColumnProtected}
                        onSort={cycleSort}
                        onFilter={(column, trigger) => openFilter(column, trigger)}
                        onProfile={(column, trigger) => openStats(column, trigger)}
                        onHide={hideColumn} {display} {cellTitle}
                        selectedColumn={commandColumn?.name ?? ''}
                        onSelectColumn={(name) => { columnTarget = name; selectedCell = null; }}
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
                        totalRows={safeTotalRows(result.total_rows)} totalLabel={count(result.total_rows)}
                        seekDisabled={loadingData} onSeekRow={seekRow}
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
                {/if}
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
        </section>
      {/if}
      </div>
    </main>
  {/snippet}
</AppShell>
{/if}

{#if commandPrefix || sourcePicking}
  {#key combinationId}<CommandHint prefix={commandPrefix} {sourcePicking} {sourceDigits} sourceCount={nodes.length} />{/key}
{/if}
{#if commandMode && commandMode !== 'operation'}
  <CommandDialog
    title={commandMode === 'wheel' ? 'Action wheel' : commandMode === 'find-column' ? 'Find column' : commandMode === 'sort' ? `Sort ${commandColumn?.name ?? 'column'}` : 'Commands'}
    wheel={commandMode === 'wheel'} searchable={commandMode === 'find-column' || commandMode === 'help'}
    simple={commandMode === 'wheel' && actionMenuMode === 'simple'} compact={commandMode === 'find-column'}
    query={commandQuery} items={commandItems} onQuery={(value) => commandQuery = value}
    onChoose={chooseCommand} onClose={() => closeCommands()}
    onBack={commandMode !== 'wheel' && commandMode !== 'find-column' ? () => showCommands('wheel') : undefined}
  />
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
  .workspace-content { display: contents; }
  .workspace-content[hidden] { display: none; }

  .workspace { flex: 1; min-height: 0; display: flex; flex-direction: column; }
  .data-stage { flex: 1; min-height: 0; display: flex; position: relative; }
  .table-pane { min-width: 0; min-height: 0; flex: 1; display: flex; flex-direction: column; }
  .table-pane.compact { --row-height: 26px; }
  .table-pane.comfortable { --row-height: 42px; }
  .table-card { position: relative; min-height: 180px; flex: 1; overflow: hidden; }
  .table-card.recording::before, .table-card.recording::after { content: ''; position: absolute; z-index: 6; top: 0; bottom: 0; width: 2px; pointer-events: none; background: var(--action); }
  .table-card.recording::before { left: 0; }
  .table-card.recording::after { right: 0; }
  .table-state { display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 8px; height: 100%; padding: 40px; text-align: center; color: var(--muted); font-family: var(--font-ui); font-size: 13px; }
  .loading-overlay { position: absolute; inset: 0; z-index: 5; display: flex; align-items: center; justify-content: center; gap: 8px; background: color-mix(in srgb, var(--surface) 72%, transparent); font-size: 12.5px; color: var(--muted); }
  .cell-edit-error { position: absolute; right: 10px; bottom: 10px; z-index: 7; max-width: min(460px, calc(100% - 20px)); padding: 8px 10px; border: 1px solid var(--error); border-radius: var(--radius-md); background: var(--surface); box-shadow: var(--shadow-popover); color: var(--error); font-size: 12px; }
  .banner { margin: 14px 20px; padding: 12px 14px; border-radius: var(--radius-lg); border: 1px solid var(--line); background: var(--surface-inset); font-size: 12.5px; color: var(--muted); }
  .banner strong { display: block; margin-bottom: 4px; color: var(--ink); font-size: 13px; }
  .banner p { margin: 0; }
  .error-banner { display: flex; align-items: center; justify-content: space-between; gap: 12px; border-color: var(--error); }
  .error-banner strong { color: var(--error); }
  .error-banner button { flex: none; height: 28px; padding: 0 12px; border-radius: var(--radius-md); border: 1px solid var(--control-border); background: var(--surface); }
</style>
