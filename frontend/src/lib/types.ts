import type { TableRows } from './table-rows';

export type FilterOperator = '=' | '!=' | 'in' | 'is_null' | 'not_null' | 'contains' | 'starts_with' | 'ends_with' | '>' | '>=' | '<' | '<=';
export type SortDirection = 'asc' | 'desc';
export type ProfileKind = 'numeric' | 'categorical' | 'date';
export type VizKind = 'categorical' | 'discrete' | 'continuous' | 'date';
export type ChartType = 'bar' | 'histogram' | 'box' | 'scatter' | 'line' | 'pie';
export type BarLayout = 'grouped' | 'stacked' | 'stacked100';
export type TimeGrain = 'hour' | 'day' | 'week' | 'month' | 'quarter' | 'year';
export type AggregateCount = number | string;

export interface SourceSummary {
  id: string;
  name: string;
}

export interface NodeInfo extends SourceSummary {
  source?: string;
  kind: string;
  project_id?: string;
}

export interface ProjectInfo {
  id: string;
  name: string;
  node_id: string;
  source_count: number;
}

export interface WorkbookPreview {
  id: string;
  name: string;
  kind: 'workbook';
  sheets: string[];
}

export interface DatasetInfo {
  id: string;
  name: string;
  schema: string;
  type: string;
  columns: string[];
}

export interface BaseViewInfo extends DatasetInfo {
  project_id: string;
  source_id: string;
  source_name: string;
  node_id: string;
  sql: string;
}

export interface ProjectSourceInfo extends NodeInfo {
  project_id: string;
  views: BaseViewInfo[];
}

export interface CategoryValue {
  value: string;
  count: AggregateCount;
}

export interface ProfileValue {
  value: string | boolean;
  count: AggregateCount;
}

export interface CategoryValuesResponse {
  values: CategoryValue[];
  total: AggregateCount;
  offset: number;
  limit: number;
  has_more: boolean;
}

export interface ColumnInfo {
  name: string;
  type: string;
  numeric: boolean;
  null_fraction: number;
  profile_kind: ProfileKind | null;
}

export interface FilterCondition {
  column: string;
  operator: FilterOperator;
  connector?: 'and' | 'or';
  value?: string | number | boolean | (string | number | boolean)[];
}

export interface SortCondition {
  column: string;
  direction: SortDirection;
}

export interface QueryRequest {
  page: number;
  page_size: number;
  filters: FilterCondition[];
  sorts: SortCondition[];
  dedupe_columns: string[];
}

export interface QueryResponse {
  columns: ColumnInfo[];
  rows: TableRows;
  page: number;
  page_size: number;
  total_rows: AggregateCount;
  total_pages: AggregateCount;
  elapsed_ms: number;
  sql: string;
}

export interface SqlQueryRequest extends QueryRequest {
  sql: string;
}

export type JoinReference =
  | { node_id: string; dataset: string; sql?: never; name?: never }
  | { node_id: string; dataset?: never; sql: string; name?: string };

export interface JoinWorkspaceRequest {
  left: JoinReference;
  right: JoinReference;
  left_keys: string[];
  right_keys: string[];
}

export interface JoinWorkspaceDataset {
  schema: string;
  name: string;
}

export type JoinRelationship = 'cartesian' | 'one_to_one' | 'one_to_many' | 'many_to_one' | 'many_to_many';

export interface JoinWorkspaceResponse {
  node_id: string;
  left: JoinWorkspaceDataset;
  right: JoinWorkspaceDataset;
  left_rows: AggregateCount;
  right_rows: AggregateCount;
  output_rows: AggregateCount;
  relationship: JoinRelationship;
  cartesian_risk: boolean;
}

export type NumericValue = number | string;

export interface HistogramBin {
  lower: NumericValue;
  upper: NumericValue;
  count: AggregateCount;
}

interface ColumnStatsBase {
  type: string;
  kind: ProfileKind;
  row_count: AggregateCount;
  non_null_count: AggregateCount;
  null_count: AggregateCount;
  null_fraction: number;
}

export interface NumericColumnStats extends ColumnStatsBase {
  kind: 'numeric';
  min: NumericValue | null;
  max: NumericValue | null;
  mean: NumericValue | null;
  stddev: NumericValue | null;
  p25: NumericValue | null;
  median: NumericValue | null;
  p75: NumericValue | null;
  histogram: HistogramBin[];
}

export interface CategoricalColumnStats extends ColumnStatsBase {
  kind: 'categorical';
  distinct_count: AggregateCount;
  top_values: ProfileValue[];
}

export interface DateYearCount {
  year: string;
  count: AggregateCount;
}

export interface DateColumnStats extends ColumnStatsBase {
  kind: 'date';
  min: string | null;
  max: string | null;
  distinct_count: AggregateCount;
  histogram: HistogramBin[];
  year_counts: DateYearCount[];
}

export type ColumnStats = NumericColumnStats | CategoricalColumnStats | DateColumnStats;

export type AggregateMetric = 'count' | 'distinct' | 'min' | 'max' | 'sum' | 'avg' | 'median' | 'stddev';
export type AggregateRecipeItem = { id: number; column: string; metrics: AggregateMetric[] | null };

export interface ChartEncodings {
  x?: string;
  y?: string;
  category?: string;
  value?: string;
  group?: string;
  size?: string;
  color?: string;
  pattern?: string;
}

