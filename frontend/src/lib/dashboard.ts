import type { ChartSpec, DashboardDataset, DashboardPlacement, DashboardSelection, DashboardTab, FilterCondition } from './types';

export const DASHBOARD_STORAGE_KEY = 'quark.dashboards.v1';
export const MIN_TILE = { width: 64, height: 48 } as const;
export const DEFAULT_TILE = { width: 560, height: 360 } as const;

export function emptyDashboardDataset(datasetId: string): DashboardDataset {
  return { datasetId, activeTabId: '', charts: [], tabs: [] };
}

export function chartTitle(spec: ChartSpec): string {
  const fields = [
    spec.encodings.category, spec.encodings.x, spec.encodings.value, spec.encodings.y,
    spec.encodings.group, spec.encodings.size, spec.encodings.color, spec.encodings.pattern
  ].filter(Boolean);
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

export function nextDashboardName(names: readonly string[]): string {
  let number = 1;
  while (names.includes(`Dashboard_${number}`)) number++;
  return `Dashboard_${number}`;
}

export function clampPlacement(placement: DashboardPlacement, boardWidth = Infinity): DashboardPlacement {
  const width = Math.min(boardWidth, Math.max(MIN_TILE.width, Math.round(placement.width)));
  return {
    ...placement,
    width,
    height: Math.max(MIN_TILE.height, Math.round(placement.height)),
    x: Math.max(0, Math.min(boardWidth - width, Math.round(placement.x))),
    y: Math.max(0, Math.round(placement.y))
  };
}

export type SnapGuide = { axis: 'x' | 'y'; position: number; start: number; end: number };

export function snapResize(placement: DashboardPlacement, neighbors: DashboardPlacement[], threshold = 6) {
  return snapPlacement(placement, neighbors, 'resize', threshold);
}

export function snapMove(placement: DashboardPlacement, neighbors: DashboardPlacement[], boardWidth = Infinity, threshold = 6) {
  return snapPlacement(placement, neighbors, 'move', threshold, boardWidth);
}

function snapPlacement(placement: DashboardPlacement, neighbors: DashboardPlacement[], mode: 'move' | 'resize', threshold: number, boardWidth = Infinity) {
  const guides: SnapGuide[] = [];
  const result = { ...placement };
  for (const axis of ['x', 'y'] as const) {
    const size = axis === 'x' ? 'width' : 'height';
    const cross = axis === 'x' ? 'y' : 'x';
    const crossSize = axis === 'x' ? 'height' : 'width';
    const offsets = mode === 'move' ? [0, placement[size] / 2, placement[size]] : [placement[size]];
    let nearest: { position: number; delta: number; neighbor: DashboardPlacement } | undefined;
    let distance = threshold + 1;
    for (const neighbor of neighbors) {
      if (neighbor.id === placement.id) continue;
      const targets = [neighbor[axis], neighbor[axis] + neighbor[size]];
      if (mode === 'move') targets.push(neighbor[axis] + neighbor[size] / 2);
      for (const position of targets) {
        for (const offset of offsets) {
          const delta = position - (placement[axis] + offset);
          const origin = placement[axis] + delta;
          const fits = mode === 'resize' ? position - placement[axis] >= MIN_TILE[size]
            : origin >= 0 && (axis !== 'x' || origin + placement.width <= boardWidth);
          if (Math.abs(delta) <= threshold && Math.abs(delta) < distance && fits) {
            nearest = { position, delta, neighbor };
            distance = Math.abs(delta);
          }
        }
      }
    }
    if (nearest) {
      if (mode === 'move') result[axis] += nearest.delta;
      else result[size] += nearest.delta;
      guides.push({ axis, position: nearest.position,
        start: Math.min(result[cross], nearest.neighbor[cross]) - 8,
        end: Math.max(result[cross] + result[crossSize], nearest.neighbor[cross] + nearest.neighbor[crossSize]) + 8 });
    }
  }
  return { placement: result, guides };
}

export function updatePlacedChart(document: DashboardDataset, placementId: string, spec: ChartSpec): DashboardDataset {
  const placement = document.tabs.find((tab) => tab.id === document.activeTabId)?.placements.find((item) => item.id === placementId);
  const chart = document.charts.find((item) => item.id === placement?.chartId);
  if (!placement || !chart || JSON.stringify(chart.spec) === JSON.stringify(spec)) return document;
  // Reused charts share a definition until one tile is edited.
  const shared = document.tabs.flatMap((tab) => tab.placements).filter((item) => item.chartId === chart.id).length > 1;
  const updated = { ...chart, id: shared ? crypto.randomUUID() : chart.id,
    title: chart.title === chartTitle(chart.spec) ? chartTitle(spec) : chart.title,
    spec: { ...spec, encodings: { ...spec.encodings } }
  };
  return updateActiveDashboardTab({ ...document,
    charts: shared ? [...document.charts, updated] : document.charts.map((item) => item.id === chart.id ? updated : item)
  }, (tab) => ({ ...tab, placements: tab.placements.map((item) => item.id === placementId ? { ...item, chartId: updated.id } : item) }));
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
