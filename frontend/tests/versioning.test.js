import assert from 'node:assert/strict';
import test from 'node:test';
import {
  DEFAULT_PROJECT_ID,
  LEGACY_STORAGE_KEY,
  LEGACY_VERSIONING_STORAGE_KEY,
  VERSIONING_STORAGE_KEY,
  activateVersion,
  createSourceHistory,
  createView,
  finalizeVersion,
  matchColumnsByRegex,
  migrateDatasetHistories,
  migrateSavedQueries,
  rebindLegacyHistories,
  stageVersionChange,
  versionDiff,
  versionLabel,
  versionRestoreMetadata
} from '../src/lib/versioning.ts';

const source = {
  id: 'view-cars',
  projectId: 'project-1',
  name: 'Cars',
  sourceId: 'source-1',
  nodeId: 'node-1',
  dataset: 'cars',
  sql: 'SELECT * FROM cars',
  columns: ['make', 'model', 'year'],
  hiddenColumns: ['year'],
  timestamp: '2026-09-02T08:00:00.000Z'
};
const join = {
  left: { node_id: 'node-1', sql: 'SELECT * FROM cars', name: 'Cars' },
  right: { node_id: 'node-1', sql: 'SELECT * FROM makers', name: 'Makers' },
  left_keys: ['make'],
  right_keys: ['name']
};

function derived(overrides = {}) {
  return createView({
    id: 'view-german-cars',
    projectId: 'project-1',
    name: 'German cars',
    sourceId: 'source-1',
    nodeId: 'node-1',
    dataset: 'cars',
    sql: "SELECT * FROM cars WHERE country = 'DE'",
    columns: ['make', 'model'],
    hiddenColumns: [],
    timestamp: '2026-09-02T08:10:00.000Z',
    join,
    rows: [{ make: 'BMW' }],
    ...overrides
  });
}

test('creates every source and derived View at Version 1 without row data', () => {
  const base = createSourceHistory({ ...source, rows: [{ make: 'BMW' }] });
  const view = derived();

  assert.deepEqual(base, {
    id: 'view-cars',
    projectId: 'project-1',
    name: 'Cars',
    kind: 'source',
    sourceId: 'source-1',
    nodeId: 'node-1',
    dataset: 'cars',
    versions: [{
      id: 'v1',
      number: 1,
      nodeId: 'node-1',
      dataset: 'cars',
      sql: 'SELECT * FROM cars',
      columns: ['make', 'model', 'year'],
      hiddenColumns: ['year'],
      timestamp: '2026-09-02T08:00:00.000Z',
      changes: []
    }],
    activeVersionId: 'v1',
    pendingParentId: null,
    pendingChanges: []
  });
  assert.equal(view.kind, 'derived');
  assert.equal(view.versions.length, 1);
  assert.equal(view.versions[0].number, 1);
  assert.equal(JSON.stringify([base, view]).includes('rows'), false);
});

test('finalizes changes only in the selected View history', () => {
  const base = createSourceHistory(source);
  const view = derived();
  const finalized = finalizeVersion(stageVersionChange(view, { kind: 'hide', summary: 'Hide model' }), {
    sql: view.versions[0].sql,
    columns: ['make', 'model'],
    hiddenColumns: ['model'],
    timestamp: '2026-09-02T08:20:00.000Z'
  });

  assert.equal(base.versions.length, 1);
  assert.equal(view.versions.length, 1);
  assert.equal(finalized.id, view.id);
  assert.equal(finalized.versions.length, 2);
  assert.equal(finalized.versions[1].parentId, 'v1');
  assert.equal(finalized.versions[1].number, 2);
});

