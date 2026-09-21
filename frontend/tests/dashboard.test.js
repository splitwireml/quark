import assert from 'node:assert/strict';
import test from 'node:test';
import { chartTitle, clampPlacement, nextDashboardName, snapMove, snapResize, updatePlacedChart, composeFilters, filtersForChart, readDashboards, selectionChartIdAtFilterIndex, updateActiveDashboardTab, withoutSelections } from '../src/lib/dashboard.ts';

const base = [{ column: 'year', operator: '>=', value: 2024 }];
const selections = [
  { chartId: 'bar', filters: [{ column: 'region', operator: '=', value: 'East' }] },
  { chartId: 'scatter', filters: [{ column: 'sales', operator: '>=', value: 10 }, { column: 'sales', operator: '<=', value: 20 }] }
];

test('dashboard filters intersect and source charts exclude only their own selection', () => {
  const all = composeFilters(base, selections);
  assert.deepEqual(all.map((filter) => filter.column), ['year', 'region', 'sales', 'sales']);
  assert.deepEqual(filtersForChart(all, selections, 'bar').map((filter) => filter.column), ['year', 'sales', 'sales']);
  assert.deepEqual(filtersForChart(all, selections, 'scatter').map((filter) => filter.column), ['year', 'region']);
  assert.deepEqual(withoutSelections(all, selections), base);
  assert.equal(selectionChartIdAtFilterIndex(all, selections, 0), undefined);
  assert.equal(selectionChartIdAtFilterIndex(all, selections, 1), 'bar');
  assert.equal(selectionChartIdAtFilterIndex(all, selections, 2), 'scatter');
});

test('dashboard selections preserve base logic and remove the selected occurrence of duplicate filters', () => {
  const duplicate = { column: 'region', operator: '=', value: 'East' };
  const baseWithOr = [duplicate, { column: 'year', operator: '=', value: 2024, connector: 'or' }];
  const selected = [{ chartId: 'bar', filters: [duplicate] }];
  const all = composeFilters(baseWithOr, selected);

  assert.equal(all[1].connector, 'or');
  assert.equal(all[2].connector, 'and');
  assert.deepEqual(withoutSelections(all, selected), baseWithOr);
});

test('placements stay inside the bounded canvas', () => {
  assert.deepEqual(clampPlacement({ id: 'p', chartId: 'c', x: 1190, y: -5, width: 400, height: 100 }, 1200), {
    id: 'p', chartId: 'c', x: 800, y: 0, width: 400, height: 100
  });
});

test('invalid dashboard storage is ignored', () => {
  assert.deepEqual(readDashboards('{broken'), []);
  assert.equal(readDashboards('[{"datasetId":"d","activeTabId":"","charts":[],"tabs":[]}]').length, 1);
  assert.deepEqual(readDashboards('[{"datasetId":"d","activeTabId":"t","charts":[],"tabs":[{"id":"t"}]}]'), []);
});

test('active tab updates do not touch sibling layouts', () => {
  const document = { datasetId: 'd', activeTabId: 'a', charts: [], tabs: [
    { id: 'a', name: 'A', scrollTop: 0, placements: [] },
    { id: 'b', name: 'B', scrollTop: 20, placements: [] }
  ] };
  const next = updateActiveDashboardTab(document, (tab) => ({ ...tab, scrollTop: 50 }));
  assert.equal(next.tabs[0].scrollTop, 50);
  assert.equal(next.tabs[1], document.tabs[1]);
});

test('free placements retain drawn sizes and can extend beyond the former 1200px canvas', () => {
  const small = { id: 'p', chartId: 'c', x: 1300, y: 70, width: 173, height: 119 };
  assert.deepEqual(clampPlacement(small), small);
  assert.deepEqual(clampPlacement({ ...small, x: -10, width: -4, height: 0 }), { ...small, x: 0, width: 64, height: 48 });
});

