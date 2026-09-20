import assert from 'node:assert/strict';
import test from 'node:test';
import {
  classifyColumn,
  filtersFromMark,
  niceTicks,
  specFromColumns,
  suggestCharts,
  toPlotNumber
} from '../src/lib/visualize.ts';

function col(name, type, profile_kind, numeric = profile_kind === 'numeric') {
  return { name, type, numeric, null_fraction: 0, profile_kind };
}

const region = col('region', 'VARCHAR', 'categorical', false);
const flag = col('flag', 'BOOLEAN', 'categorical', false);
const rating = col('rating', 'INTEGER', 'numeric');
const price = col('price', 'DOUBLE', 'numeric');
const amount = col('amount', 'DECIMAL(10,2)', 'numeric');
const delay = col('delay', 'FLOAT', 'numeric');
const day = col('day', 'DATE', 'date', false);
const blob = col('payload', 'BLOB', null, false);

function charts(columns) {
  return suggestCharts(columns).map((item) => item.chart);
}

function defaultOf(columns) {
  return suggestCharts(columns)[0];
}

test('classifies columns by profile kind and integer vs float', () => {
  assert.equal(classifyColumn(region), 'categorical');
  assert.equal(classifyColumn(flag), 'categorical');
  assert.equal(classifyColumn(rating), 'discrete');
  assert.equal(classifyColumn(col('count', 'BIGINT', 'numeric')), 'discrete');
  assert.equal(classifyColumn(col('tiny', 'UTINYINT', 'numeric')), 'discrete');
  assert.equal(classifyColumn(price), 'continuous');
  assert.equal(classifyColumn(amount), 'continuous');
  assert.equal(classifyColumn(day), 'date');
  assert.equal(classifyColumn(blob), null);
});

test('one categorical column suggests a count bar', () => {
  const suggestion = defaultOf([region]);
  assert.deepEqual(charts([region]), ['bar']);
  assert.equal(suggestion.chart, 'bar');
  assert.deepEqual(suggestion.encodings, { category: 'region' });
  assert.equal(suggestion.metric, 'count');
});

test('one discrete column prefers a count bar and offers histogram and box', () => {
  assert.deepEqual(charts([rating]), ['bar', 'histogram', 'box']);
  assert.deepEqual(defaultOf([rating]).encodings, { category: 'rating' });
  assert.deepEqual(suggestCharts([rating])[1].encodings, { value: 'rating' });
  assert.deepEqual(suggestCharts([rating])[2].encodings, { value: 'rating' });
});

test('one continuous column prefers histogram then box', () => {
  assert.deepEqual(charts([price]), ['histogram', 'box']);
  assert.deepEqual(defaultOf([price]).encodings, { value: 'price' });
});

test('one date column prefers a time histogram then a year bar', () => {
  assert.deepEqual(charts([day]), ['histogram', 'bar']);
  assert.deepEqual(defaultOf([day]).encodings, { value: 'day' });
  assert.deepEqual(suggestCharts([day])[1].encodings, { category: 'day' });
});

test('categorical plus continuous defaults to an aggregated bar', () => {
  const suggestion = defaultOf([region, price]);
  assert.deepEqual(charts([region, price]), ['bar', 'box', 'histogram']);
  assert.equal(suggestion.chart, 'bar');
  assert.deepEqual(suggestion.encodings, { category: 'region', value: 'price' });
  assert.equal(suggestion.metric, 'avg');
  assert.deepEqual(suggestCharts([region, price])[1].encodings, { group: 'region', value: 'price' });
  assert.deepEqual(suggestCharts([region, price])[2].encodings, { group: 'region', value: 'price' });
});

test('two number columns prefer scatter even when one is integer', () => {
  assert.deepEqual(charts([rating, price]), ['scatter', 'line']);
  assert.deepEqual(defaultOf([rating, price]).encodings, { x: 'rating', y: 'price' });
  assert.deepEqual(charts([rating, col('qty', 'INTEGER', 'numeric')]), ['scatter', 'line']);
});

test('two continuous columns prefer scatter then line, in selection order', () => {
  assert.deepEqual(charts([price, amount]), ['scatter', 'line']);
  assert.deepEqual(defaultOf([price, amount]).encodings, { x: 'price', y: 'amount' });
  assert.deepEqual(defaultOf([amount, price]).encodings, { x: 'amount', y: 'price' });
});

test('date plus continuous prefers line then scatter', () => {
  assert.deepEqual(charts([day, price]), ['line', 'scatter']);
  assert.deepEqual(defaultOf([day, price]).encodings, { x: 'day', y: 'price' });
});