test('creates numbered sibling forks from the restored parent and diffs against that parent', () => {
  const v1 = createSourceHistory(source);
  const v2 = finalizeVersion(stageVersionChange(v1, { kind: 'rename', summary: 'Rename make' }), {
    sql: 'SELECT make AS brand, model, year FROM cars',
    columns: ['brand', 'model', 'year'],
    hiddenColumns: ['year'],
    timestamp: '2026-09-02T08:05:00.000Z'
  });
  const f2 = finalizeVersion(stageVersionChange(activateVersion(v2, 'v1'), { kind: 'hide', summary: 'Hide model' }), {
    sql: source.sql,
    columns: source.columns,
    hiddenColumns: ['model', 'year'],
    timestamp: '2026-09-02T08:10:00.000Z'
  });
  const f3 = finalizeVersion(stageVersionChange(activateVersion(f2, 'v1'), { kind: 'sort', summary: 'Sort model' }), {
    sql: `${source.sql} ORDER BY model`,
    columns: source.columns,
    hiddenColumns: source.hiddenColumns,
    timestamp: '2026-09-02T08:15:00.000Z'
  });

  assert.deepEqual(f3.versions.map(({ id, parentId, number, fork }) => ({ id, parentId, number, fork })), [
    { id: 'v1', parentId: undefined, number: 1, fork: undefined },
    { id: 'v2', parentId: 'v1', number: 2, fork: undefined },
    { id: 'f2-v2', parentId: 'v1', number: 2, fork: 2 },
    { id: 'f3-v2', parentId: 'v1', number: 2, fork: 3 }
  ]);
  assert.equal(new Set(f3.versions.map(({ id }) => id)).size, f3.versions.length);
  assert.deepEqual(versionDiff(f3, 'f2-v2'), {
    parentId: 'v1',
    versionId: 'f2-v2',
    before: versionRestoreMetadata(f3.versions[0]),
    after: versionRestoreMetadata(f3.versions[2]),
    changes: f3.versions[2].changes
  });
});

test('reuses a matching direct child snapshot and clears pending changes', () => {
  const v1 = createSourceHistory(source);
  const v2 = finalizeVersion(stageVersionChange(v1, { kind: 'rename', summary: 'Rename make' }), {
    sql: 'SELECT make AS brand, model, year FROM cars',
    columns: ['brand', 'model', 'year'],
    hiddenColumns: ['year'],
    timestamp: '2026-09-02T08:05:00.000Z',
    join
  });
  const pending = stageVersionChange(activateVersion(v2, 'v1'), { kind: 'sql', summary: 'Different wording' });
  const reused = finalizeVersion(pending, {
    ...v2.versions[1],
    timestamp: '2026-09-02T09:00:00.000Z'
  });

  assert.equal(reused.versions.length, 2);
  assert.equal(reused.activeVersionId, 'v2');
  assert.equal(reused.pendingParentId, null);
  assert.deepEqual(reused.pendingChanges, []);
});

test('continues an existing fork with the parent fork and strict next number', () => {
  const v1 = createSourceHistory(source);
  const v2 = finalizeVersion(stageVersionChange(v1, { kind: 'rename', summary: 'Rename make' }), {
    ...source,
    sql: 'SELECT make AS brand, model, year FROM cars',
    columns: ['brand', 'model', 'year']
  });
  const f2v2 = finalizeVersion(stageVersionChange(activateVersion(v2, 'v1'), { kind: 'hide', summary: 'Hide model' }), {
    ...source,
    hiddenColumns: ['model', 'year']
  });
  const f2v3 = finalizeVersion(stageVersionChange(f2v2, { kind: 'sort', summary: 'Sort model' }), {
    ...source,
    sql: `${source.sql} ORDER BY model`,
    hiddenColumns: ['model', 'year']
  });
  const child = f2v3.versions.find(({ id }) => id === f2v3.activeVersionId);

  assert.deepEqual(child && { id: child.id, parentId: child.parentId, number: child.number, fork: child.fork }, {
    id: 'f2-v3', parentId: 'f2-v2', number: 3, fork: 2
  });
});

test('formats linear and forked version labels', () => {
  assert.equal(versionLabel({ number: 1 }), 'v1');
  assert.equal(versionLabel({ number: 3 }), 'v3');
  assert.equal(versionLabel({ number: 2, fork: 2 }), 'f2 v2');
});

test('does nothing when finalizing without staged changes', () => {
  const history = createSourceHistory(source);
  assert.strictEqual(finalizeVersion(history, source), history);
});

