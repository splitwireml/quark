import type { CategoryValuesResponse, ColumnStats, DatasetInfo, NodeInfo, QueryRequest, QueryResponse } from './types';

async function request<T>(url: string, init?: RequestInit): Promise<T> {
  const response = await fetch(url, init);
  if (!response.ok) {
    let message = `${response.status} ${response.statusText}`;
    try {
      const body = await response.json();
      message = body.detail ?? body.message ?? message;
    } catch { /* use HTTP status */ }
    throw new Error(typeof message === 'string' ? message : JSON.stringify(message));
  }
  return response.json() as Promise<T>;
}

const json = (body: unknown): RequestInit => ({
  method: 'POST',
  headers: { 'Content-Type': 'application/json' },
  body: JSON.stringify(body)
});

export function listNodes(): Promise<NodeInfo[]> {
  return request('/api/nodes');
}

export function uploadNode(file: File): Promise<NodeInfo> {
  const body = new FormData();
  body.append('file', file);
  return request('/api/nodes/upload', { method: 'POST', body });
}

export function attachNode(path: string): Promise<NodeInfo> {
  return request('/api/nodes/attach', json({ path }));
}

export function listDatasets(nodeId: string): Promise<DatasetInfo[]> {
  return request(`/api/nodes/${encodeURIComponent(nodeId)}/datasets`);
}

export function queryDataset(nodeId: string, dataset: string, body: QueryRequest): Promise<QueryResponse> {
  return request(`/api/nodes/${encodeURIComponent(nodeId)}/datasets/${encodeURIComponent(dataset)}/query`, json(body));
}

export function getColumnStats(nodeId: string, dataset: string, column: string): Promise<ColumnStats> {
  return request(`/api/nodes/${encodeURIComponent(nodeId)}/datasets/${encodeURIComponent(dataset)}/columns/${encodeURIComponent(column)}/stats`);
}

export function getCategoryValues(
  nodeId: string,
  dataset: string,
  column: string,
  params: { search?: string; offset?: number; limit?: number } = {}
): Promise<CategoryValuesResponse> {
  const query = new URLSearchParams();
  if (params.search) query.set('search', params.search);
  if (params.offset !== undefined) query.set('offset', String(params.offset));
  if (params.limit !== undefined) query.set('limit', String(params.limit));
  const suffix = query.size ? `?${query}` : '';
  return request(`/api/nodes/${encodeURIComponent(nodeId)}/datasets/${encodeURIComponent(dataset)}/columns/${encodeURIComponent(column)}/values${suffix}`);
}
