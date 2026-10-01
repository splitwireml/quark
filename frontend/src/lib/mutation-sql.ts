export function quoteIdentifier(value: string): string {
  return `"${value.replace(/"/g, '""')}"`;
}

export function quoteLiteral(value: string): string {
  return `'${value.replace(/'/g, "''")}'`;
}

export type FormulaKind = 'text' | 'number' | 'date' | 'boolean' | 'unknown';

export function inferFormulaKind(expression: string, columns: { name: string; type: string; numeric: boolean }[]): FormulaKind {
  const value = expression.trim();
  const column = columns.find((item) => value === quoteIdentifier(item.name));
  if (column) {
    if (column.numeric) return 'number';
    if (/DATE|TIME/i.test(column.type)) return 'date';
    if (/BOOL/i.test(column.type)) return 'boolean';
    if (/CHAR|TEXT|STRING/i.test(column.type)) return 'text';
  }
  if (/^'(?:[^']|'')*'$/.test(value)) return 'text';
  if (/^-?(?:\d+(?:\.\d*)?|\.\d+)$/.test(value)) return 'number';
  if (/^(?:TRUE|FALSE)$/i.test(value) || /\s+IS\s+(?:NOT\s+)?NULL$/i.test(value)) return 'boolean';
  const not = value.match(/^NOT\s*\(([\s\S]*)\)$/i);
  if (not) return inferFormulaKind(not[1], columns) === 'boolean' ? 'boolean' : 'unknown';

  // Read only operators outside quoted values and parentheses; incomplete formulas stay unknown.
  const operators = [
    [' OR ', 1, 'boolean'], [' AND ', 2, 'boolean'],
    [' <> ', 3, 'compare'], [' >= ', 3, 'compare'], [' <= ', 3, 'compare'], [' = ', 3, 'compare'], [' > ', 3, 'compare'], [' < ', 3, 'compare'],
    [' || ', 4, 'text'], [' + ', 5, 'number'], [' - ', 5, 'number'], [' * ', 6, 'number'], [' / ', 6, 'number'],
  ] as const;
  let quote = '';
  let depth = 0;
  let binary: { index: number; token: string; rank: number; result: string; separator?: number } | null = null;
  let pendingBetween = false;
  for (let index = 0; index < value.length; index++) {
    const character = value[index];
    if (quote) {
      if (character === quote) { if (value[index + 1] === quote) index++; else quote = ''; }
      continue;
    }
    if (character === "'" || character === '"') { quote = character; continue; }
    if (character === '(') { depth++; continue; }
    if (character === ')') { depth--; if (depth < 0) return 'unknown'; continue; }
    if (depth) continue;
    if (pendingBetween) {
      if (value.slice(index, index + 5).toUpperCase() === ' AND ') {
        if (binary?.result === 'between') binary.separator = index;
        pendingBetween = false;
      }
      continue;
    }
    if (value.slice(index, index + 9).toUpperCase() === ' BETWEEN ') {
      if (!binary || 3 <= binary.rank) binary = { index, token: ' BETWEEN ', rank: 3, result: 'between' };
      pendingBetween = true;
      continue;
    }
    for (const [token, rank, result] of operators) {
      if (value.slice(index, index + token.length).toUpperCase() === token && (!binary || rank <= binary.rank)) binary = { index, token, rank, result };
    }
  }
  if (quote || depth) return 'unknown';
  if (binary) {
    if (binary.result === 'between') {
      if (binary.separator === undefined) return 'unknown';
      const left = inferFormulaKind(value.slice(0, binary.index), columns);
      const lower = inferFormulaKind(value.slice(binary.index + binary.token.length, binary.separator), columns);
      const upper = inferFormulaKind(value.slice(binary.separator + 5), columns);
      return left !== 'unknown' && lower !== 'unknown' && upper !== 'unknown' ? 'boolean' : 'unknown';
    }
    const left = inferFormulaKind(value.slice(0, binary.index), columns);
    const right = inferFormulaKind(value.slice(binary.index + binary.token.length), columns);
    if (left === 'unknown' || right === 'unknown') return 'unknown';
    if (binary.result === 'compare') return 'boolean';
    if (binary.result === 'boolean') return left === 'boolean' && right === 'boolean' ? 'boolean' : 'unknown';
    if (binary.result === 'number') return left === 'number' && right === 'number' ? 'number' : 'unknown';
    return 'text';
  }
  if (value.startsWith('(') && value.endsWith(')')) return inferFormulaKind(value.slice(1, -1), columns);
  const call = value.match(/^([a-z_][a-z_0-9]*)\s*\(([\s\S]*)\)$/i);
  if (!call || !call[2].trim() || /,\s*$/.test(call[2])) return 'unknown';
  const functionCall = call[1].toLowerCase();
  if (functionCall === 'cast' || functionCall === 'try_cast') {
    const target = value.match(/\bAS\s+([a-z][a-z0-9_]*)(?:\([^)]*\))?\s*\)$/i)?.[1] ?? '';
    if (/^(?:double|float|decimal|numeric|real|hugeint|bigint|integer|int|smallint|tinyint|utinyint|usmallint|uinteger|ubigint)$/i.test(target)) return 'number';
    if (/^(?:varchar|char|text|string)$/i.test(target)) return 'text';
    if (/^(?:date|time|timestamp|timestamptz)$/i.test(target)) return 'date';
    if (/^(?:bool|boolean)$/i.test(target)) return 'boolean';
  }
  if (['length', 'year', 'month', 'day', 'date_diff', 'abs', 'round', 'sqrt', 'power'].includes(functionCall)) return 'number';
  if (['upper', 'lower', 'trim', 'left', 'right', 'regexp_extract'].includes(functionCall)) return 'text';
  return 'unknown';
}

