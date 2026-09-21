import type { IconName } from './icons';
import type {
  AggregateMetric,
  BarLayout,
  ChartEncodings,
  ChartMark,
  ChartSpec,
  ChartSuggestion,
  ChartType,
  ColumnInfo,
  EncodingRole,
  FilterCondition,
  TimeGrain,
  VizKind
} from './types';

const INTEGER_PREFIXES = [
  'TINYINT', 'SMALLINT', 'INTEGER', 'BIGINT', 'HUGEINT',
  'UTINYINT', 'USMALLINT', 'UINTEGER', 'UBIGINT', 'UHUGEINT'
];

type Classified = { name: string; kind: VizKind };

function isIntegerType(type: string): boolean {
  const upper = type.toUpperCase();
  return INTEGER_PREFIXES.some((prefix) => upper.startsWith(prefix));
}

export function classifyColumn(column: Pick<ColumnInfo, 'type' | 'profile_kind' | 'numeric'>): VizKind | null {
  if (column.profile_kind === 'categorical') return 'categorical';
  if (column.profile_kind === 'date') return 'date';
  if (column.profile_kind === 'numeric') return isIntegerType(column.type) ? 'discrete' : 'continuous';
  return null;
}

function of(items: Classified[], kind: VizKind): Classified[] {
  return items.filter((item) => item.kind === kind);
}

function barCount(column: Classified): ChartSuggestion {
  return { chart: 'bar', encodings: { category: column.name }, metric: 'count' };
}

function hist(column: Classified, group?: Classified): ChartSuggestion {
  return { chart: 'histogram', encodings: group ? { group: group.name, value: column.name } : { value: column.name } };
}

function box(column: Classified, group?: Classified): ChartSuggestion {
  return { chart: 'box', encodings: group ? { group: group.name, value: column.name } : { value: column.name } };
}

function barMeasure(category: Classified, value: Classified, group?: Classified): ChartSuggestion {
  return {
    chart: 'bar',
    encodings: group
      ? { category: category.name, value: value.name, group: group.name }
      : { category: category.name, value: value.name },
    metric: 'avg'
  };
}

export function suggestCharts(columns: ColumnInfo[]): ChartSuggestion[] {
  const items = columns
    .map((column) => ({ name: column.name, kind: classifyColumn(column) }))
    .filter((item): item is Classified => item.kind !== null);
  if (items.length === 0) return [];

  const cats = of(items, 'categorical');
  const conts = of(items, 'continuous');
  const dates = of(items, 'date');
  const nums = items.filter((item) => item.kind === 'continuous' || item.kind === 'discrete');

  if (items.length === 1) {
    const [item] = items;
    if (item.kind === 'categorical') return [barCount(item)];
    if (item.kind === 'discrete') return [barCount(item), hist(item), box(item)];
    if (item.kind === 'continuous') return [hist(item), box(item)];
    return [hist(item), barCount(item)];
  }

  if (dates.length === 1 && nums.length === 1 && items.length === 2) {
    const encodings = { x: dates[0].name, y: nums[0].name };
    return [{ chart: 'line', encodings }, { chart: 'scatter', encodings }];
  }

  if (nums.length === 2 && items.length === 2) {
    const encodings = { x: nums[0].name, y: nums[1].name };
    return [{ chart: 'scatter', encodings }, { chart: 'line', encodings }];
  }

  if (cats.length === 1 && nums.length === 1 && items.length === 2) {
    return [barMeasure(cats[0], nums[0]), box(nums[0], cats[0]), hist(nums[0], cats[0])];
  }

  if (nums.length === 3 && items.length === 3) {
    return [{ chart: 'scatter', encodings: { x: nums[0].name, y: nums[1].name, size: nums[2].name } }];
  }

  if (nums.length === 2 && cats.length === 1 && items.length === 3) {
    return [{ chart: 'scatter', encodings: { x: nums[0].name, y: nums[1].name, color: cats[0].name } }];
  }

  if (cats.length === 2 && nums.length === 1 && items.length === 3) {
    return [barMeasure(cats[0], nums[0], cats[1])];
  }

  return [];
}

export function specFromColumns(columns: ColumnInfo[]): ChartSpec | null {
  const suggestion = suggestCharts(columns)[0];
  if (!suggestion) return null;
  return suggestion.metric
    ? { chart: suggestion.chart, encodings: suggestion.encodings, metric: suggestion.metric }
    : { chart: suggestion.chart, encodings: suggestion.encodings };
}

