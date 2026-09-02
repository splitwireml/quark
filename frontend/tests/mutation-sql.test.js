import assert from 'node:assert/strict';
import test from 'node:test';
import { buildCellEditSql, buildColumnReplacementSql, buildIfExpression, buildMutationSql, buildSwitchExpression, hasVolatileRowOrder, nextDuplicateColumnName, quoteIdentifier, quoteLiteral, stripTerminalSemicolon } from '../src/lib/mutation-sql.ts';

test('quotes SQL identifiers and literals', () => {
  assert.equal(quoteIdentifier('a"b'), '"a""b"');
  assert.equal(quoteLiteral("O'Brien"), "'O''Brien'");
});

test('strips one terminal semicolon before trailing comments', () => {
  assert.equal(stripTerminalSemicolon(' SELECT 1;  '), 'SELECT 1');
  assert.equal(stripTerminalSemicolon('SELECT 1;;'), 'SELECT 1;');
  assert.equal(stripTerminalSemicolon('SELECT 1; -- trailing'), 'SELECT 1 -- trailing');
  assert.equal(stripTerminalSemicolon('SELECT 1; /* trailing */'), 'SELECT 1 /* trailing */');
});

test('detects sources whose row order changes on re-execution', () => {
  assert.equal(hasVolatileRowOrder('SELECT * FROM items ORDER BY random()'), true);
  assert.equal(hasVolatileRowOrder('SELECT uuid() AS id FROM items'), true);
  assert.equal(hasVolatileRowOrder('SELECT uuidv4() AS id FROM items'), true);
  assert.equal(hasVolatileRowOrder('SELECT gen_random_uuid() AS id FROM items'), true);
  assert.equal(hasVolatileRowOrder('SELECT * FROM items USING SAMPLE 10%'), true);
  assert.equal(hasVolatileRowOrder('SELECT * FROM items TABLESAMPLE 10 PERCENT'), true);
  assert.equal(hasVolatileRowOrder('SELECT * FROM items ORDER BY id'), false);
});

test('inserts an aliased expression at the full result column index', () => {
  assert.equal(
    buildMutationSql(' SELECT * FROM cars; ', ['first', 'hidden', 'last'], 2, 'upper("first")', 'new "value"'),
    'SELECT "first", "hidden", upper("first") AS "new ""value""", "last" FROM (\nSELECT * FROM cars\n) AS mutation_source'
  );
});

test('keeps a trailing line comment away from the wrapper close', () => {
  assert.equal(
    buildMutationSql('SELECT 1 AS first; -- trailing', ['first'], 1, '2', 'second'),
    'SELECT "first", 2 AS "second" FROM (\nSELECT 1 AS first -- trailing\n) AS mutation_source'
  );
});

test('replaces a column in place with its current name', () => {
  assert.equal(
    buildColumnReplacementSql('SELECT * FROM cars;', ['first', 'amount', 'last'], 'amount', 'try_cast("amount" AS DOUBLE)', 'amount'),
    'SELECT "first", try_cast("amount" AS DOUBLE) AS "amount", "last" FROM (\nSELECT * FROM cars\n) AS mutation_source'
  );
});

test('renames a column in place and rejects case-insensitive collisions', () => {
  assert.equal(
    buildColumnReplacementSql('SELECT * FROM cars', ['first', 'amount', 'last'], 'amount', '"amount"', 'total'),
    'SELECT "first", "amount" AS "total", "last" FROM (\nSELECT * FROM cars\n) AS mutation_source'
  );
  assert.equal(buildColumnReplacementSql('SELECT * FROM cars', ['first', 'amount', 'last'], 'amount', '"amount"', 'LAST'), '');
});

test('chooses the next case-insensitive duplicate column suffix', () => {
  assert.equal(nextDuplicateColumnName('value', ['value']), 'value_2');
  assert.equal(nextDuplicateColumnName('Value', ['value', 'VALUE_2', 'value_3']), 'Value_4');
});

test('builds non-nested IF and switch expressions only when every operand is filled', () => {
  assert.equal(buildIfExpression('"active" = TRUE', "'yes'", "'no'"), `CASE WHEN "active" = TRUE THEN 'yes' ELSE 'no' END`);
  assert.equal(buildIfExpression('', "'yes'", "'no'"), '');
  assert.equal(buildSwitchExpression('"status"', [{ match: "'open'", thenValue: '1' }, { match: "'closed'", thenValue: '2' }], '0'), `CASE "status" WHEN 'open' THEN 1 WHEN 'closed' THEN 2 ELSE 0 END`);
  assert.equal(buildSwitchExpression('"status"', [{ match: '', thenValue: '1' }], '0'), '');
});

test('replaces one absolute row cell while preserving column order', () => {
  assert.equal(
    buildCellEditSql(' SELECT * FROM cars; -- current view', ['first', 'amount', 'last'], 101, 'amount', "12'3"),
    'SELECT "first", CASE WHEN "__quark_row_number" = 101 THEN cast_to_type(\'12\'\'3\', "amount") ELSE "amount" END AS "amount", "last" FROM (\nSELECT *, row_number() OVER () AS "__quark_row_number" FROM (\nSELECT * FROM cars -- current view\n) AS cell_edit_source\n) AS cell_edit_numbered'
  );
});

test('uses a case-insensitive collision-free internal row alias', () => {
  const sql = buildCellEditSql('SELECT 1', ['__QUARK_ROW_NUMBER', '__quark_row_number_2', 'value'], 1, 'value', 'new');
  assert.match(sql, /row_number\(\) OVER \(\) AS "__quark_row_number_3"/);
  assert.match(sql, /CASE WHEN "__quark_row_number_3" = 1/);
});

test('rejects invalid cell edits', () => {
  assert.equal(buildCellEditSql('', ['value'], 1, 'value', 'new'), '');
  assert.equal(buildCellEditSql('SELECT 1', [], 1, 'value', 'new'), '');
  assert.equal(buildCellEditSql('SELECT 1', ['value'], 0, 'value', 'new'), '');
  assert.equal(buildCellEditSql('SELECT 1', ['value'], 1.5, 'value', 'new'), '');
  assert.equal(buildCellEditSql('SELECT 1', ['value'], 1, 'missing', 'new'), '');
  assert.equal(buildCellEditSql('SELECT 1', ['value', 'value'], 1, 'value', 'new'), '');
});