export type EncodingRole = keyof ChartEncodings;

export interface ChartSpec {
  chart: ChartType;
  encodings: ChartEncodings;
  metric?: AggregateMetric;
  density?: boolean;
  layout?: BarLayout;
  grain?: TimeGrain;
}

export interface DashboardChart {
  id: string;
  title: string;
  spec: ChartSpec;
}

export interface DashboardPlacement {
  id: string;
  chartId: string;
  x: number;
  y: number;
  width: number;
  height: number;
}

export interface DashboardTab {
  id: string;
  name: string;
  scrollTop: number;
  placements: DashboardPlacement[];
}

export interface DashboardDataset {
  datasetId: string;
  activeTabId: string;
  charts: DashboardChart[];
  tabs: DashboardTab[];
}

export interface DashboardSelection {
  chartId: string;
  filters: FilterCondition[];
}

export interface ChartSuggestion {
  chart: ChartType;
  encodings: ChartEncodings;
  metric?: AggregateMetric;
}

export type ChartMark =
  | { kind: 'category'; value: string | boolean | number }
  | { kind: 'bin'; lower: NumericValue; upper: NumericValue; last?: boolean }
  | { kind: 'region'; xMin: NumericValue; xMax: NumericValue; yMin: NumericValue; yMax: NumericValue };

export interface BarVisualizeRow {
  label: string | boolean | number;
  value: AggregateCount;
  n?: AggregateCount;
}

export interface BarVisualizeResponse {
  chart: 'bar';
  rows: BarVisualizeRow[];
  other_count: AggregateCount;
  elapsed_ms: number;
}

export interface HistogramVisualizeResponse {
  chart: 'histogram';
  bins: HistogramBin[];
  elapsed_ms: number;
}

export interface BoxGroup {
  label: string | boolean | number;
  p25: NumericValue;
  median: NumericValue;
  p75: NumericValue;
  whisker_low: NumericValue;
  whisker_high: NumericValue;
  outliers: NumericValue[];
  count: AggregateCount;
}

export interface BoxVisualizeResponse {
  chart: 'box';
  groups: BoxGroup[];
  elapsed_ms: number;
}

export interface ScatterPoint {
  x: NumericValue;
  y: NumericValue;
  color?: string | number | boolean | null;
}

export interface ScatterVisualizeResponse {
  chart: 'scatter';
  points: ScatterPoint[];
  total_points: AggregateCount;
  elapsed_ms: number;
}

export type VisualizeResponse = BarVisualizeResponse | HistogramVisualizeResponse | BoxVisualizeResponse | ScatterVisualizeResponse;

export interface ChartHover {
  title: string;
  lines: string[];
  hint?: string;
}

export interface VisualizeRequest extends QueryRequest {
  spec: ChartSpec;
}

export interface SqlVisualizeRequest extends SqlQueryRequest {
  spec: ChartSpec;
}

export type RowDensity = 'compact' | 'default' | 'comfortable';
export type DistributionMode = 'count' | 'percent';

export interface SavedQuery {
  id: string;
  name: string;
  sql: string;
  nodeId: string;
  dataset: string;
}

export type SerializableValue = null | boolean | number | string | SerializableValue[] | { [key: string]: SerializableValue };

export interface VersionChange {
  kind: string;
  summary: string;
  details?: Record<string, SerializableValue>;
}

export interface Version {
  id: string;
  parentId?: string;
  number: number;
  fork?: number;
  nodeId: string;
  dataset: string;
  sql: string;
  columns: string[];
  hiddenColumns: string[];
  timestamp: string;
  changes: VersionChange[];
  join?: JoinWorkspaceRequest;
}

export interface View {
  id: string;
  name: string;
  nodeId: string;
  dataset: string;
  sql: string;
  timestamp: string;
  join?: JoinWorkspaceRequest;
}

export type ViewKind = 'source' | 'derived';

export interface ViewHistory {
  id: string;
  projectId: string;
  name: string;
  kind: ViewKind;
  sourceId?: string;
  nodeId: string;
  dataset: string;
  versions: Version[];
  activeVersionId: string;
  pendingParentId: string | null;
  pendingChanges: VersionChange[];
}

export interface DatasetVersionHistory {
  nodeId: string;
  dataset: string;
  versions: Version[];
  views: View[];
  activeVersionId: string;
  pendingParentId: string | null;
  pendingChanges: VersionChange[];
}

export interface VersionRestoreMetadata {
  nodeId: string;
  dataset: string;
  sql: string;
  columns: string[];
  hiddenColumns: string[];
  join?: JoinWorkspaceRequest;
}

export interface VersionDiff {
  parentId: string;
  versionId: string;
  before: VersionRestoreMetadata;
  after: VersionRestoreMetadata;
  changes: VersionChange[];
}

export type ExportFormat = 'csv' | 'xlsx' | 'parquet' | 'json';

export type JsonLayout = 'rows' | 'columns';

export interface ExportSheetRequest {
  node_id: string;
  name: string;
  sql: string;
}

export interface ExportOption extends ExportSheetRequest {
  key: string;
  source: string;
}

export interface ExportRequest {
  format: ExportFormat;
  json_layout?: JsonLayout;
  filename?: string;
  sheets: ExportSheetRequest[];
}

export interface ExportDownload {
  blob: Blob;
  filename: string;
}
