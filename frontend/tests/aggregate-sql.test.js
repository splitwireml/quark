import assert from 'node:assert/strict';
import test from 'node:test';
import { buildAggregateSql } from '../src/lib/aggregate-sql.ts';

const source = 'SELECT * FROM "main"."cars" WHERE "active" = true';

test('count on one column creates its distribution', () => {
  assert.equal(
    buildAggregateSql(source, ['Make'], ['count']),
    'SELECT "Make", count(*) AS "count" FROM (SELECT * FROM "main"."cars" WHERE "active" = true) AS filtered GROUP BY "Make" ORDER BY "Make"'
  );
});

test('every column before the last is an index', () => {
  assert.equal(
    buildAggregateSql(source, ['Make', 'Model', 'Price'], ['count', 'sum']),
    'SELECT "Make", "Model", count("Price") AS "count", sum("Price") AS "sum" FROM (SELECT * FROM "main"."cars" WHERE "active" = true) AS filtered GROUP BY "Make", "Model" ORDER BY "Make"'
  );
});
