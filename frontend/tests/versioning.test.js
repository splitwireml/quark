import assert from 'node:assert/strict';
import test from 'node:test';
import {
  LEGACY_STORAGE_KEY,
  VERSIONING_STORAGE_KEY,
  activateVersion,
  createSourceHistory,
  createView,
  finalizeVersion,
  matchColumnsByRegex,
  migrateSavedQueries,
  stageVersionChange,
  versionDiff,
  versionRestoreMetadata
} from '../src/lib/versioning.ts';

const source = {
  nodeId: 'node-1',
  dataset: 'cars',
  sql: 'SELECT * FROM cars',
  columns: ['make', 'model', 'year'],
  hiddenColumns: ['year'],
  timestamp: '2026-09-02T08:00:00.000Z'
};
const join = {
  left: { node_id: 'left-node', dataset: 'cars' },
  right: { node_id: 'right-node', dataset: 'makers' },
  left_keys: ['make'],
  right_keys: ['name']
};

test('creates source version 1 with metadata and no row data', () => {
  const history = createSourceHistory({ ...source, rows: [{ make: 'BMW' }] });

  assert.deepEqual(history, {
    nodeId: 'node-1',
    dataset: 'cars',
    versions: [{ ...source, id: 'v1', number: 1, changes: [] }],
    views: [],
    activeVersionId: 'v1',
    pendingParentId: null,
    pendingChanges: []
  });
  assert.equal(JSON.stringify(history).includes('rows'), false);
});

test('finalizes multiple staged changes as one sequential version', () => {
  const v1 = createSourceHistory(source);
  const staged = stageVersionChange(
    stageVersionChange(v1, { kind: 'rename', summary: 'Rename make', details: { from: 'make', to: 'brand' } }),
    { kind: 'hide', summary: 'Hide year', details: { columns: ['year'] } }
  );
  const history = finalizeVersion(staged, {
    sql: 'SELECT make AS brand, model, year FROM cars',
    columns: ['brand', 'model', 'year'],
    hiddenColumns: ['year'],
    timestamp: '2026-09-02T08:05:00.000Z',
    join
  });

  assert.equal(history.versions.length, 2);
  assert.equal(history.versions[1].number, 2);
  assert.equal(history.versions[1].parentId, 'v1');
  assert.deepEqual(history.versions[1].changes, staged.pendingChanges);
  assert.deepEqual(history.pendingChanges, []);
  assert.deepEqual(v1.pendingChanges, []);
  assert.deepEqual(JSON.parse(JSON.stringify(history)).versions[1].join, join);
});

test('does nothing when finalizing without staged changes', () => {
  const history = createSourceHistory(source);
  assert.strictEqual(finalizeVersion(history, source), history);
});

test('uses the recording parent for ancestry and diffs after restoring an older version', () => {
  const v1 = createSourceHistory(source);
  const v2 = finalizeVersion(stageVersionChange(v1, { kind: 'rename', summary: 'Rename make' }), {
    sql: 'SELECT make AS brand, model, year FROM cars',
    columns: ['brand', 'model', 'year'],
    hiddenColumns: ['year'],
    timestamp: '2026-09-02T08:05:00.000Z'
  });
  const restored = activateVersion(v2, 'v1');
  const v3 = finalizeVersion(stageVersionChange(restored, { kind: 'hide', summary: 'Hide model' }), {
    sql: source.sql,
    columns: source.columns,
    hiddenColumns: ['model', 'year'],
    timestamp: '2026-09-02T08:10:00.000Z'
  });

  assert.equal(v3.versions[2].parentId, 'v1');
  assert.deepEqual(versionDiff(v3, 'v3'), {
    parentId: 'v1',
    versionId: 'v3',
    before: versionRestoreMetadata(v3.versions[0]),
    after: versionRestoreMetadata(v3.versions[2]),
    changes: v3.versions[2].changes
  });
});

test('returns deterministic ordered restore metadata', () => {
  const version = createSourceHistory(source).versions[0];

  assert.deepEqual(versionRestoreMetadata(version), {
    nodeId: 'node-1',
    dataset: 'cars',
    sql: 'SELECT * FROM cars',
    columns: ['make', 'model', 'year'],
    hiddenColumns: ['year']
  });
});

test('creates a view without incrementing version history', () => {
  const history = createView(createSourceHistory(source), {
    id: 'view-1',
    name: 'German cars',
    sql: "SELECT * FROM cars WHERE country = 'DE'",
    timestamp: '2026-09-02T08:10:00.000Z',
    join,
    rows: [{ make: 'BMW' }]
  });

  assert.equal(history.versions.length, 1);
  assert.deepEqual(history.views, [{
    id: 'view-1',
    name: 'German cars',
    sql: "SELECT * FROM cars WHERE country = 'DE'",
    nodeId: 'node-1',
    dataset: 'cars',
    timestamp: '2026-09-02T08:10:00.000Z',
    join
  }]);
});

test('matches columns by regex and reports invalid patterns', () => {
  assert.deepEqual(matchColumnsByRegex(['Make', 'model', 'year'], '^m'), { matches: ['Make', 'model'], error: '' });
  assert.deepEqual(matchColumnsByRegex(['Make'], '['), { matches: [], error: 'Invalid regular expression.' });
});

test('migrates only valid legacy saved queries into views', () => {
  assert.equal(LEGACY_STORAGE_KEY, 'quark.savedQueries');
  assert.equal(VERSIONING_STORAGE_KEY, 'quark.versioning.v1');
  assert.deepEqual(migrateSavedQueries([
    { id: 'saved-1', name: 'Recent cars', sql: 'SELECT * FROM cars', nodeId: 'node-1', dataset: 'cars', rows: [{ make: 'BMW' }] },
    { id: 'broken', name: 'Missing dataset', sql: 'SELECT 1', nodeId: 'node-1' },
    null
  ], '2026-09-02T08:15:00.000Z'), [{
    id: 'saved-1',
    name: 'Recent cars',
    sql: 'SELECT * FROM cars',
    nodeId: 'node-1',
    dataset: 'cars',
    timestamp: '2026-09-02T08:15:00.000Z'
  }]);
});

test('strips runtime payload fields from joins and staged changes', () => {
  const unsafeJoin = {
    ...join,
    left: { ...join.left, rows: [{ make: 'BMW' }] },
    right: { ...join.right, rows: [{ name: 'BMW' }] }
  };
  const history = stageVersionChange(createSourceHistory({ ...source, join: unsafeJoin }), {
    kind: 'join',
    summary: 'Join makers',
    rows: [{ make: 'BMW' }]
  });

  assert.deepEqual(history.versions[0].join, join);
  assert.deepEqual(history.pendingChanges, [{ kind: 'join', summary: 'Join makers' }]);
});
