import type { ChartSpec, DashboardDataset, DashboardPlacement, DashboardSelection, DashboardTab, FilterCondition } from './types';

export const DASHBOARD_STORAGE_KEY = 'quark.dashboards.v1';
export const DASHBOARD_WIDTH = 1200;
export const DEFAULT_TILE = { width: 560, height: 360 } as const;

export function emptyDashboardDataset(datasetId: string): DashboardDataset {
  return { datasetId, activeTabId: '', charts: [], tabs: [] };
}

export function chartTitle(spec: ChartSpec): string {
  const fields = [spec.encodings.category, spec.encodings.x, spec.encodings.value, spec.encodings.y, spec.encodings.group].filter(Boolean);
  return `${spec.chart[0].toUpperCase()}${spec.chart.slice(1)} · ${fields.join(' × ') || 'Chart'}`;
}

export function normalizeFilters(filters: FilterCondition[]): FilterCondition[] {
  return filters.map((filter, index) => {
    if (index) return { ...filter };
    const { connector: _connector, ...rest } = filter;
    return rest;
  });
}

function filterKey(filter: FilterCondition): string {
  return JSON.stringify([filter.column, filter.operator, filter.value]);
}

export function withoutSelections(filters: FilterCondition[], selections: DashboardSelection[]): FilterCondition[] {
  const removed = new Map<string, number>();
  for (const selection of selections) {
    for (const filter of selection.filters) {
      const key = filterKey(filter);
      removed.set(key, (removed.get(key) ?? 0) + 1);
    }
  }
  return normalizeFilters(filters.filter((filter, index) => {
    const key = filterKey(filter);
    const count = removed.get(key) ?? 0;
    if (!count) return true;
    const later = filters.slice(index + 1).filter((item) => filterKey(item) === key).length;
    if (later >= count) return true;
    removed.set(key, count - 1);
    return false;
  }));
}

export function composeFilters(base: FilterCondition[], selections: DashboardSelection[]): FilterCondition[] {
  const combined = normalizeFilters(base);
  for (const filter of selections.flatMap((selection) => selection.filters)) {
    const { connector: _connector, ...rest } = filter;
    combined.push(combined.length ? { ...rest, connector: 'and' } : rest);
  }
  return combined;
}

export function filtersForChart(filters: FilterCondition[], selections: DashboardSelection[], chartId: string): FilterCondition[] {
  const base = withoutSelections(filters, selections);
  return composeFilters(base, selections.filter((selection) => selection.chartId !== chartId));
}

export function updateActiveDashboardTab(document: DashboardDataset, update: (tab: DashboardTab) => DashboardTab): DashboardDataset {
  return { ...document, tabs: document.tabs.map((tab) => tab.id === document.activeTabId ? update(tab) : tab) };
}

export function selectionChartIdAtFilterIndex(filters: FilterCondition[], selections: DashboardSelection[], index: number): string | undefined {
  const baseLength = withoutSelections(filters, selections).length;
  if (index < baseLength) return undefined;
  let offset = baseLength;
  for (const selection of selections) {
    if (index < offset + selection.filters.length) return selection.chartId;
    offset += selection.filters.length;
  }
}

export function clampPlacement(placement: DashboardPlacement): DashboardPlacement {
  const width = Math.min(DASHBOARD_WIDTH, Math.max(280, Math.round(placement.width)));
  const height = Math.max(220, Math.round(placement.height));
  return {
    ...placement,
    width,
    height,
    x: Math.max(0, Math.min(DASHBOARD_WIDTH - width, Math.round(placement.x))),
    y: Math.max(0, Math.round(placement.y))
  };
}

export function readDashboards(value: string | null): DashboardDataset[] {
  if (!value) return [];
  try {
    const parsed: unknown = JSON.parse(value);
    if (!Array.isArray(parsed)) return [];
    return parsed.filter((item): item is DashboardDataset => {
      if (!item || typeof item !== 'object') return false;
      const dataset = item as Partial<DashboardDataset>;
      return typeof dataset.datasetId === 'string' && typeof dataset.activeTabId === 'string'
        && Array.isArray(dataset.charts) && dataset.charts.every((chart) => {
          if (!chart || typeof chart !== 'object') return false;
          const value = chart as unknown as Record<string, unknown>;
          const spec = value.spec as Record<string, unknown> | undefined;
          return typeof value.id === 'string' && typeof value.title === 'string' && !!spec
            && ['bar', 'histogram', 'box', 'scatter', 'line'].includes(String(spec.chart))
            && !!spec.encodings && typeof spec.encodings === 'object';
        })
        && Array.isArray(dataset.tabs) && dataset.tabs.every((tab) => {
          if (!tab || typeof tab !== 'object') return false;
          const value = tab as unknown as Record<string, unknown>;
          return typeof value.id === 'string' && typeof value.name === 'string' && typeof value.scrollTop === 'number'
            && Array.isArray(value.placements) && value.placements.every((placement) => {
              if (!placement || typeof placement !== 'object') return false;
              const entry = placement as Record<string, unknown>;
              return typeof entry.id === 'string' && typeof entry.chartId === 'string'
                && ['x', 'y', 'width', 'height'].every((key) => typeof entry[key] === 'number' && Number.isFinite(entry[key]));
            });
        });
    });
  } catch {
    return [];
  }
}
