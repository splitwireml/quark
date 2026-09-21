import assert from 'node:assert/strict';
import test from 'node:test';
import {
  applyRoles,
  autoGrain,
  chartRoles,
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
  assert.deepEqual(charts([rating, price]), ['scatter']);
  assert.deepEqual(defaultOf([rating, price]).encodings, { x: 'rating', y: 'price' });
  assert.deepEqual(charts([rating, col('qty', 'INTEGER', 'numeric')]), ['scatter']);
});

test('two continuous columns are a scatter, in selection order', () => {
  assert.deepEqual(charts([price, amount]), ['scatter']);
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

test('chartRoles lists only the roles each chart accepts', () => {
  assert.deepEqual(chartRoles('bar'), ['category', 'value', 'group']);
  assert.deepEqual(chartRoles('pie'), ['category', 'value']);
  assert.deepEqual(chartRoles('scatter'), ['x', 'y', 'size', 'color', 'pattern']);
  assert.deepEqual(chartRoles('line'), ['x', 'y', 'group']);
  assert.deepEqual(chartRoles('histogram'), ['value', 'group']);
  assert.deepEqual(chartRoles('box'), ['value', 'group']);
});

test('applyRoles swaps two columns between roles', () => {
  const encodings = { x: 'price', y: 'amount' };
  const result = applyRoles(encodings, 'scatter', { price: 'y', amount: 'x' }, [price, amount]);
  assert.deepEqual(result, { y: 'price', x: 'amount' });
});

test('applyRoles displaces the previous occupant of a role to unassigned', () => {
  const result = applyRoles({ x: 'price', y: 'amount' }, 'scatter', { price: 'y' }, [price, amount]);
  assert.deepEqual(result, { y: 'price' });
});

test('applyRoles leaves untouched roles to inference', () => {
  const result = applyRoles({ x: 'price', y: 'amount', color: 'region' }, 'scatter', { region: 'pattern' }, [price, amount, region]);
  assert.deepEqual(result, { x: 'price', y: 'amount', pattern: 'region' });
});

test('applyRoles drops an override whose column left the selection', () => {
  const result = applyRoles({ x: 'price', y: 'amount' }, 'scatter', { region: 'color' }, [price, amount]);
  assert.deepEqual(result, { x: 'price', y: 'amount' });
});

test('applyRoles drops an override the active chart does not accept', () => {
  const result = applyRoles({ category: 'region', value: 'price' }, 'bar', { price: 'size' }, [region, price]);
  assert.deepEqual(result, { category: 'region', value: 'price' });
});

test('clicking a series bar filters on both the category and the group', () => {
  const spec = { chart: 'bar', encodings: { category: 'region', value: 'price', group: 'flag' }, metric: 'sum' };
  assert.deepEqual(filtersFromMark(spec, { kind: 'category', value: 'east', series: 'true' }), [
    { column: 'region', operator: '=', value: 'east' },
    { column: 'flag', operator: '=', value: 'true', connector: 'and' }
  ]);
});

test('a category mark without a series filters on the category alone', () => {
  const spec = { chart: 'bar', encodings: { category: 'region', group: 'flag' } };
  assert.deepEqual(filtersFromMark(spec, { kind: 'category', value: 'east' }), [
    { column: 'region', operator: '=', value: 'east' }
  ]);
});

test('extra columns beyond a scatter pair land on the size, color, and shape channels', () => {
  const suggestion = defaultOf([price, amount, rating, region, flag]);
  assert.equal(suggestion.chart, 'scatter');
  assert.deepEqual(suggestion.encodings, {
    x: 'price', y: 'amount', size: 'rating', color: 'region', pattern: 'flag'
  });
});

test('two numerics and two categoricals fill color and shape but not size', () => {
  const suggestion = defaultOf([price, amount, region, flag]);
  assert.deepEqual(suggestion.encodings, { x: 'price', y: 'amount', color: 'region', pattern: 'flag' });
});

test('four numerics put the spare ones on size and color', () => {
  const suggestion = defaultOf([price, amount, rating, delay]);
  assert.deepEqual(suggestion.encodings, { x: 'price', y: 'amount', size: 'rating', color: 'delay' });
});

test('autoGrain picks the finest grain that fits the point budget', () => {
  assert.equal(autoGrain(60 * 60 * 24), 'hour');
  assert.equal(autoGrain(60 * 60 * 24 * 60), 'day');
  assert.equal(autoGrain(60 * 60 * 24 * 365 * 5), 'week');
  assert.equal(autoGrain(60 * 60 * 24 * 365 * 20), 'month');
  assert.equal(autoGrain(60 * 60 * 24 * 365 * 500), 'year');
});

test('two numeric columns no longer suggest a line chart', () => {
  assert.deepEqual(charts([price, amount]), ['scatter']);
});

test('a date, a number, and a category make a grouped line', () => {
  const suggestion = defaultOf([day, price, region]);
  assert.equal(suggestion.chart, 'line');
  assert.deepEqual(suggestion.encodings, { x: 'day', y: 'price', group: 'region' });
});

test('a date with only categories counts rows per group over time', () => {
  const suggestion = defaultOf([day, region]);
  assert.equal(suggestion.chart, 'line');
  assert.deepEqual(suggestion.encodings, { x: 'day', group: 'region' });
});

test('a date and a number alone still prefer line then scatter', () => {
  assert.deepEqual(charts([day, price]), ['line', 'scatter']);
});
