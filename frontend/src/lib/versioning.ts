import type {
  DatasetVersionHistory,
  JoinWorkspaceRequest,
  SavedQuery,
  Version,
  VersionChange,
  VersionDiff,
  VersionRestoreMetadata,
  View
} from './types.ts';

export const LEGACY_STORAGE_KEY = 'quark.savedQueries';
export const VERSIONING_STORAGE_KEY = 'quark.versioning.v1';

type SnapshotInput = Pick<Version, 'sql' | 'columns' | 'hiddenColumns' | 'timestamp' | 'join'>;
type SourceInput = SnapshotInput & Pick<Version, 'nodeId' | 'dataset'>;
type ViewInput = Omit<View, 'nodeId' | 'dataset'>;

function copyJoin(join: JoinWorkspaceRequest): JoinWorkspaceRequest {
  return {
    left: { ...join.left },
    right: { ...join.right },
    left_keys: [...join.left_keys],
    right_keys: [...join.right_keys]
  };
}

function restoreMetadata(version: Version): VersionRestoreMetadata {
  return {
    nodeId: version.nodeId,
    dataset: version.dataset,
    sql: version.sql,
    columns: [...version.columns],
    hiddenColumns: [...version.hiddenColumns],
    ...(version.join ? { join: copyJoin(version.join) } : {})
  };
}

export function createSourceHistory(source: SourceInput): DatasetVersionHistory {
  return {
    nodeId: source.nodeId,
    dataset: source.dataset,
    versions: [{
      ...source,
      columns: [...source.columns],
      hiddenColumns: [...source.hiddenColumns],
      ...(source.join ? { join: copyJoin(source.join) } : {}),
      id: 'v1',
      number: 1,
      changes: []
    }],
    views: [],
    activeVersionId: 'v1',
    pendingParentId: null,
    pendingChanges: []
  };
}

export function activateVersion(history: DatasetVersionHistory, versionId: string): DatasetVersionHistory {
  return history.activeVersionId === versionId || !history.versions.some((version) => version.id === versionId)
    ? history
    : { ...history, activeVersionId: versionId };
}

export function stageVersionChange(history: DatasetVersionHistory, change: VersionChange): DatasetVersionHistory {
  return {
    ...history,
    pendingParentId: history.pendingParentId ?? history.activeVersionId,
    pendingChanges: [...history.pendingChanges, change]
  };
}

export function finalizeVersion(history: DatasetVersionHistory, snapshot: SnapshotInput): DatasetVersionHistory {
  if (!history.pendingChanges.length) return history;
  const number = history.versions[history.versions.length - 1].number + 1;
  const id = `v${number}`;
  const version: Version = {
    id,
    parentId: history.pendingParentId ?? history.activeVersionId,
    number,
    nodeId: history.nodeId,
    dataset: history.dataset,
    sql: snapshot.sql,
    columns: [...snapshot.columns],
    hiddenColumns: [...snapshot.hiddenColumns],
    timestamp: snapshot.timestamp,
    changes: [...history.pendingChanges],
    ...(snapshot.join ? { join: copyJoin(snapshot.join) } : {})
  };
  return {
    ...history,
    versions: [...history.versions, version],
    activeVersionId: id,
    pendingParentId: null,
    pendingChanges: []
  };
}

export function versionRestoreMetadata(version: Version): VersionRestoreMetadata {
  return restoreMetadata(version);
}

export function versionDiff(history: DatasetVersionHistory, versionId: string): VersionDiff | null {
  const version = history.versions.find((item) => item.id === versionId);
  const parent = version?.parentId && history.versions.find((item) => item.id === version.parentId);
  return version && parent ? {
    parentId: parent.id,
    versionId: version.id,
    before: restoreMetadata(parent),
    after: restoreMetadata(version),
    changes: [...version.changes]
  } : null;
}

export function createView(history: DatasetVersionHistory, input: ViewInput): DatasetVersionHistory {
  const view: View = {
    ...input,
    nodeId: history.nodeId,
    dataset: history.dataset,
    ...(input.join ? { join: copyJoin(input.join) } : {})
  };
  return { ...history, views: [...history.views, view] };
}

export function matchColumnsByRegex(columns: string[], pattern: string): { matches: string[]; error: string } {
  try {
    const regex = new RegExp(pattern, 'i');
    return { matches: columns.filter((column) => regex.test(column)), error: '' };
  } catch {
    return { matches: [], error: 'Invalid regular expression.' };
  }
}

function isLegacySavedQuery(value: unknown): value is SavedQuery {
  if (!value || typeof value !== 'object') return false;
  const item = value as Record<string, unknown>;
  return ['id', 'name', 'sql', 'nodeId', 'dataset'].every((key) => typeof item[key] === 'string' && item[key].trim());
}

export function migrateSavedQueries(value: unknown, timestamp: string): View[] {
  return Array.isArray(value) ? value.filter(isLegacySavedQuery).map(({ id, name, sql, nodeId, dataset }) => ({
    id,
    name,
    sql,
    nodeId,
    dataset,
    timestamp
  })) : [];
}
