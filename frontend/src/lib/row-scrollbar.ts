import type { AggregateCount } from './types';

// Smallest thumb that stays grabbable when the dataset dwarfs the viewport.
export const MIN_THUMB_PX = 32;

// total_rows can arrive as an approximate string beyond float precision; the
// scrollbar only needs a finite bound, so cap it instead of handling BigInt.
export function safeTotalRows(total: AggregateCount): number {
  const numeric = typeof total === 'number' ? total : Number(total);
  if (!Number.isFinite(numeric)) return 0;
  return Math.max(0, Math.min(Math.floor(numeric), Number.MAX_SAFE_INTEGER));
}

export type ThumbGeometry = {
  // False when every row fits in the viewport and no scrollbar is needed.
  scrollable: boolean;
  thumbHeight: number;
  thumbTop: number;
};

// All row arguments are absolute dataset rows (fractions allowed); rows share
// one height per density, so geometry is pure proportion with no measurement.
export function thumbGeometry(
  trackHeight: number,
  totalRows: number,
  firstVisibleRow: number,
  visibleRows: number,
): ThumbGeometry {
  if (!(trackHeight > 0) || !(totalRows > 0) || !(visibleRows > 0) || totalRows <= visibleRows) {
    return { scrollable: false, thumbHeight: 0, thumbTop: 0 };
  }
  const maxFirst = Math.max(0, totalRows - visibleRows);
  const first = Math.min(Math.max(firstVisibleRow, 0), maxFirst);
  const thumbHeight = Math.min(trackHeight, Math.max(MIN_THUMB_PX, (trackHeight * visibleRows) / totalRows));
  const travel = Math.max(0, trackHeight - thumbHeight);
  const thumbTop = maxFirst <= 0 || travel <= 0 ? 0 : (travel * first) / maxFirst;
  return { scrollable: true, thumbHeight, thumbTop };
}

export function clampAbsoluteRow(row: number, totalRows: number): number {
  if (!(totalRows > 0) || !Number.isFinite(row)) return 0;
  return Math.min(Math.max(Math.floor(row), 0), Math.max(0, Math.ceil(totalRows) - 1));
}

// Splits an absolute dataset row into the 1-based page holding it and the
// row's index within that page. Out-of-range rows pin to the nearest page edge.
export function absoluteRowToPage(
  absoluteRow: number,
  pageSize: number,
  totalPages: number,
): { page: number; intraRow: number } {
  const size = Math.max(1, Math.floor(pageSize));
  const pages = Math.max(1, Math.floor(totalPages));
  const page = Math.min(Math.max(Math.floor(absoluteRow / size) + 1, 1), pages);
  const intraRow = Math.min(Math.max(absoluteRow - (page - 1) * size, 0), size - 1);
  return { page, intraRow };
}
