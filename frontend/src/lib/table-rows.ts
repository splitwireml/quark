import { tableFromIPC } from 'apache-arrow';
import type { QueryResponse } from './types';

export const ARROW_MEDIA_TYPE = 'application/vnd.apache.arrow.stream';
const MIN_SAFE_INTEGER = BigInt(Number.MIN_SAFE_INTEGER);
const MAX_SAFE_INTEGER = BigInt(Number.MAX_SAFE_INTEGER);

// Cached pages retain column buffers. Only cells requested by the grid are decoded.
export interface TableRows {
  readonly length: number;
  cell(row: number, column: string): unknown;
}

export function jsonRows(rows: Record<string, unknown>[]): TableRows {
  return { length: rows.length, cell: (row, column) => rows[row]?.[column] };
}

export function concatRows(chunks: TableRows[]): TableRows {
  return {
    length: chunks.reduce((sum, chunk) => sum + chunk.length, 0),
    cell(row, column) {
      if (row < 0) return undefined;
      for (const chunk of chunks) {
        if (row < chunk.length) return chunk.cell(row, column);
        row -= chunk.length;
      }
      return undefined;
    },
  };
}

export function arrowQuery(bytes: Uint8Array): QueryResponse {
  const table = tableFromIPC(bytes);
  const encoded = table.schema.metadata.get('quark');
  if (!encoded) throw new Error('Missing Arrow query metadata');
  const { json_columns = [], ...metadata } = JSON.parse(encoded);
  const jsonColumns = new Set<string>(json_columns);
  const columns = new Map(table.schema.fields.map((field, index) => [field.name, table.getChildAt(index)!]));
  return {
    ...metadata,
    rows: {
      length: table.numRows,
      cell(row, column) {
        if (row < 0 || row >= table.numRows) return undefined;
        const value = columns.get(column)?.get(row);
        if (value == null) return value;
        if (jsonColumns.has(column)) return JSON.parse(value);
        if (typeof value === 'bigint') return value >= MIN_SAFE_INTEGER && value <= MAX_SAFE_INTEGER ? Number(value) : String(value);
        if (typeof value === 'number') return Number.isFinite(value) ? value : null;
        if (value instanceof Uint8Array) return Array.from(value, byte => byte.toString(16).padStart(2, '0')).join('');
        return value;
      },
    },
  };
}

export async function decodeQueryResponse(response: Response): Promise<QueryResponse> {
  if (response.headers.get('content-type')?.includes(ARROW_MEDIA_TYPE)) {
    return arrowQuery(new Uint8Array(await response.arrayBuffer()));
  }
  const result = await response.json();
  return { ...result, rows: jsonRows(result.rows) };
}