export function inferActiveFormulaKind(expression: string, columns: { name: string; type: string; numeric: boolean }[]): FormulaKind {
  const result = inferFormulaKind(expression, columns);
  if (result !== 'unknown') return result;
  const range = expression.match(/^(.*)\s+BETWEEN\s+(.*?)\s+AND\s*$/i);
  if (range && !/\s+AND\s+/i.test(range[2])) return inferActiveFormulaKind(range[1], columns);
  // ponytail: this recognizes a trailing operand after AND/OR; use a SQL parser if nested completions become necessary.
  return inferFormulaKind(expression.trim().split(/\s+(?:AND|OR)\s+/i).pop() ?? '', columns);
}

export function sqlValueFromInput(value: string, asText = false): string {
  const text = value.trim();
  if (!text) return '';
  return /^'(?:[^']|'')*'$/.test(text) || (!asText && (/^-?(?:\d+(?:\.\d*)?|\.\d+)$/.test(text) || /^(?:TRUE|FALSE|NULL)$/i.test(text))) ? text : quoteLiteral(text);
}

export function stripTerminalSemicolon(value: string): string {
  return value.trim().replace(/;(?=\s*(?:(?:--[^\n]*(?:\n|$))|(?:\/\*[\s\S]*?\*\/))*\s*$)/, '').trim();
}

export function hasVolatileRowOrder(value: string): boolean {
  return /\b(?:random|uuid(?:v4)?|gen_random_uuid)\s*\(/i.test(value) || /\b(?:using\s+sample|tablesample)\b/i.test(value);
}

export function buildMutationSql(sourceSql: string, columns: string[], insertIndex: number, expression: string, alias: string): string {
  const source = stripTerminalSemicolon(sourceSql);
  const formula = expression.trim();
  const name = alias.trim();
  if (!source || !formula || !name || !Number.isInteger(insertIndex) || insertIndex < 0 || insertIndex > columns.length) return '';
  const select = columns.map(quoteIdentifier);
  select.splice(insertIndex, 0, `${formula} AS ${quoteIdentifier(name)}`);
  return `SELECT ${select.join(', ')} FROM (\n${source}\n) AS mutation_source`;
}

export function buildColumnReplacementSql(sourceSql: string, columns: string[], targetColumn: string, expression: string, alias: string): string {
  const index = columns.indexOf(targetColumn);
  const remaining = columns.filter((_, columnIndex) => columnIndex !== index);
  const name = alias.trim().toLowerCase();
  if (index < 0 || remaining.some((column) => column.toLowerCase() === name)) return '';
  return buildMutationSql(sourceSql, remaining, index, expression, alias);
}

export function nextDuplicateColumnName(name: string, columns: string[]): string {
  const existing = new Set(columns.map((column) => column.toLowerCase()));
  let suffix = 2;
  while (existing.has(`${name}_${suffix}`.toLowerCase())) suffix++;
  return `${name}_${suffix}`;
}

export function buildIfExpression(condition: string, thenValue: string, elseValue: string): string {
  const operands = [condition, thenValue, elseValue].map((value) => value.trim());
  return operands.every(Boolean) ? `CASE WHEN ${operands[0]} THEN ${operands[1]} ELSE ${operands[2]} END` : '';
}

export function buildSwitchExpression(cases: { condition: string; thenValue: string }[], elseValue: string): string {
  const fallback = elseValue.trim();
  const branches = cases.map((item) => ({ condition: item.condition.trim(), thenValue: item.thenValue.trim() }));
  return fallback && branches.length && branches.every((item) => item.condition && item.thenValue)
    ? `CASE ${branches.map((item) => `WHEN ${item.condition} THEN ${item.thenValue}`).join(' ')} ELSE ${fallback} END`
    : '';
}

export function buildCellEditSql(sourceSql: string, columns: string[], rowNumber: number, targetColumn: string, value: string): string {
  const source = stripTerminalSemicolon(sourceSql);
  const normalizedColumns = columns.map((column) => column.toLowerCase());
  if (!source || !columns.length || new Set(normalizedColumns).size !== columns.length || !Number.isSafeInteger(rowNumber) || rowNumber < 1 || !columns.includes(targetColumn)) return '';
  let rowAlias = '__quark_row_number';
  for (let suffix = 2; normalizedColumns.includes(rowAlias.toLowerCase()); suffix++) rowAlias = `__quark_row_number_${suffix}`;
  const target = quoteIdentifier(targetColumn);
  const select = columns.map((column) => column === targetColumn
    ? `CASE WHEN ${quoteIdentifier(rowAlias)} = ${rowNumber} THEN cast_to_type(${quoteLiteral(value)}, ${target}) ELSE ${target} END AS ${target}`
    : quoteIdentifier(column));
  return `SELECT ${select.join(', ')} FROM (\nSELECT *, row_number() OVER () AS ${quoteIdentifier(rowAlias)} FROM (\n${source}\n) AS cell_edit_source\n) AS cell_edit_numbered`;
}