export function filtersFromMark(spec: ChartSpec, mark: ChartMark, existingCount = 0): FilterCondition[] {
  const and = (index: number): Pick<FilterCondition, 'connector'> | object => (
    existingCount + index > 0 ? { connector: 'and' as const } : {}
  );
  if (mark.kind === 'category') {
    const column = spec.encodings.category ?? spec.encodings.group;
    if (!column) return [];
    const filters: FilterCondition[] = [{ column, operator: '=', value: mark.value, ...and(0) }];
    if (mark.series !== undefined && spec.encodings.group && spec.encodings.group !== column) {
      filters.push({ column: spec.encodings.group, operator: '=', value: mark.series, ...and(1) });
    }
    return filters;
  }
  if (mark.kind === 'region') {
    const filters: FilterCondition[] = [];
    if (spec.encodings.x) {
      filters.push({ column: spec.encodings.x, operator: '>=', value: mark.xMin, ...and(filters.length) });
      filters.push({ column: spec.encodings.x, operator: '<=', value: mark.xMax, ...and(filters.length) });
    }
    if (spec.encodings.y) {
      filters.push({ column: spec.encodings.y, operator: '>=', value: mark.yMin, ...and(filters.length) });
      filters.push({ column: spec.encodings.y, operator: '<=', value: mark.yMax, ...and(filters.length) });
    }
    return filters;
  }
  if (mark.kind !== 'bin' || !spec.encodings.value) return [];
  return [
    { column: spec.encodings.value, operator: '>=', value: mark.lower, ...and(0) },
    { column: spec.encodings.value, operator: mark.last ? '<=' : '<', value: mark.upper, ...and(1) }
  ];
}

export function fromPlotNumber(value: number, sample: string | number | boolean): string | number {
  if (typeof sample === 'string' && sample !== '' && Number.isNaN(Number(sample))) {
    const date = new Date(value);
    if (!Number.isFinite(date.getTime())) return value;
    return sample.length > 10 ? date.toISOString().slice(0, 19) : date.toISOString().slice(0, 10);
  }
  return value;
}

export function toPlotNumber(value: string | number | boolean): number {
  if (typeof value === 'number') return value;
  if (typeof value === 'boolean') return value ? 1 : 0;
  const numeric = Number(value);
  if (Number.isFinite(numeric) && value.trim() !== '') return numeric;
  const timestamp = Date.parse(value);
  return Number.isFinite(timestamp) ? timestamp : Number.NaN;
}

function roundStep(value: number, step: number): number {
  const decimals = Math.max(0, Math.min(8, -Math.floor(Math.log10(step) + 1e-12)));
  return Number(value.toFixed(decimals));
}

export function niceTicks(min: number, max: number, count = 5): number[] {
  if (!Number.isFinite(min) || !Number.isFinite(max)) return [0, 1];
  if (min === max) {
    const pad = Math.abs(min) * 0.1 || 1;
    return niceTicks(min - pad, max + pad, count);
  }
  if (min > max) [min, max] = [max, min];
  const span = max - min;
  const target = Math.max(2, count);
  const raw = span / (target - 1);
  const startExp = Math.floor(Math.log10(raw)) - 1;
  let best: number[] = [];
  let bestScore = Infinity;
  for (let exp = startExp; exp <= startExp + 3; exp++) {
    for (const multiplier of [1, 2, 5]) {
      const step = multiplier * 10 ** exp;
      if (step <= 0) continue;
      const start = roundStep(Math.floor(min / step) * step, step);
      const end = roundStep(Math.ceil(max / step) * step, step);
      const ticks: number[] = [];
      const n = Math.round((end - start) / step);
      for (let i = 0; i <= n; i++) ticks.push(roundStep(start + i * step, step));
      if (ticks.length < 2) continue;
      const extra = (ticks[0] === min ? 0 : 0.15) + (ticks[ticks.length - 1] === max ? 0 : 0.15);
      const score = Math.abs(ticks.length - target) + extra + (ticks.length > target + 2 ? ticks.length : 0);
      if (score < bestScore) {
        best = ticks;
        bestScore = score;
      }
    }
  }
  return best;
}

export function formatPlotTick(value: number, sample?: string | number | boolean): string {
  if (typeof sample === 'string' && sample !== '' && Number.isNaN(Number(sample))) {
    const date = new Date(value);
    if (!Number.isFinite(date.getTime())) return '';
    return sample.length > 10 ? date.toISOString().slice(0, 16).replace('T', ' ') : date.toISOString().slice(0, 10);
  }
  return formatTick(value);
}

export function formatTick(value: number): string {
  if (!Number.isFinite(value)) return '';
  const abs = Math.abs(value);
  if (abs >= 1000) return new Intl.NumberFormat(undefined, { maximumFractionDigits: 1, notation: abs >= 10000 ? 'compact' : 'standard' }).format(value);
  if (Number.isInteger(value)) return String(value);
  const digits = abs >= 10 ? 1 : abs >= 1 ? 2 : 3;
  return String(Number(value.toFixed(digits)));
}

