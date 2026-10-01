import assert from 'node:assert/strict';
import test from 'node:test';
import { buildCellEditSql, buildColumnReplacementSql, buildIfExpression, buildMutationSql, buildSwitchExpression, hasVolatileRowOrder, inferActiveFormulaKind, inferFormulaKind, nextDuplicateColumnName, quoteIdentifier, quoteLiteral, sqlValueFromInput, stripTerminalSemicolon } from '../src/lib/mutation-sql.ts';
import { columnFormulas } from '../src/lib/column-formulas.ts';

test('quotes SQL identifiers and literals', () => {
  assert.equal(quoteIdentifier('a"b'), '"a""b"');
  assert.equal(quoteLiteral("O'Brien"), "'O''Brien'");
});

test('watches the result type as a column formula changes', () => {
  const columns = [{ name: 'title', type: 'VARCHAR', numeric: false }, { name: 'amount', type: 'DOUBLE', numeric: true }, { name: 'a + b', type: 'VARCHAR', numeric: false }];
  assert.equal(inferFormulaKind('"title"', columns), 'text');
  assert.equal(inferFormulaKind('length("title")', columns), 'number');
  assert.equal(inferFormulaKind('round(length("title"))', columns), 'number');
  assert.equal(inferFormulaKind('cast(length("title") AS VARCHAR)', columns), 'text');
  assert.equal(inferFormulaKind('try_cast("title" AS DOUBLE)', columns), 'number');
  assert.equal(inferFormulaKind('"amount"', columns), 'number');
  assert.equal(inferFormulaKind('"amount" + 2', columns), 'number');
  assert.equal(inferFormulaKind('("amount" + 2) / 100', columns), 'number');
  assert.equal(inferFormulaKind('"amount" + ', columns), 'unknown');
  assert.equal(inferFormulaKind('"amount" >= 2', columns), 'boolean');
  assert.equal(inferFormulaKind('"amount" BETWEEN 1 AND 2', columns), 'boolean');
  assert.equal(inferFormulaKind('"amount" BETWEEN 1 AND 2 AND "title" = \'ok\'', columns), 'boolean');
  assert.equal(inferFormulaKind('"amount" BETWEEN 1 AND ', columns), 'unknown');
  assert.equal(inferFormulaKind('("amount" >= 2) AND TRUE', columns), 'boolean');
  assert.equal(inferFormulaKind('"title" = \'A AND B\'', columns), 'boolean');
  assert.equal(inferFormulaKind('"title" IS NOT NULL', columns), 'boolean');
  assert.equal(inferFormulaKind('"a + b"', columns), 'text');
  assert.equal(inferFormulaKind('"title" || ","', columns), 'unknown');
  assert.equal(inferFormulaKind('"title" || \',\'', columns), 'text');
  assert.equal(inferFormulaKind('power("amount", 2)', columns), 'number');
  assert.equal(inferFormulaKind('left("title", 1)', columns), 'text');
  assert.equal(inferFormulaKind('regexp_extract("title", \'x\')', columns), 'text');
  assert.equal(inferFormulaKind('length(', columns), 'unknown');
  assert.equal(inferActiveFormulaKind('"amount" < 10 AND "amount"', columns), 'number');
  assert.equal(inferActiveFormulaKind('"amount" < 10 AND "title"', columns), 'text');
  assert.equal(inferActiveFormulaKind('"amount" BETWEEN  AND ', columns), 'number');
  assert.equal(inferActiveFormulaKind('"amount" BETWEEN 1 AND ', columns), 'number');
  assert.equal(inferActiveFormulaKind('"amount" BETWEEN 1 AND 2 AND "title"', columns), 'text');
  assert.equal(inferActiveFormulaKind('"title" = \'A AND B\'', columns), 'boolean');
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
  assert.equal(buildSwitchExpression([{ condition: '"status" = \'open\'', thenValue: '1' }, { condition: '"status" <> \'closed\'', thenValue: '2' }], '0'), `CASE WHEN "status" = 'open' THEN 1 WHEN "status" <> 'closed' THEN 2 ELSE 0 END`);
  assert.equal(buildSwitchExpression([{ condition: '', thenValue: '1' }], '0'), '');
});

test('switch conditions compose comparisons, arithmetic, and logic', () => {
  assert.equal(buildSwitchExpression([{ condition: '"amount" < 10 AND length("region") > 2', thenValue: sqlValueFromInput("O'Reilly") }, { condition: '"amount" >= 10', thenValue: sqlValueFromInput('2') }], sqlValueFromInput('NULL')),
    `CASE WHEN "amount" < 10 AND length("region") > 2 THEN 'O''Reilly' WHEN "amount" >= 10 THEN 2 ELSE NULL END`);
  assert.equal(sqlValueFromInput("'already quoted'"), "'already quoted'");
  assert.equal(sqlValueFromInput('001', true), "'001'");
});

test('generated column formulas follow saved versions, rename, copy, and pending edits', () => {
  const history = {
    activeVersionId: 'v2',
    versions: [
      { id: 'v1', changes: [] },
      { id: 'v2', parentId: 'v1', changes: [
        { kind: 'add', details: { column: 'length', expression: 'length("name")' } },
        { kind: 'rename', details: { from: 'length', to: 'name_length' } },
        { kind: 'duplicate', details: { column: 'name_length', copy: 'name_length_2' } },
      ] },
    ],
    pendingChanges: [{ kind: 'modify', details: { column: 'name_length_2', output: 'name_length_2', expression: 'abs("name_length_2")' } }],
  };
  assert.deepEqual({ ...columnFormulas(history) }, {
    name_length: 'length("name")',
    name_length_2: 'abs("name_length_2")',
  });
  assert.equal(
    buildMutationSql('SELECT * FROM sample', ['name', 'name_length', 'name_length_2'], 2, columnFormulas(history).name_length, nextDuplicateColumnName('name_length', ['name', 'name_length', 'name_length_2'])),
    'SELECT "name", "name_length", length("name") AS "name_length_3", "name_length_2" FROM (\nSELECT * FROM sample\n) AS mutation_source',
  );
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
