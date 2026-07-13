export type FilterOperator = '=' | '!=' | 'in' | 'is_null' | 'not_null' | 'contains' | 'starts_with' | 'ends_with' | '>' | '>=' | '<' | '<=';
export type SortDirection = 'asc' | 'desc';

export interface NodeInfo {
  id: string;
  name: string;
  source: string;
  kind: string;
}

export interface DatasetInfo {
  id: string;
  name: string;
  schema: string;
  type: string;
}

export interface CategoryValue {
  value: string;
  count: number;
}

export interface ColumnInfo {
  name: string;
  type: string;
  numeric: boolean;
  null_fraction: number;
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
}

export interface QueryResponse {
  columns: ColumnInfo[];
  rows: Record<string, unknown>[];
  page: number;
  page_size: number;
  total_rows: number;
  total_pages: number;
  elapsed_ms: number;
}

export interface HistogramBin {
  lower: number;
  upper: number;
  count: number;
}

export interface ColumnStats {
  type: string;
  row_count: number;
  non_null_count: number;
  null_count: number;
  null_fraction: number;
  min: number | null;
  max: number | null;
  mean: number | null;
  stddev: number | null;
  p25: number | null;
  median: number | null;
  p75: number | null;
  histogram: HistogramBin[];
}