test('flattens legacy base histories and nested saved Views', () => {
  const legacyV1 = {
    id: 'v1', number: 1, nodeId: 'node-1', dataset: 'cars', sql: 'SELECT * FROM cars',
    columns: ['make', 'model'], hiddenColumns: [], timestamp: '2026-09-01T08:00:00.000Z', changes: [], rows: [{ make: 'BMW' }]
  };
  const legacyV2 = {
    ...legacyV1, id: 'v2', parentId: 'v1', number: 2, sql: 'SELECT make FROM cars', columns: ['make'],
    timestamp: '2026-09-01T09:00:00.000Z', changes: [{ kind: 'hide', summary: 'Hide model' }]
  };
  const migrated = migrateDatasetHistories([{
    nodeId: 'node-1',
    dataset: 'cars',
    versions: [legacyV1, legacyV2],
    views: [{
      id: 'saved-1', name: 'German cars', sql: "SELECT * FROM cars WHERE country = 'DE'",
      nodeId: 'node-1', dataset: 'cars', timestamp: '2026-09-01T10:00:00.000Z', join,
      rows: [{ make: 'BMW' }]
    }],
    activeVersionId: 'v2',
    pendingParentId: 'v2',
    pendingChanges: [{ kind: 'sort', summary: 'Sort make' }]
  }]);

  assert.equal(migrated.length, 2);
  assert.deepEqual(migrated[0], {
    id: 'node-1:cars',
    projectId: DEFAULT_PROJECT_ID,
    name: 'cars',
    kind: 'source',
    nodeId: 'node-1',
    dataset: 'cars',
    versions: [
      { id: 'v1', number: 1, nodeId: 'node-1', dataset: 'cars', sql: 'SELECT * FROM cars', columns: ['make', 'model'], hiddenColumns: [], timestamp: '2026-09-01T08:00:00.000Z', changes: [] },
      { id: 'v2', parentId: 'v1', number: 2, nodeId: 'node-1', dataset: 'cars', sql: 'SELECT make FROM cars', columns: ['make'], hiddenColumns: [], timestamp: '2026-09-01T09:00:00.000Z', changes: [{ kind: 'hide', summary: 'Hide model' }] }
    ],
    activeVersionId: 'v2',
    pendingParentId: 'v2',
    pendingChanges: [{ kind: 'sort', summary: 'Sort make' }]
  });
  assert.deepEqual(migrated[1], {
    id: 'saved-1',
    projectId: DEFAULT_PROJECT_ID,
    name: 'German cars',
    kind: 'derived',
    sourceId: 'node-1:cars',
    nodeId: 'node-1',
    dataset: 'cars',
    versions: [{
      id: 'v1', number: 1, nodeId: 'node-1', dataset: 'cars',
      sql: "SELECT * FROM cars WHERE country = 'DE'", columns: ['make'], hiddenColumns: [],
      timestamp: '2026-09-01T10:00:00.000Z', changes: [], join
    }],
    activeVersionId: 'v1',
    pendingParentId: null,
    pendingChanges: []
  });
  assert.equal(JSON.stringify(migrated).includes('rows'), false);
});

test('skips malformed legacy entries without blocking valid histories', () => {
  const version = {
    id: 'v1', number: 1, nodeId: 'node-2', dataset: 'trucks', sql: 'SELECT * FROM trucks',
    columns: ['make'], hiddenColumns: [], timestamp: '2026-09-01T08:00:00.000Z', changes: [], rows: [{ make: 'Volvo' }]
  };
  let migrated;

  assert.doesNotThrow(() => {
    migrated = migrateDatasetHistories([
      null,
      { nodeId: 'broken', dataset: 'broken', versions: {}, views: [], activeVersionId: 'v1', pendingParentId: null, pendingChanges: [] },
      {
        nodeId: 'node-2', dataset: 'trucks', versions: [{ ...version, id: 'bad', columns: null }, version],
        views: [null, { id: 'bad-view', name: 'Bad', nodeId: 'node-2' }, {
          id: 'saved-2', name: 'Swedish trucks', nodeId: 'node-2', dataset: 'trucks',
          sql: "SELECT * FROM trucks WHERE country = 'SE'", timestamp: '2026-09-01T09:00:00.000Z', rows: [{ make: 'Volvo' }]
        }],
        activeVersionId: 'v1', pendingParentId: null, pendingChanges: [], rows: [{ make: 'Volvo' }]
      }
    ]);
  });

  assert.deepEqual(migrated.map((history) => history.id), ['node-2:trucks', 'saved-2']);
  assert.equal(migrated[0].versions.length, 1);
  assert.equal(JSON.stringify(migrated).includes('rows'), false);
});

