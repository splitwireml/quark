import { StandardSQL } from '@codemirror/lang-sql';
import type {
  BaseViewInfo,
  DatasetVersionHistory,
  JoinWorkspaceRequest,
  ProjectInfo,
  SavedQuery,
  Version,
  VersionChange,
  VersionDiff,
  VersionRestoreMetadata,
  View,
  ViewHistory,
  ViewKind
} from './types.ts';

export const DEFAULT_PROJECT_ID = 'default';
export const LEGACY_STORAGE_KEY = 'quark.savedQueries';
export const LEGACY_VERSIONING_STORAGE_KEY = 'quark.versioning.v1';
export const VERSIONING_STORAGE_KEY = 'quark.versioning.v2';

type SnapshotInput = Pick<Version, 'sql' | 'columns' | 'hiddenColumns' | 'timestamp' | 'join'>;
type SourceInput = SnapshotInput & Pick<Version, 'nodeId' | 'dataset'> & Partial<Pick<ViewHistory, 'id' | 'projectId' | 'name' | 'sourceId'>>;
type ViewInput = SnapshotInput & Pick<Version, 'nodeId' | 'dataset'> & Pick<ViewHistory, 'id' | 'projectId' | 'name'> & Partial<Pick<ViewHistory, 'sourceId'>>;

function copyJoin(join: JoinWorkspaceRequest): JoinWorkspaceRequest {
  const side = (reference: JoinWorkspaceRequest['left']): JoinWorkspaceRequest['left'] => reference.sql !== undefined
    ? { node_id: reference.node_id, sql: reference.sql, ...(reference.name !== undefined ? { name: reference.name } : {}) }
    : { node_id: reference.node_id, dataset: reference.dataset };
  return {
    left: side(join.left),
    right: side(join.right),
    left_keys: [...join.left_keys],
    right_keys: [...join.right_keys]
  };
}

function copyChange(change: VersionChange): VersionChange {
  return {
    kind: change.kind,
    summary: change.summary,
    ...(change.details ? { details: structuredClone(change.details) } : {})
  };
}