export const visualizeMetrics: { value: AggregateMetric; label: string; tip: string }[] = [
  { value: 'avg', label: 'Avg', tip: 'Average of the number in each category' },
  { value: 'median', label: 'Median', tip: 'Median of the number in each category' },
  { value: 'stddev', label: 'Std', tip: 'Standard deviation in each category' },
  { value: 'min', label: 'Min', tip: 'Minimum in each category' },
  { value: 'max', label: 'Max', tip: 'Maximum in each category' },
  { value: 'count', label: 'Count', tip: 'Row count in each category' }
];

export function metricTitle(metric: AggregateMetric | undefined, column: string): string {
  const labels: Record<AggregateMetric, string> = {
    count: 'Count',
    distinct: 'Distinct count',
    min: 'Min',
    max: 'Max',
    sum: 'Sum',
    avg: 'Average',
    median: 'Median',
    stddev: 'Std. dev.'
  };
  if (!metric || metric === 'count') return column ? `Count of ${column}` : 'Rows';
  return `${labels[metric]} of ${column}`;
}

export const chartMeta: Record<ChartType, { label: string; tip: string; icon: IconName }> = {
  bar: { label: 'Bar', tip: 'Counts of each value', icon: 'bar' },
  histogram: { label: 'Histogram', tip: 'Distribution of a number', icon: 'histogram' },
  box: { label: 'Box', tip: 'Median, quartiles, and outliers', icon: 'box' },
  scatter: { label: 'Scatter', tip: 'Two numbers against each other', icon: 'scatter' },
  line: { label: 'Line', tip: 'A number over an ordered axis', icon: 'line' },
  pie: { label: 'Pie', tip: 'Shares of a whole', icon: 'pie' }
};

export function groupColumns(columns: ColumnInfo[]): { kind: VizKind; label: string; columns: ColumnInfo[] }[] {
  const buckets: Record<VizKind, ColumnInfo[]> = { categorical: [], discrete: [], continuous: [], date: [] };
  for (const column of columns) {
    const kind = classifyColumn(column);
    if (kind) buckets[kind].push(column);
  }
  const numbers = [...buckets.discrete, ...buckets.continuous];
  const groups: { kind: VizKind; label: string; columns: ColumnInfo[] }[] = [
    { kind: 'categorical', label: 'Categorical', columns: buckets.categorical },
    { kind: 'continuous', label: 'Numbers', columns: numbers },
    { kind: 'date', label: 'Dates', columns: buckets.date }
  ];
  return groups.filter((group) => group.columns.length > 0);
}

const CHART_ROLES: Record<ChartType, EncodingRole[]> = {
  bar: ['category', 'value', 'group'],
  pie: ['category', 'value'],
  histogram: ['value', 'group'],
  box: ['value', 'group'],
  scatter: ['x', 'y', 'size', 'color', 'pattern'],
  line: ['x', 'y', 'group']
};

export function chartRoles(chart: ChartType): EncodingRole[] {
  return CHART_ROLES[chart];
}

export function applyRoles(
  encodings: ChartEncodings,
  chart: ChartType,
  overrides: Record<string, EncodingRole>,
  selected: ColumnInfo[]
): ChartEncodings {
  const legal = new Set(chartRoles(chart));
  const present = new Set(selected.map((column) => column.name));
  const result: ChartEncodings = { ...encodings };
  for (const [name, role] of Object.entries(overrides)) {
    if (!present.has(name) || !legal.has(role)) continue;
    for (const key of Object.keys(result) as EncodingRole[]) {
      if (result[key] === name) delete result[key];
    }
    result[role] = name;
  }
  return result;
}

export const barLayouts: { value: BarLayout; label: string; tip: string }[] = [
  { value: 'grouped', label: 'Grouped', tip: 'One bar per series, side by side' },
  { value: 'stacked', label: 'Stacked', tip: 'Series stacked into one bar per category' },
  { value: 'stacked100', label: '100%', tip: 'Each bar fills the height, showing shares' }
];

export const timeGrains: { value: TimeGrain; label: string; tip: string }[] = [
  { value: 'hour', label: 'Hour', tip: 'One point per hour' },
  { value: 'day', label: 'Day', tip: 'One point per day' },
  { value: 'week', label: 'Week', tip: 'One point per week' },
  { value: 'month', label: 'Month', tip: 'One point per month' },
  { value: 'quarter', label: 'Quarter', tip: 'One point per quarter' },
  { value: 'year', label: 'Year', tip: 'One point per year' }
];