test('repairs dangling active and pending version pointers', () => {
  const versions = [
    { id: 'v1', number: 1, nodeId: 'node-3', dataset: 'parts', sql: 'SELECT old FROM parts', columns: ['old'], hiddenColumns: [], timestamp: '2026-09-01T08:00:00.000Z', changes: [] },
    { id: 'v2', parentId: 'v1', number: 2, nodeId: 'node-3', dataset: 'parts', sql: 'SELECT current FROM parts', columns: ['current'], hiddenColumns: [], timestamp: '2026-09-01T09:00:00.000Z', changes: [] }
  ];
  const [base, view] = migrateDatasetHistories([{
    nodeId: 'node-3', dataset: 'parts', versions,
    views: [{ id: 'saved-3', name: 'Current parts', nodeId: 'node-3', dataset: 'parts', sql: 'SELECT current FROM parts', timestamp: '2026-09-01T10:00:00.000Z' }],
    activeVersionId: 'missing', pendingParentId: 'also-missing', pendingChanges: []
  }]);

  assert.equal(base.activeVersionId, 'v2');
  assert.equal(base.pendingParentId, null);
  assert.deepEqual(view.versions[0].columns, ['current']);
});

test('keeps only the first flat history with each ID', () => {
  const history = (id, nodeId, dataset, views) => ({
    id, nodeId, dataset, views,
    versions: [{ id: 'v1', number: 1, nodeId, dataset, sql: `SELECT * FROM ${dataset}`, columns: ['id'], hiddenColumns: [], timestamp: '2026-09-01T08:00:00.000Z', changes: [] }],
    activeVersionId: 'v1', pendingParentId: null, pendingChanges: []
  });
  const view = (id, nodeId, dataset) => ({ id, name: id, nodeId, dataset, sql: `SELECT * FROM ${dataset}`, timestamp: '2026-09-01T09:00:00.000Z' });

  const migrated = migrateDatasetHistories([
    history('source-1', 'node-1', 'cars', [view('source-1', 'node-1', 'cars'), view('saved', 'node-1', 'cars')]),
    history('source-2', 'node-2', 'trucks', [view('saved', 'node-2', 'trucks'), view('saved-2', 'node-2', 'trucks')])
  ]);

  assert.deepEqual(migrated.map(({ id }) => id), ['source-1', 'saved', 'source-2', 'saved-2']);
});

test('keeps storage keys for both flat and legacy histories', () => {
  assert.equal(LEGACY_STORAGE_KEY, 'quark.savedQueries');
  assert.equal(LEGACY_VERSIONING_STORAGE_KEY, 'quark.versioning.v1');
  assert.equal(VERSIONING_STORAGE_KEY, 'quark.versioning.v2');
});

test('migrates valid legacy saved queries into flat derived histories', () => {
  const migrated = migrateSavedQueries([
    { id: 'saved-1', name: 'Recent cars', sql: 'SELECT * FROM cars', nodeId: 'node-1', dataset: 'cars', rows: [{ make: 'BMW' }] },
    { id: 'broken', name: 'Missing dataset', sql: 'SELECT 1', nodeId: 'node-1' }
  ], '2026-09-02T08:15:00.000Z');

  assert.equal(migrated.length, 1);
  assert.equal(migrated[0].kind, 'derived');
  assert.equal(migrated[0].projectId, DEFAULT_PROJECT_ID);
  assert.equal(migrated[0].versions[0].sql, 'SELECT * FROM cars');
  assert.equal(JSON.stringify(migrated).includes('rows'), false);
});

test('strips runtime payload fields while preserving SQL join references', () => {
  const unsafeJoin = {
    ...join,
    left: { ...join.left, rows: [{ make: 'BMW' }] },
    right: { ...join.right, rows: [{ name: 'BMW' }] }
  };
  const history = stageVersionChange(createSourceHistory({ ...source, join: unsafeJoin }), {
    kind: 'join', summary: 'Join makers', rows: [{ make: 'BMW' }]
  });

  assert.deepEqual(history.versions[0].join, join);
  assert.deepEqual(history.pendingChanges, [{ kind: 'join', summary: 'Join makers' }]);
});