test('a third continuous column becomes scatter size', () => {
  const suggestion = defaultOf([price, amount, delay]);
  assert.equal(suggestion.chart, 'scatter');
  assert.deepEqual(suggestion.encodings, { x: 'price', y: 'amount', size: 'delay' });
});

test('two continuous plus a category colors the scatter', () => {
  assert.deepEqual(defaultOf([price, amount, region]).encodings, { x: 'price', y: 'amount', color: 'region' });
});

test('two categories plus a continuous measure groups the bar', () => {
  const suggestion = defaultOf([region, flag, price]);
  assert.equal(suggestion.chart, 'bar');
  assert.equal(suggestion.metric, 'avg');
  assert.deepEqual(suggestion.encodings, { category: 'region', value: 'price', group: 'flag' });
});

test('two categoricals alone and empty selections suggest nothing', () => {
  assert.deepEqual(suggestCharts([]), []);
  assert.deepEqual(suggestCharts([region, flag]), []);
  assert.equal(specFromColumns([]), null);
});

test('specFromColumns returns the default suggestion as a spec', () => {
  assert.deepEqual(specFromColumns([price]), {
    chart: 'histogram',
    encodings: { value: 'price' }
  });
  assert.deepEqual(specFromColumns([region, price]), {
    chart: 'bar',
    encodings: { category: 'region', value: 'price' },
    metric: 'avg'
  });
});

test('clicking a bar produces an equality filter on the category encoding', () => {
  const spec = specFromColumns([region]);
  assert.deepEqual(filtersFromMark(spec, { kind: 'category', value: 'west' }), [
    { column: 'region', operator: '=', value: 'west' }
  ]);
  assert.deepEqual(filtersFromMark(spec, { kind: 'category', value: 'west' }, 1), [
    { column: 'region', operator: '=', value: 'west', connector: 'and' }
  ]);
});

test('clicking a grouped box uses the group encoding as a category filter', () => {
  const spec = suggestCharts([region, price]).find((item) => item.chart === 'box');
  assert.deepEqual(filtersFromMark(spec, { kind: 'category', value: 'west' }), [
    { column: 'region', operator: '=', value: 'west' }
  ]);
});

test('nice ticks stay finite when the domain is empty or unbounded', () => {
  assert.deepEqual(niceTicks(Number.POSITIVE_INFINITY, Number.NEGATIVE_INFINITY), [0, 1]);
  assert.deepEqual(niceTicks(Number.NaN, 10), [0, 1]);
});

test('nice ticks land on 1-2-5 steps and include the origin for counts', () => {
  const hundreds = niceTicks(0, 100, 5);
  assert.equal(hundreds[0], 0);
  assert.equal(hundreds[hundreds.length - 1], 100);
  assert.equal(hundreds.length >= 4 && hundreds.length <= 8, true);
  assert.deepEqual(niceTicks(0, 3, 4), [0, 1, 2, 3]);
  const ticks = niceTicks(12, 87, 5);
  assert.equal(ticks[0] <= 12, true);
  assert.equal(ticks[ticks.length - 1] >= 87, true);
  for (let i = 1; i < ticks.length; i++) assert.equal(ticks[i] > ticks[i - 1], true);
});

test('toPlotNumber reads numeric and ISO date encodings', () => {
  assert.equal(toPlotNumber(12.5), 12.5);
  assert.equal(toPlotNumber('100'), 100);
  assert.equal(toPlotNumber('2025-01-02'), Date.parse('2025-01-02'));
});

test('a scatter region becomes inclusive filters on both axes', () => {
  const spec = defaultOf([price, amount]);
  assert.deepEqual(filtersFromMark(spec, { kind: 'region', xMin: 1, xMax: 4, yMin: 10, yMax: 20 }), [
    { column: 'price', operator: '>=', value: 1 },
    { column: 'price', operator: '<=', value: 4, connector: 'and' },
    { column: 'amount', operator: '>=', value: 10, connector: 'and' },
    { column: 'amount', operator: '<=', value: 20, connector: 'and' }
  ]);
});

test('clicking a histogram bin produces a closed-open range except the last bin', () => {
  const spec = specFromColumns([price]);
  assert.deepEqual(filtersFromMark(spec, { kind: 'bin', lower: 10, upper: 20 }), [
    { column: 'price', operator: '>=', value: 10 },
    { column: 'price', operator: '<', value: 20, connector: 'and' }
  ]);
  assert.deepEqual(filtersFromMark(spec, { kind: 'bin', lower: 80, upper: 100, last: true }), [
    { column: 'price', operator: '>=', value: 80 },
    { column: 'price', operator: '<=', value: 100, connector: 'and' }
  ]);
});
