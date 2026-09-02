import assert from 'node:assert/strict';
import test from 'node:test';
import { buildAggregateSql } from '../src/lib/aggregate-sql.ts';

const source = 'SELECT * FROM "main"."cars" WHERE "active" = true';

test('count on one column creates its distribution', () => {
  assert.equal(
    buildAggregateSql(source, [], [{ column: 'Make', metrics: ['count'] }]),
    'SELECT "Make", count(*) AS "count" FROM (SELECT * FROM "main"."cars" WHERE "active" = true) AS filtered GROUP BY "Make" ORDER BY "Make"'
  );
});

test('indexes are independent from an aggregate field', () => {
  assert.equal(
    buildAggregateSql(source, ['Make', 'Model'], [{ column: 'Price', metrics: ['count', 'sum'] }]),
    'SELECT "Make", "Model", count("Price") AS "count", sum("Price") AS "sum" FROM (SELECT * FROM "main"."cars" WHERE "active" = true) AS filtered GROUP BY "Make", "Model" ORDER BY "Make"'
  );
});

test('aggregates fields before and after an index without position semantics', () => {
  const orderedSource = 'SELECT "Price", "Make", "Year" FROM "main"."cars"';
  assert.equal(
    buildAggregateSql(orderedSource, ['Make'], [
      { column: 'Price', metrics: ['sum'] },
      { column: 'Year', metrics: ['avg'] }
    ]),
    'SELECT "Make", sum("Price") AS "Price sum", avg("Year") AS "Year avg" FROM (SELECT "Price", "Make", "Year" FROM "main"."cars") AS filtered GROUP BY "Make" ORDER BY "Make"'
  );
});

test('skips empty aggregate entries and returns no SQL without metrics', () => {
  assert.equal(
    buildAggregateSql(source, ['Make'], [
      { column: 'Price', metrics: [] },
      { column: 'Year', metrics: ['max'] }
    ]),
    'SELECT "Make", max("Year") AS "max" FROM (SELECT * FROM "main"."cars" WHERE "active" = true) AS filtered GROUP BY "Make" ORDER BY "Make"'
  );
  assert.equal(buildAggregateSql(source, ['Make'], [{ column: 'Price', metrics: [] }]), '');
});

test('quotes index, aggregate, and alias identifiers safely', () => {
  assert.equal(
    buildAggregateSql(source, ['Maker"Name'], [
      { column: 'List"Price', metrics: ['sum'] },
      { column: 'Model"Year', metrics: ['avg'] }
    ]),
    'SELECT "Maker""Name", sum("List""Price") AS "List""Price sum", avg("Model""Year") AS "Model""Year avg" FROM (SELECT * FROM "main"."cars" WHERE "active" = true) AS filtered GROUP BY "Maker""Name" ORDER BY "Maker""Name"'
  );
});