test('does not alias staged or diff change metadata', () => {
  const change = { kind: 'hide', summary: 'Hide model', details: { columns: ['model'] } };
  const staged = stageVersionChange(createSourceHistory(source), change);
  change.details.columns[0] = 'year';
  const finalized = finalizeVersion(staged, { ...source, timestamp: '2026-09-02T08:20:00.000Z' });
  const diff = versionDiff(finalized, 'v2');
  diff.changes[0].summary = 'mutated';
  diff.changes[0].details.columns[0] = 'year';

  assert.deepEqual(finalized.versions[1].changes, [{ kind: 'hide', summary: 'Hide model', details: { columns: ['model'] } }]);
});

test('matches columns by regex and reports invalid patterns', () => {
  assert.deepEqual(matchColumnsByRegex(['Make', 'model', 'year'], '^m'), { matches: ['Make', 'model'], error: '' });
  assert.deepEqual(matchColumnsByRegex(['Make'], '['), { matches: [], error: 'Invalid regular expression.' });
});

const defaultProject = { id: 'default', name: 'Default', node_id: 'project_default', source_count: 2 };
const datasetId = (schema, name) => Buffer.from(JSON.stringify([schema, name])).toString('base64url');
const legacyVersion = (nodeId, dataset, sql, overrides = {}) => ({
  id: 'v1', number: 1, nodeId, dataset, sql, columns: ['id'], hiddenColumns: [],
  timestamp: '2026-09-01T08:00:00.000Z', changes: [], rows: [{ id: 1 }], ...overrides
});
const baseView = (id, sourceId, schema, name, sql) => ({
  id, project_id: 'default', source_id: sourceId, source_name: `${sourceId}.csv`,
  node_id: 'project_default', name, schema, type: 'VIEW', columns: ['id'], sql
});

test('rebinds a selectable legacy base history without losing versions or creating a duplicate', () => {
  const dataset = datasetId('main', 'cars');
  const history = {
    id: `source-cars:${dataset}`, projectId: 'default', name: dataset, kind: 'source',
    nodeId: 'source-cars', dataset,
    versions: [
      legacyVersion('source-cars', dataset, 'SELECT * FROM "main"."cars"'),
      legacyVersion('source-cars', dataset, 'SELECT id FROM "main"."cars" ORDER BY id', {
        id: 'v2', parentId: 'v1', number: 2, hiddenColumns: ['id'],
        timestamp: '2026-09-01T09:00:00.000Z', changes: [{ kind: 'sort', summary: 'Sort id' }]
      })
    ],
    activeVersionId: 'v2', pendingParentId: 'v1', pendingChanges: [{ kind: 'hide', summary: 'Hide id' }],
    rows: [{ id: 1 }]
  };
  const base = baseView('base-cars', 'source-cars', 'main', 'cars', 'SELECT * FROM "source_cars"."cars"');
  const duplicate = createSourceHistory({
    id: base.id, projectId: 'default', sourceId: base.source_id, name: base.name,
    nodeId: base.node_id, dataset: base.name, sql: base.sql, columns: base.columns,
    hiddenColumns: [], timestamp: '2026-09-02T08:00:00.000Z'
  });

  const rebound = rebindLegacyHistories([history, duplicate], [base], defaultProject);

  assert.deepEqual(rebound.map(({ id }) => id), ['base-cars']);
  assert.equal(rebound[0].sourceId, 'source-cars');
  assert.equal(rebound[0].name, 'cars');
  assert.equal(rebound[0].nodeId, 'project_default');
  assert.equal(rebound[0].dataset, 'cars');
  assert.equal(rebound[0].versions.length, 2);
  assert.equal(rebound[0].activeVersionId, 'v2');
  assert.equal(rebound[0].versions.find(({ id }) => id === rebound[0].activeVersionId).parentId, 'v1');
  assert.equal(rebound[0].versions[0].sql, base.sql);
  assert.equal(rebound[0].versions[1].sql, 'SELECT id FROM "source_cars"."cars" ORDER BY id');
  assert.deepEqual(rebound[0].versions[1].changes, [{ kind: 'sort', summary: 'Sort id' }]);
  assert.deepEqual(rebound[0].versions[1].hiddenColumns, ['id']);
  assert.deepEqual(rebound[0].pendingChanges, [{ kind: 'hide', summary: 'Hide id' }]);
  assert.equal(JSON.stringify(rebound).includes('rows'), false);
});

