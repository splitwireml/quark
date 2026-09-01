export function quoteIdentifier(value: string): string {
  return `"${value.replace(/"/g, '""')}"`;
}

export function quoteLiteral(value: string): string {
  return `'${value.replace(/'/g, "''")}'`;
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
