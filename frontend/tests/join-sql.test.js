import assert from 'node:assert/strict';
import test from 'node:test';
import { buildJoinSql } from '../src/lib/join-sql.ts';

const orders = { id: 'orders', name: 'orders', schema: 'main', columns: ['id', 'customer_id', 'total'] };
const customers = { id: 'customers', name: 'customers', schema: 'main', columns: ['id', 'name'] };

test('builds an inner join with composite keys and quoted identifiers', () => {
  assert.equal(
    buildJoinSql(orders, customers, [{ left: 'customer_id', right: 'id' }, { left: 'id', right: 'id' }], ['id', 'total'], ['id', 'name']),
    'SELECT "left"."id" AS "orders.id", "left"."total", "right"."id" AS "customers.id", "right"."name" FROM "main"."orders" AS "left" INNER JOIN "main"."customers" AS "right" ON "left"."customer_id" = "right"."id" AND "left"."id" = "right"."id"'
  );
});

test('escapes identifiers and includes schemas when dataset names collide', () => {
  const left = { id: 'one', name: 'same"name', schema: 'left schema', columns: ['shared'] };
  const right = { id: 'two', name: 'same"name', schema: 'right schema', columns: ['shared'] };
  assert.equal(
    buildJoinSql(left, right, [{ left: 'shared', right: 'shared' }], ['shared'], ['shared']),
    'SELECT "left"."shared" AS "left schema.same""name.shared", "right"."shared" AS "right schema.same""name.shared" FROM "left schema"."same""name" AS "left" INNER JOIN "right schema"."same""name" AS "right" ON "left"."shared" = "right"."shared"'
  );
});

test('disambiguates generated aliases from literal column names', () => {
  const awkwardOrders = { ...orders, columns: ['id', 'orders.id'] };
  assert.equal(
    buildJoinSql(awkwardOrders, customers, [{ left: 'id', right: 'id' }], ['id', 'orders.id'], ['id']),
    'SELECT "left"."id" AS "orders.id", "left"."orders.id" AS "orders.id 2", "right"."id" AS "customers.id" FROM "main"."orders" AS "left" INNER JOIN "main"."customers" AS "right" ON "left"."id" = "right"."id"'
  );
});

test('returns empty SQL for incomplete or repeated key pairs', () => {
  assert.equal(buildJoinSql(orders, customers, [{ left: '', right: 'id' }], ['id'], ['name']), '');
  assert.equal(buildJoinSql(orders, customers, [{ left: 'id', right: 'id' }, { left: 'id', right: 'id' }], ['id'], ['name']), '');
});

test('returns empty SQL without keys or output columns', () => {
  assert.equal(buildJoinSql(orders, customers, [], ['id'], ['name']), '');
  assert.equal(buildJoinSql(orders, customers, [{ left: 'id', right: 'id' }], [], []), '');
});