test('rebinds every nested saved SQL version to executable project relations', () => {
  const dataset = datasetId('main', 'cars');
  const sourceId = `source-cars:${dataset}`;
  const history = {
    id: 'saved-cars', projectId: 'default', name: 'Saved cars', kind: 'derived', sourceId,
    nodeId: 'source-cars', dataset,
    versions: [
      legacyVersion('source-cars', dataset, 'SELECT id FROM "main"."cars" WHERE id > 1'),
      legacyVersion('source-cars', dataset, 'SELECT count(*) FROM (SELECT id FROM cars WHERE id > 1) saved', {
        id: 'v2', parentId: 'v1', number: 2
      })
    ],
    activeVersionId: 'v2', pendingParentId: null, pendingChanges: []
  };
  const base = baseView('base-cars', 'source-cars', 'main', 'cars', 'SELECT * FROM "source_cars"."cars"');

  const [rebound] = rebindLegacyHistories([history], [base], defaultProject);

  assert.equal(rebound.id, 'saved-cars');
  assert.equal(rebound.name, 'Saved cars');
  assert.equal(rebound.sourceId, 'source-cars');
  assert.equal(rebound.versions.length, 2);
  assert.equal(rebound.versions[0].sql, 'SELECT id FROM "source_cars"."cars" WHERE id > 1');
  assert.equal(rebound.versions[1].sql, 'SELECT count(*) FROM (SELECT id FROM "source_cars"."cars" WHERE id > 1) saved');
  assert.ok(rebound.versions.every((version) => version.nodeId === 'project_default' && version.dataset === 'cars'));
});

test('rebinds cross-source join aliases and dataset lineage to project SQL references', () => {
  const ordersDataset = datasetId('sales', 'orders');
  const customersDataset = datasetId('crm', 'customers');
  const ordersBase = baseView('base-orders', 'orders-source', 'sales', 'orders', 'SELECT * FROM "source_orders"."orders"');
  const customersBase = baseView('base-customers', 'customers-source', 'crm', 'customers', 'SELECT * FROM "source_customers"."customers"');
  const history = {
    id: 'saved-join', projectId: 'default', name: 'Orders + customers', kind: 'derived',
    sourceId: `orders-source:${ordersDataset}`, nodeId: 'join-old', dataset: ordersDataset,
    versions: [legacyVersion('join-old', ordersDataset,
      'SELECT "left"."id" FROM "left_source"."orders" AS "left" JOIN "right_source"."customers" AS "right" ON "left"."id" = "right"."id"', {
        join: {
          left: { node_id: 'orders-source', dataset: ordersDataset },
          right: { node_id: 'customers-source', dataset: customersDataset },
          left_keys: ['id'], right_keys: ['id']
        }
      })],
    activeVersionId: 'v1', pendingParentId: null, pendingChanges: []
  };

  const [rebound] = rebindLegacyHistories([history], [ordersBase, customersBase], defaultProject);

  assert.equal(rebound.versions[0].sql,
    'SELECT "left"."id" FROM "source_orders"."orders" AS "left" JOIN "source_customers"."customers" AS "right" ON "left"."id" = "right"."id"');
  assert.deepEqual(rebound.versions[0].join, {
    left: { node_id: 'project_default', sql: ordersBase.sql, name: 'orders' },
    right: { node_id: 'project_default', sql: customersBase.sql, name: 'customers' },
    left_keys: ['id'], right_keys: ['id']
  });
});

test('does not replace a CTE that shadows a legacy table name', () => {
  const dataset = datasetId('main', 'cars');
  const history = {
    id: 'saved-cte', projectId: 'default', name: 'Saved CTE', kind: 'derived',
    nodeId: 'source-cars', dataset,
    versions: [legacyVersion('source-cars', dataset,
      `WITH cars(id, label) AS MATERIALIZED (SELECT id, '\"main\".\"cars\"' FROM cars WHERE id > 1) SELECT * FROM cars`)],
    activeVersionId: 'v1', pendingParentId: null, pendingChanges: []
  };
  const base = baseView('base-cars', 'source-cars', 'main', 'cars', 'SELECT * FROM "source_cars"."cars"');

  const [rebound] = rebindLegacyHistories([history], [base], defaultProject);

  assert.equal(rebound.versions[0].sql,
    `WITH cars(id, label) AS MATERIALIZED (SELECT id, '\"main\".\"cars\"' FROM \"source_cars\".\"cars\" WHERE id > 1) SELECT * FROM cars`);
});