function copyVersion(version: Version): Version {
  return {
    id: version.id,
    ...(version.parentId ? { parentId: version.parentId } : {}),
    number: version.number,
    ...(version.fork ? { fork: version.fork } : {}),
    nodeId: version.nodeId,
    dataset: version.dataset,
    sql: version.sql,
    columns: [...version.columns],
    hiddenColumns: [...version.hiddenColumns],
    timestamp: version.timestamp,
    changes: version.changes.map(copyChange),
    ...(version.join ? { join: copyJoin(version.join) } : {})
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

function createHistory(input: ViewInput, kind: ViewKind): ViewHistory {
  return {
    id: input.id,
    projectId: input.projectId,
    name: input.name,
    kind,
    ...(input.sourceId ? { sourceId: input.sourceId } : {}),
    nodeId: input.nodeId,
    dataset: input.dataset,
    versions: [{
      id: 'v1',
      number: 1,
      nodeId: input.nodeId,
      dataset: input.dataset,
      sql: input.sql,
      columns: [...input.columns],
      hiddenColumns: [...input.hiddenColumns],
      timestamp: input.timestamp,
      changes: [],
      ...(input.join ? { join: copyJoin(input.join) } : {})
    }],
    activeVersionId: 'v1',
    pendingParentId: null,
    pendingChanges: []
  };
}

export function createSourceHistory(source: SourceInput): ViewHistory {
  return createHistory({
    ...source,
    id: source.id ?? `${source.nodeId}:${source.dataset}`,
    projectId: source.projectId ?? DEFAULT_PROJECT_ID,
    name: source.name ?? source.dataset
  }, 'source');
}

export function createView(input: ViewInput): ViewHistory {
  return createHistory(input, 'derived');
}

export function activateVersion(history: ViewHistory, versionId: string): ViewHistory {
  return history.activeVersionId === versionId || !history.versions.some((version) => version.id === versionId)
    ? history
    : { ...history, activeVersionId: versionId };
}

export function stageVersionChange(history: ViewHistory, change: VersionChange): ViewHistory {
  return {
    ...history,
    pendingParentId: history.pendingParentId ?? history.activeVersionId,
    pendingChanges: [...history.pendingChanges, copyChange(change)]
  };
}

function sameSnapshot(version: Version, snapshot: SnapshotInput): boolean {
  const value = (item: Pick<Version, 'sql' | 'columns' | 'hiddenColumns' | 'join'>) => JSON.stringify([
    item.sql, item.columns, item.hiddenColumns, item.join ? copyJoin(item.join) : null
  ]);
  return value(version) === value(snapshot);
}

export function versionLabel(version: Pick<Version, 'number' | 'fork'>): string {
  return `${version.fork ? `f${version.fork} ` : ''}v${version.number}`;
}

export function finalizeVersion(history: ViewHistory, snapshot: SnapshotInput): ViewHistory {
  if (!history.pendingChanges.length) return history;
  const parent = history.versions.find((version) => version.id === (history.pendingParentId ?? history.activeVersionId));
  if (!parent) return history;
  const children = history.versions.filter((version) => version.parentId === parent.id);
  const existing = children.find((version) => sameSnapshot(version, snapshot));
  if (existing) return { ...history, activeVersionId: existing.id, pendingParentId: null, pendingChanges: [] };
  const number = parent.number + 1;
  const fork = children.length ? Math.max(1, ...history.versions.map((version) => version.fork ?? 1)) + 1 : parent.fork;
  const id = fork ? `f${fork}-v${number}` : `v${number}`;
  const version: Version = {
    id,
    parentId: parent.id,
    number,
    ...(fork ? { fork } : {}),
    nodeId: history.nodeId,
    dataset: history.dataset,
    sql: snapshot.sql,
    columns: [...snapshot.columns],
    hiddenColumns: [...snapshot.hiddenColumns],
    timestamp: snapshot.timestamp,
    changes: history.pendingChanges.map(copyChange),
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

export function versionDiff(history: ViewHistory, versionId: string): VersionDiff | null {
  const version = history.versions.find((item) => item.id === versionId);
  const parent = version?.parentId && history.versions.find((item) => item.id === version.parentId);
  return version && parent ? {
    parentId: parent.id,
    versionId: version.id,
    before: restoreMetadata(parent),
    after: restoreMetadata(version),
    changes: version.changes.map(copyChange)
  } : null;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === 'object' && !Array.isArray(value);
}

function isNonEmptyString(value: unknown): value is string {
  return typeof value === 'string' && Boolean(value.trim());
}

function isStringArray(value: unknown): value is string[] {
  return Array.isArray(value) && value.every((item) => typeof item === 'string');
}

function isVersionChange(value: unknown): value is VersionChange {
  return isRecord(value) && typeof value.kind === 'string' && typeof value.summary === 'string'
    && (value.details === undefined || isRecord(value.details));
}

function isJoin(value: unknown): value is JoinWorkspaceRequest {
  if (!isRecord(value) || !isRecord(value.left) || !isRecord(value.right)) return false;
  const side = (item: Record<string, unknown>) => isNonEmptyString(item.node_id)
    && (item.sql === undefined
      ? isNonEmptyString(item.dataset)
      : isNonEmptyString(item.sql) && (item.name === undefined || typeof item.name === 'string'));
  return side(value.left) && side(value.right) && isStringArray(value.left_keys) && isStringArray(value.right_keys);
}

function isVersion(value: unknown): value is Version {
  return isRecord(value)
    && ['id', 'nodeId', 'dataset', 'sql', 'timestamp'].every((key) => isNonEmptyString(value[key]))
    && typeof value.number === 'number' && Number.isFinite(value.number)
    && (value.fork === undefined || typeof value.fork === 'number' && Number.isInteger(value.fork) && value.fork >= 2)
    && (value.parentId === undefined || typeof value.parentId === 'string')
    && isStringArray(value.columns) && isStringArray(value.hiddenColumns)
    && Array.isArray(value.changes) && value.changes.every(isVersionChange)
    && (value.join === undefined || isJoin(value.join));
}

function isView(value: unknown): value is View & Partial<Pick<Version, 'columns' | 'hiddenColumns'>> {
  return isRecord(value)
    && ['id', 'name', 'nodeId', 'dataset', 'sql', 'timestamp'].every((key) => isNonEmptyString(value[key]))
    && (value.columns === undefined || isStringArray(value.columns))
    && (value.hiddenColumns === undefined || isStringArray(value.hiddenColumns))
    && (value.join === undefined || isJoin(value.join));
}

export function migrateDatasetHistories(value: unknown, defaultProjectId = DEFAULT_PROJECT_ID): ViewHistory[] {
  const migrated: ViewHistory[] = [];
  const ids = new Set<string>();
  if (!Array.isArray(value)) return migrated;
  for (const item of value) {
    if (!isRecord(item) || !isNonEmptyString(item.nodeId) || !isNonEmptyString(item.dataset)
      || !Array.isArray(item.versions) || !Array.isArray(item.views)) continue;
    const history = item as unknown as DatasetVersionHistory;
    const metadata = history as DatasetVersionHistory & Partial<Pick<ViewHistory, 'id' | 'projectId' | 'name' | 'sourceId'>>;
    const id = isNonEmptyString(metadata.id) ? metadata.id : `${history.nodeId}:${history.dataset}`;
    const projectId = isNonEmptyString(metadata.projectId) ? metadata.projectId : defaultProjectId;
    const sourceId = isNonEmptyString(metadata.sourceId) ? metadata.sourceId : id;
    const versions: Version[] = [];
    for (const version of history.versions) {
      if (!isVersion(version)) continue;
      try { versions.push(copyVersion(version)); } catch { /* skip malformed persisted entry */ }
    }
    if (!versions.length) continue;
    const active = versions.find((version) => version.id === history.activeVersionId) ?? versions[versions.length - 1];
    const pendingChanges: VersionChange[] = [];
    if (Array.isArray(history.pendingChanges)) {
      for (const change of history.pendingChanges) {
        if (!isVersionChange(change)) continue;
        try { pendingChanges.push(copyChange(change)); } catch { /* skip malformed persisted entry */ }
      }
    }
    const base: ViewHistory = {
      id,
      projectId,
      name: isNonEmptyString(metadata.name) ? metadata.name : history.dataset,
      kind: 'source',
      ...(isNonEmptyString(metadata.sourceId) ? { sourceId: metadata.sourceId } : {}),
      nodeId: history.nodeId,
      dataset: history.dataset,
      versions,
      activeVersionId: active.id,
      pendingParentId: versions.some((version) => version.id === history.pendingParentId) ? history.pendingParentId : null,
      pendingChanges
    };
    const views = history.views.filter(isView).map((view) => {
      return createView({
        id: view.id,
        projectId,
        name: view.name,
        sourceId,
        nodeId: view.nodeId,
        dataset: view.dataset,
        sql: view.sql,
        columns: view.columns ?? active.columns,
        hiddenColumns: view.hiddenColumns ?? active.hiddenColumns,
        timestamp: view.timestamp,
        ...(view.join ? { join: view.join } : {})
      });
    });
    for (const entry of [base, ...views]) {
      if (ids.has(entry.id)) continue;
      ids.add(entry.id);
      migrated.push(entry);
    }
  }
  return migrated;
}

type LegacyBinding = { base: BaseViewInfo; schema: string; name: string; relation: string };
type CteScope = { name: string; bodyStart: number; bodyEnd: number; scopeEnd: number; recursive: boolean };

function decodeDatasetIdentity(dataset: string): [string, string] | null {
  try {
    if (dataset.length % 4 === 1) return null;
    const encoded = dataset.replace(/-/g, '+').replace(/_/g, '/') + '='.repeat((4 - dataset.length % 4) % 4);
    const bytes = Uint8Array.from(atob(encoded), (character) => character.charCodeAt(0));
    const value: unknown = JSON.parse(new TextDecoder().decode(bytes));
    return Array.isArray(value) && value.length === 2 && value.every((item) => typeof item === 'string')
      ? value as [string, string]
      : null;
  } catch {
    return null;
  }
}

function identifierParts(value: string): string[] {
  const parts: string[] = [];
  const pattern = /"((?:[^"]|"")*)"|([A-Za-z_][\w$]*)/g;
  let match: RegExpExecArray | null;
  while ((match = pattern.exec(value))) parts.push((match[1]?.replace(/""/g, '"') ?? match[2]).toLowerCase());
  return parts;
}

function executableRelation(sql: string): string {
  return sql.trim().match(/^SELECT\s+\*\s+FROM\s+(.+?)\s*;?$/i)?.[1] ?? `(${sql.trim().replace(/;\s*$/, '')})`;
}

function cteScopes(sql: string): CteScope[] {
  const scopes: CteScope[] = [];
  const text = (node: { from: number; to: number }) => sql.slice(node.from, node.to);
  const walk = (node: ReturnType<typeof StandardSQL.language.parser.parse>['topNode']) => {
    const children = [];
    for (let child = node.firstChild; child; child = child.nextSibling) children.push(child);
    const withIndex = children.findIndex((child) => child.name === 'Keyword' && text(child).toUpperCase() === 'WITH');
    if (withIndex >= 0) {
      let index = withIndex + 1;
      while (children[index]?.name.endsWith('Comment')) index++;
      const recursive = children[index]?.name === 'Keyword' && text(children[index]).toUpperCase() === 'RECURSIVE';
      if (recursive) index++;
      while (index < children.length) {
        while (children[index]?.name.endsWith('Comment')) index++;
        const nameNode = children[index];
        if (!nameNode || nameNode.name !== 'Identifier' && nameNode.name !== 'QuotedIdentifier') break;
        const name = text(nameNode).replace(/^"|"$/g, '').replace(/""/g, '"').toLowerCase();
        while (index < children.length && !(children[index].name === 'Keyword' && text(children[index]).toUpperCase() === 'AS')) index++;
        if (index >= children.length) break;
        index++;
        while (index < children.length && children[index].name !== 'Parens') index++;
        const body = children[index];
        if (!body) break;
        scopes.push({ name, bodyStart: body.from + 1, bodyEnd: body.to - 1, scopeEnd: node.to, recursive });
        index++;
        while (children[index]?.name.endsWith('Comment')) index++;
        if (children[index]?.name !== 'Punctuation' || text(children[index]) !== ',') break;
        index++;
      }
    }
    for (const child of children) if (child.name === 'Statement' || child.name === 'Parens') walk(child);
  };
  walk(StandardSQL.language.parser.parse(sql).topNode);
  return scopes;
}

function relationRanges(sql: string): { from: number; to: number }[] {
  const ranges: { from: number; to: number }[] = [];
  const cursor = StandardSQL.language.parser.parse(sql).cursor();
  let expectingRelation = false;
  do {
    if (cursor.name === 'Keyword') {
      const keyword = sql.slice(cursor.from, cursor.to).toUpperCase();
      if (keyword === 'FROM' || keyword === 'JOIN') expectingRelation = true;
      else if (expectingRelation && keyword !== 'LATERAL' && keyword !== 'ONLY') expectingRelation = false;
    } else if (expectingRelation) {
      if (cursor.name.endsWith('Comment')) continue;
      if (cursor.name === 'CompositeIdentifier' || cursor.name === 'Identifier' || cursor.name === 'QuotedIdentifier') {
        ranges.push({ from: cursor.from, to: cursor.to });
      }
      expectingRelation = false;
    }
  } while (cursor.next());
  return ranges;
}

// ponytail: parser-backed FROM/JOIN rewrites cover generated/common legacy SQL; expand only if comma joins need migration.
function rebindRelationSql(sql: string, bindings: LegacyBinding[], aliases: Map<string, Set<string>>): string {
  const scopes = cteScopes(sql);
  const replacements: { from: number; to: number; value: string }[] = [];
  for (const range of relationRanges(sql)) {
    const parts = identifierParts(sql.slice(range.from, range.to));
    let candidates: LegacyBinding[] = [];
    if (parts.length === 2) {
      candidates = bindings.filter((binding) => {
        const schemas = [binding.schema, ...(aliases.get(binding.base.id) ?? [])].map((schema) => schema.toLowerCase());
        return schemas.includes(parts[0]) && binding.name.toLowerCase() === parts[1];
      });
    } else if (parts.length === 1) {
      candidates = bindings.filter((binding) => binding.name.toLowerCase() === parts[0]);
    }
    if (candidates.length !== 1) continue;
    const binding = candidates[0];
    if (parts.length === 1 && scopes.some((scope) =>
      scope.name === parts[0] && (range.from > scope.bodyEnd && range.from < scope.scopeEnd
        || scope.recursive && range.from >= scope.bodyStart && range.from < scope.bodyEnd)
    )) continue;
    replacements.push({ ...range, value: binding.relation });
  }
  return replacements.sort((left, right) => right.from - left.from)
    .reduce((value, replacement) => value.slice(0, replacement.from) + replacement.value + value.slice(replacement.to), sql);
}

export function rebindLegacyHistories(histories: ViewHistory[], baseViews: BaseViewInfo[], project: ProjectInfo): ViewHistory[] {
  if (project.id !== DEFAULT_PROJECT_ID) return histories;
  const bindings = baseViews.filter((base) => base.project_id === project.id).map((base) => ({
    base,
    schema: base.schema,
    name: base.name,
    relation: executableRelation(base.sql)
  }));
  const byIdentity = new Map(bindings.map((binding) => [JSON.stringify([binding.base.source_id, binding.schema, binding.name]), binding]));
  const resolve = (nodeId: string, dataset: string): LegacyBinding | undefined => {
    const identity = decodeDatasetIdentity(dataset);
    return identity ? byIdentity.get(JSON.stringify([nodeId, ...identity])) : undefined;
  };
  const sourceBindings = new Map<string, LegacyBinding>();
  for (const history of histories) {
    if (history.projectId !== project.id || history.kind !== 'source' || history.nodeId === project.node_id) continue;
    const binding = resolve(history.nodeId, history.dataset);
    if (binding) sourceBindings.set(history.id, binding);
  }

  const entries: { history: ViewHistory; changed: boolean }[] = histories.map((history) => {
    if (history.projectId !== project.id || history.nodeId === project.node_id) return { history, changed: false };
    let primary = history.kind === 'source' ? resolve(history.nodeId, history.dataset) : history.sourceId ? sourceBindings.get(history.sourceId) : undefined;
    primary ??= resolve(history.nodeId, history.dataset);
    const joinBindings = new Set<LegacyBinding>();
    for (const version of history.versions) {
      for (const reference of version.join ? [version.join.left, version.join.right] : []) {
        if (reference.dataset === undefined) continue;
        const binding = resolve(reference.node_id, reference.dataset);
        if (binding) joinBindings.add(binding);
      }
    }
    primary ??= joinBindings.values().next().value;
    if (!primary) return { history, changed: false };
    const sourceIds = new Set([primary.base.source_id, ...[...joinBindings].map((binding) => binding.base.source_id)]);
    const relationBindings = bindings.filter((binding) => sourceIds.has(binding.base.source_id));
    const versions = history.versions.map((version) => {
      const aliases = new Map<string, Set<string>>();
      const side = (reference: JoinWorkspaceRequest['left'], alias: string): JoinWorkspaceRequest['left'] => {
        if (reference.dataset === undefined) return {
          node_id: reference.node_id,
          sql: reference.sql,
          ...(reference.name !== undefined ? { name: reference.name } : {})
        };
        const binding = resolve(reference.node_id, reference.dataset);
        if (!binding) return { node_id: reference.node_id, dataset: reference.dataset };
        const names = aliases.get(binding.base.id) ?? new Set<string>();
        names.add(alias);
        aliases.set(binding.base.id, names);
        return { node_id: project.node_id, sql: binding.base.sql, name: binding.base.name };
      };
      const join = version.join ? {
        left: side(version.join.left, 'left_source'),
        right: side(version.join.right, 'right_source'),
        left_keys: [...version.join.left_keys],
        right_keys: [...version.join.right_keys]
      } : undefined;
      return {
        ...copyVersion(version),
        nodeId: project.node_id,
        dataset: primary.base.name,
        sql: rebindRelationSql(version.sql, relationBindings, aliases),
        ...(join ? { join } : {})
      };
    });
    return {
      changed: true,
      history: {
        id: history.kind === 'source' ? primary.base.id : history.id,
        projectId: history.projectId,
        name: history.kind === 'source' ? primary.base.name : history.name,
        kind: history.kind,
        sourceId: primary.base.source_id,
        nodeId: project.node_id,
        dataset: primary.base.name,
        versions,
        activeVersionId: history.activeVersionId,
        pendingParentId: history.pendingParentId,
        pendingChanges: history.pendingChanges.map(copyChange)
      }
    };
  });
  if (!entries.some((entry) => entry.changed)) return histories;
  const rebound: typeof entries = [];
  const ids = new Map<string, number>();
  for (const entry of entries) {
    const existing = ids.get(entry.history.id);
    if (existing === undefined) {
      ids.set(entry.history.id, rebound.length);
      rebound.push(entry);
    } else if (entry.changed && !rebound[existing].changed) {
      rebound[existing] = entry;
    }
  }
  return rebound.map((entry) => entry.history);
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

export function migrateSavedQueries(value: unknown, timestamp: string, projectId = DEFAULT_PROJECT_ID): ViewHistory[] {
  return Array.isArray(value) ? value.filter(isLegacySavedQuery).map(({ id, name, sql, nodeId, dataset }) => createView({
    id,
    projectId,
    name,
    sql,
    nodeId,
    dataset,
    columns: [],
    hiddenColumns: [],
    timestamp
  })) : [];
}
