import assert from 'node:assert/strict';
import test from 'node:test';
import { clampPlacement, composeFilters, filtersForChart, readDashboards, selectionChartIdAtFilterIndex, updateActiveDashboardTab, withoutSelections } from '../src/lib/dashboard.ts';

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
  assert.deepEqual(clampPlacement({ id: 'p', chartId: 'c', x: 1190, y: -5, width: 400, height: 100 }), {
    id: 'p', chartId: 'c', x: 800, y: 0, width: 400, height: 220
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