test('respects sibling, recursive, and nested CTE scopes', () => {
  const dataset = datasetId('main', 'cars');
  const base = baseView('base-cars', 'source-cars', 'main', 'cars', 'SELECT * FROM "source_cars"."cars"');
  const relation = '"source_cars"."cars"';
  const cases = [
    [
      'SELECT * FROM /* legacy relation */ cars',
      `SELECT * FROM /* legacy relation */ ${relation}`
    ],
    [
      'SELECT * FROM -- legacy relation\n cars',
      `SELECT * FROM -- legacy relation\n ${relation}`
    ],
    [
      'WITH /* named */ cars(id) AS /* keep */ MATERIALIZED (SELECT id FROM cars) SELECT * FROM cars',
      `WITH /* named */ cars(id) AS /* keep */ MATERIALIZED (SELECT id FROM ${relation}) SELECT * FROM cars`
    ],
    [
      'WITH raw AS (SELECT id FROM cars), cars AS (SELECT id FROM raw) SELECT * FROM cars',
      `WITH raw AS (SELECT id FROM ${relation}), cars AS (SELECT id FROM raw) SELECT * FROM cars`
    ],
    [
      'WITH RECURSIVE cars(id) AS (SELECT 1 UNION ALL SELECT id + 1 FROM cars WHERE id < 3) SELECT * FROM cars',
      'WITH RECURSIVE cars(id) AS (SELECT 1 UNION ALL SELECT id + 1 FROM cars WHERE id < 3) SELECT * FROM cars'
    ],
    [
      'SELECT * FROM (WITH cars AS (SELECT id FROM cars) SELECT * FROM cars) nested JOIN cars ON true',
      `SELECT * FROM (WITH cars AS (SELECT id FROM ${relation}) SELECT * FROM cars) nested JOIN ${relation} ON true`
    ],
    [
      'WITH wrapper AS (SELECT * FROM (WITH RECURSIVE x(n) AS (SELECT 1 UNION ALL SELECT n + 1 FROM x) SELECT * FROM x)), cars AS (SELECT id FROM cars) SELECT * FROM cars',
      `WITH wrapper AS (SELECT * FROM (WITH RECURSIVE x(n) AS (SELECT 1 UNION ALL SELECT n + 1 FROM x) SELECT * FROM x)), cars AS (SELECT id FROM ${relation}) SELECT * FROM cars`
    ]
  ];
  for (const [sql, expected] of cases) {
    const history = {
      id: `saved-${sql}`, projectId: 'default', name: 'Saved CTE', kind: 'derived',
      nodeId: 'source-cars', dataset,
      versions: [legacyVersion('source-cars', dataset, sql)],
      activeVersionId: 'v1', pendingParentId: null, pendingChanges: []
    };
    assert.equal(rebindLegacyHistories([history], [base], defaultProject)[0].versions[0].sql, expected);
  }
});

test('leaves current, malformed, and unmatched histories untouched while rebinding valid siblings', () => {
  const dataset = datasetId('main', 'cars');
  const base = baseView('base-cars', 'source-cars', 'main', 'cars', 'SELECT * FROM "source_cars"."cars"');
  const valid = {
    id: `source-cars:${dataset}`, projectId: 'default', name: 'cars', kind: 'source', nodeId: 'source-cars', dataset,
    versions: [legacyVersion('source-cars', dataset, 'SELECT * FROM cars')], activeVersionId: 'v1', pendingParentId: null, pendingChanges: []
  };
  const malformed = { ...valid, id: 'malformed', dataset: '%not-base64', versions: [{ ...valid.versions[0], dataset: '%not-base64' }] };
  const unmatched = { ...valid, id: 'unmatched', nodeId: 'missing-source', versions: [{ ...valid.versions[0], nodeId: 'missing-source' }] };
  const current = { ...valid, id: 'current', nodeId: 'project_default', versions: [{ ...valid.versions[0], nodeId: 'project_default' }] };

  const rebound = rebindLegacyHistories([malformed, valid, unmatched, current], [base], defaultProject);

  assert.strictEqual(rebound[0], malformed);
  assert.equal(rebound[1].id, 'base-cars');
  assert.strictEqual(rebound[2], unmatched);
  assert.strictEqual(rebound[3], current);
  assert.strictEqual(rebindLegacyHistories([current], [base], defaultProject)[0], current);
});
