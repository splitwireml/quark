import type { BaseViewInfo, CategoryValuesResponse, ColumnStats, DatasetInfo, ExportDownload, ExportRequest, JoinWorkspaceRequest, JoinWorkspaceResponse, NodeInfo, ProjectInfo, ProjectSourceInfo, QueryRequest, QueryResponse, SourceSummary, SqlQueryRequest, WorkbookPreview } from './types';

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
  return response.status === 204 ? undefined as T : response.json() as Promise<T>;
}

const json = (body: unknown): RequestInit => ({
  method: 'POST',
  headers: { 'Content-Type': 'application/json' },
  body: JSON.stringify(body)
});

export function listProjects(): Promise<ProjectInfo[]> {
  return request('/api/projects');
}

export function createProject(name: string): Promise<ProjectInfo> {
  return request('/api/projects', json({ name }));
}

export function listProjectSources(projectId: string): Promise<SourceSummary[]> {
  return request(`/api/projects/${encodeURIComponent(projectId)}/sources`);
}

export function getProjectSource(projectId: string, sourceId: string): Promise<ProjectSourceInfo> {
  return request(`/api/projects/${encodeURIComponent(projectId)}/sources/${encodeURIComponent(sourceId)}`);
}

export function listProjectViews(projectId: string): Promise<BaseViewInfo[]> {
  return request(`/api/projects/${encodeURIComponent(projectId)}/views`);
}

export function uploadNode(projectId: string, file: File): Promise<NodeInfo | WorkbookPreview> {
  const body = new FormData();
  body.append('file', file);
  return request(`/api/projects/${encodeURIComponent(projectId)}/sources/upload`, { method: 'POST', body });
}

export function confirmWorkbook(projectId: string, id: string, sheets: string[]): Promise<NodeInfo> {
  return request(`/api/projects/${encodeURIComponent(projectId)}/sources/upload/${encodeURIComponent(id)}/confirm`, json({ sheets }));
}

export function discardWorkbook(projectId: string, id: string): Promise<void> {
  return request(`/api/projects/${encodeURIComponent(projectId)}/sources/upload/${encodeURIComponent(id)}`, { method: 'DELETE' });
}

export function attachNode(projectId: string, path: string): Promise<NodeInfo> {
  return request(`/api/projects/${encodeURIComponent(projectId)}/sources/attach`, json({ path }));
}

export function listDatasets(nodeId: string): Promise<DatasetInfo[]> {
  return request(`/api/nodes/${encodeURIComponent(nodeId)}/datasets`);
}

export function queryDataset(nodeId: string, dataset: string, body: QueryRequest): Promise<QueryResponse> {
  return request(`/api/nodes/${encodeURIComponent(nodeId)}/datasets/${encodeURIComponent(dataset)}/query`, json(body));
}

export function querySql(nodeId: string, body: SqlQueryRequest): Promise<QueryResponse> {
  return request(`/api/nodes/${encodeURIComponent(nodeId)}/sql`, json(body));
}

export function previewJoinWorkspace(body: JoinWorkspaceRequest): Promise<JoinWorkspaceResponse> {
  return request('/api/join-workspaces', json(body));
}

export async function exportData(body: ExportRequest): Promise<ExportDownload> {
  const response = await fetch('/api/exports', json(body));
  if (!response.ok) {
    let message = `${response.status} ${response.statusText}`;
    try {
      const detail = await response.json();
      message = detail.detail ?? detail.message ?? message;
    } catch { /* use HTTP status */ }
    throw new Error(typeof message === 'string' ? message : JSON.stringify(message));
  }
  const disposition = response.headers.get('Content-Disposition') ?? '';
  const encoded = disposition.match(/filename\*=UTF-8''([^;]+)/i)?.[1];
  const quoted = disposition.match(/filename="([^"]+)"/i)?.[1];
  const plain = disposition.match(/filename=([^;\s]+)/i)?.[1];
  return { blob: await response.blob(), filename: encoded ? decodeURIComponent(encoded) : quoted ?? plain ?? `quark-export.${body.format}` };
}

export function getSqlColumnStats(nodeId: string, column: string, body: SqlQueryRequest): Promise<ColumnStats> {
  return request(`/api/nodes/${encodeURIComponent(nodeId)}/sql/columns/${encodeURIComponent(column)}/stats`, json(body));
}

export function getColumnStats(nodeId: string, dataset: string, column: string, body: QueryRequest): Promise<ColumnStats> {
  return request(`/api/nodes/${encodeURIComponent(nodeId)}/datasets/${encodeURIComponent(dataset)}/columns/${encodeURIComponent(column)}/stats`, json(body));
}

export function getSqlCategoryValues(
  nodeId: string,
  column: string,
  body: Pick<SqlQueryRequest, 'sql'>,
  params: { search?: string; offset?: number; limit?: number } = {}
): Promise<CategoryValuesResponse> {
  const query = new URLSearchParams();
  if (params.search) query.set('search', params.search);
  if (params.offset !== undefined) query.set('offset', String(params.offset));
  if (params.limit !== undefined) query.set('limit', String(params.limit));
  const suffix = query.size ? `?${query}` : '';
  return request(`/api/nodes/${encodeURIComponent(nodeId)}/sql/columns/${encodeURIComponent(column)}/values${suffix}`, json(body));
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