test('resize snaps the moving edges to the nearest neighbor and releases outside tolerance', () => {
  const tile = { id: 'p', chartId: 'c', x: 20, y: 20, width: 278, height: 277 };
  const neighbor = { id: 'n', chartId: 'c', x: 300, y: 300, width: 200, height: 180 };
  const result = snapResize(tile, [tile, neighbor]);
  assert.equal(result.placement.width, 280);
  assert.equal(result.placement.height, 280);
  assert.deepEqual(result.guides.map(({ axis, position }) => [axis, position]), [['x', 300], ['y', 300]]);
  assert.deepEqual(snapResize({ ...tile, width: 270, height: 270 }, [neighbor]).guides, []);
  assert.deepEqual(snapResize(tile, [tile]).guides, []);
  const closer = { ...neighbor, id: 'closer', x: 299 };
  assert.equal(snapResize(tile, [neighbor, closer]).placement.width, 279);
});

test('default dashboard names are numbered and avoid existing names', () => {
  assert.equal(nextDashboardName([]), 'Dashboard_1');
  assert.equal(nextDashboardName(['Dashboard_1', 'Dashboard_2', 'Overview']), 'Dashboard_3');
  assert.equal(nextDashboardName(['Dashboard_2', 'Overview']), 'Dashboard_1');
});

test('moving snaps edges and centers without resizing and respects canvas boundaries', () => {
  const neighbor = { id: 'n', chartId: 'c', x: 300, y: 100, width: 200, height: 180 };
  const tile = { id: 'p', chartId: 'c', x: 97, y: 103, width: 200, height: 100 };
  const edge = snapMove(tile, [neighbor]);
  assert.deepEqual(edge.placement, { ...tile, x: 100, y: 100 });
  assert.equal(edge.guides.length, 2);
  const centered = snapMove({ ...tile, x: 348, y: 139, width: 100 }, [neighbor]);
  assert.deepEqual(centered.placement, { ...tile, x: 350, y: 140, width: 100 });
  assert.deepEqual(snapMove(tile, [tile]).guides, []);
  assert.deepEqual(snapMove({ ...tile, x: 80, y: 80 }, [neighbor]).guides, []);
  assert.equal(snapMove({ ...tile, x: 101 }, [neighbor], 299).placement.x, 101, 'out-of-bounds targets are ignored');
});

test('editing one placement preserves siblings, layout, and custom chart titles', () => {
  const spec = { chart: 'bar', encodings: { category: 'region' }, metric: 'count' };
  const nextSpec = { chart: 'histogram', encodings: { value: 'sales' } };
  const placement = { id: 'p', chartId: 'c', x: 30, y: 40, width: 240, height: 180 };
  const document = { datasetId: 'd', activeTabId: 'a', charts: [{ id: 'c', title: 'My chart', spec }], tabs: [
    { id: 'a', name: 'A', scrollTop: 0, placements: [placement] },
    { id: 'b', name: 'B', scrollTop: 20, placements: [{ ...placement, id: 'other' }] }
  ] };
  const edited = updatePlacedChart(document, 'p', nextSpec);
  const editedId = edited.tabs[0].placements[0].chartId;
  assert.notEqual(editedId, 'c');
  assert.deepEqual(edited.tabs[0].placements[0], { ...placement, chartId: editedId });
  assert.equal(edited.tabs[1], document.tabs[1]);
  assert.equal(edited.charts[0], document.charts[0]);
  assert.equal(edited.charts[1].title, 'My chart');
  assert.deepEqual(edited.charts[1].spec, nextSpec);
  const again = updatePlacedChart(edited, 'p', spec);
  assert.equal(again.charts.length, 2, 'later edits reuse the now-independent chart');
  assert.equal(again.charts[1].id, editedId);
  assert.equal(updatePlacedChart(document, 'missing', nextSpec), document);
  assert.equal(updatePlacedChart(document, 'p', spec), document);
  const automatic = updatePlacedChart({ ...document, charts: [{ id: 'c', title: 'Bar · region', spec }] }, 'p', nextSpec);
  assert.equal(automatic.charts[1].title, 'Histogram · sales');
});

test('a chart title names the group, size, and color columns', () => {
  const spec = { chart: 'scatter', encodings: { x: 'price', y: 'amount', size: 'weight', color: 'region' } };
  assert.equal(chartTitle(spec), 'Scatter · price × amount × weight × region');
});

test('a chart title without the new channels is unchanged', () => {
  assert.equal(chartTitle({ chart: 'bar', encodings: { category: 'region' } }), 'Bar · region');
});
