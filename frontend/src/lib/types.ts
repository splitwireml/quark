export type FilterOperator = '=' | '!=' | 'in' | 'is_null' | 'not_null' | 'contains' | 'starts_with' | 'ends_with' | '>' | '>=' | '<' | '<=';
export type SortDirection = 'asc' | 'desc';
export type ProfileKind = 'numeric' | 'categorical' | 'date';
export type AggregateCount = number | string;

export interface NodeInfo {
  id: string;
  name: string;
  source: string;
  kind: string;
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
  rows: Record<string, unknown>[];
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
