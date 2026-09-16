import assert from 'node:assert/strict';
import test from 'node:test';
import { tableFromArrays, tableToIPC, vectorFromArray, Binary, Uint64, Int64 } from 'apache-arrow';
import { ARROW_MEDIA_TYPE, arrowQuery, concatRows, decodeQueryResponse, jsonRows } from '../src/lib/table-rows.ts';

test('Arrow snapshots retain typed columns and match JSON cell semantics', async () => {
  const table = tableFromArrays({
    id: vectorFromArray([1n, 9007199254740993n, null], new Int64()),
    unsigned: vectorFromArray([18446744073709551615n, 0n, null], new Uint64()),
    name: ['alpha', 'βeta', null],
    number: [NaN, Infinity, 1.5],
    blob: vectorFromArray([new Uint8Array([0, 255]), new Uint8Array(), null], new Binary()),
    exotic: ['"2025-01-02T03:04:05.123456"', '{"x":[1,null]}', 'null'],
  });
  const metadata = { columns: [], page: 2, page_size: 3, total_rows: 6, total_pages: 2, sql: 'SELECT * FROM sample', elapsed_ms: 1, json_columns: ['exotic'] };
  table.schema.metadata.set('quark', JSON.stringify(metadata));
  const bytes = tableToIPC(table);
  const result = await decodeQueryResponse(new Response(bytes, { headers: { 'Content-Type': ARROW_MEDIA_TYPE } }));
  assert.equal(result.page, 2);
  assert.equal(result.rows.length, 3);
  assert.equal(Array.isArray(result.rows), false);
  assert.deepEqual(['id', 'unsigned', 'name', 'number', 'blob', 'exotic'].map(name => result.rows.cell(0, name)), [1, '18446744073709551615', 'alpha', null, '00ff', '2025-01-02T03:04:05.123456']);
  assert.equal(result.rows.cell(1, 'id'), '9007199254740993');
  assert.deepEqual(result.rows.cell(1, 'exotic'), { x: [1, null] });
  assert.equal(result.rows.cell(2, 'name'), null);
  assert.equal(result.rows.cell(3, 'id'), undefined);
  assert.equal(result.rows.cell(-1, 'id'), undefined);

  const joined = concatRows([result.rows, result.rows]);
  assert.equal(joined.length, 6);
  assert.equal(joined.cell(4, 'id'), '9007199254740993');
  assert.equal(joined.cell(6, 'id'), undefined);
  assert.equal(joined.cell(-1, 'id'), undefined);
  assert.equal(concatRows([]).length, 0);
  assert.throws(() => arrowQuery(tableToIPC(tableFromArrays({ id: [1] }))), /Missing Arrow query metadata/);
});

test('JSON compatibility and snapshot concatenation do not eagerly read cells', async () => {
  const result = await decodeQueryResponse(Response.json({ rows: [{ id: 1 }], page: 1 }));
  assert.equal(result.rows.cell(0, 'id'), 1);
  const raw = [{ id: 2 }];
  const joined = concatRows([result.rows, jsonRows(raw)]);
  raw[0].id = 3;
  assert.equal(joined.cell(1, 'id'), 3);
});
