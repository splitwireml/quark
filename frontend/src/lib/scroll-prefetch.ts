// Predictive loading math for the endless-feel grid: scroll velocity feeds a
// lookahead horizon, and the pages past the viewport edge (the critical point)
// are fetched in the background before the scroll collides with them.

export const MAX_SCROLL_PX = 10_000_000;
export const MIN_LEAD_MS = 150;
export const MAX_LEAD_MS = 1200;
export const SETTLE_ROWS_PER_SEC = 2;
// Burst cap per viewport report; sustained swipes re-trigger as the viewport moves.
export const MAX_PREFETCH_PAGES = 2;
export const SNAPSHOT_MIN_ROWS = 20;
export const SNAPSHOT_MAX_ROWS = 500;
export const LATENCY_MIN_MS = 80;
export const LATENCY_MAX_MS = 2000;

export function smoothVelocity(previous: number, instant: number, alpha = 0.35): number {
  if (!Number.isFinite(instant)) return previous;
  return previous + alpha * (instant - previous);
}

// Slow scans wait until the last 10% of a page; fast swipes lead by measured latency.
export function prefetchHorizonRows(absVelocityRowsPerSec: number, latencyMs: number, pageSize: number): number {
  const leadSec = Math.min(Math.max(latencyMs, MIN_LEAD_MS), MAX_LEAD_MS) / 1000;
  return Math.max(Math.max(1, Math.ceil(pageSize * 0.1)), Math.ceil(absVelocityRowsPerSec * leadSec));
}

function clampPage(page: number, totalPages: number): number {
  return Math.min(Math.max(Math.floor(page), 1), Math.max(1, Math.floor(totalPages)));
}

// 1-based pages covering an absolute row range, nearest edge first in `order`.
export function pagesForRange(firstRow: number, lastRow: number, pageSize: number, totalPages: number, order: 1 | -1 = 1): number[] {
  const size = Math.max(1, Math.floor(pageSize));
  const pages = Math.max(1, Math.floor(totalPages));
  const from = clampPage(Math.floor(firstRow / size) + 1, pages);
  const to = clampPage(Math.floor(lastRow / size) + 1, pages);
  const list: number[] = [];
  for (let page = Math.min(from, to); page <= Math.max(from, to); page += 1) list.push(page);
  return order === 1 ? list : list.reverse();
}

export type CriticalPlan = {
  firstRow: number;
  lastRow: number;
  velocity: number;
  pageSize: number;
  totalPages: number;
  currentPage: number;
  coveredPages: number[];
  latencyMs: number;
  // Scroll direction remembered while settled, for the near-boundary check at rest.
  lastDir: 1 | -1;
};

// Include missing visible pages first, then at most two pages of lookahead.
export function criticalPages(plan: CriticalPlan): number[] {
  const { firstRow, lastRow, velocity, pageSize, totalPages, coveredPages, latencyMs, lastDir } = plan;
  const covered = new Set(coveredPages);
  const dir: 1 | -1 = Math.abs(velocity) <= SETTLE_ROWS_PER_SEC ? lastDir : velocity > 0 ? 1 : -1;
  const horizon = Math.min(pageSize * MAX_PREFETCH_PAGES, prefetchHorizonRows(Math.abs(velocity), latencyMs, pageSize));
  const edge = dir === 1 ? lastRow : firstRow;
  return [...new Set([
    ...pagesForRange(firstRow, lastRow, pageSize, totalPages),
    ...pagesForRange(edge, edge + dir * horizon, pageSize, totalPages, dir),
  ])].filter((page) => !covered.has(page)).slice(0, MAX_PREFETCH_PAGES);
}

// Compress only the offscreen canvas. Visible rows always retain their natural height.
export function scrollRowStep(totalRows: number, rowHeight: number, viewportHeight: number): number {
  const visible = Math.min(totalRows, Math.max(0, viewportHeight / rowHeight));
  if (totalRows <= visible) return rowHeight;
  return (Math.min(MAX_SCROLL_PX, totalRows * rowHeight) - visible * rowHeight) / (totalRows - visible);
}

// A transient preview around a drag-rest position: sized to the viewport, on
// its own page grid so only the visible snapshot crosses the wire. Absolute
// start offsets come back out of the response's page/page_size.
export function snapshotWindow(absoluteRow: number, viewportRows: number): { page: number; size: number; start: number } {
  const size = Math.min(SNAPSHOT_MAX_ROWS, Math.max(SNAPSHOT_MIN_ROWS, Math.ceil(Math.max(1, viewportRows)) + 8));
  const page = Math.max(1, Math.floor(Math.max(0, absoluteRow) / size) + 1);
  return { page, size, start: (page - 1) * size };
}

export function latencyEma(previous: number, sample: number, alpha = 0.25): number {
  const next = previous + alpha * (sample - previous);
  return Math.min(LATENCY_MAX_MS, Math.max(LATENCY_MIN_MS, next));
}
