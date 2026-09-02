type JoinDataset = { name: string; schema?: string; sql?: string };
export type JoinKey = { left: string; right: string };

function quoteIdentifier(value: string): string { return `"${value.replace(/"/g, '""')}"`; }
// ponytail: duplicated for direct Node TS tests; share when TS-extension imports are enabled.
function stripTerminalSemicolon(value: string): string { return value.trim().replace(/;(?=\s*(?:(?:--[^\n]*(?:\n|$))|(?:\/\*[\s\S]*?\*\/))*\s*$)/, '').trim(); }
function relation(view: JoinDataset): string {
  if (!view.sql) return `${quoteIdentifier(view.schema ?? '')}.${quoteIdentifier(view.name)}`;
  const sql = stripTerminalSemicolon(view.sql);
  return `(${sql}${/--[^\n]*$/.test(sql) ? '\n' : ''})`;
}

export function buildJoinSql(left: JoinDataset, right: JoinDataset, keys: JoinKey[], leftColumns: string[], rightColumns: string[]): string {
  const pairs = keys.map(({ left, right }) => `${left}\0${right}`);
  if ((!left.sql && !left.schema) || (!right.sql && !right.schema) || !keys.length || keys.some(({ left, right }) => !left || !right) || new Set(pairs).size !== pairs.length || !leftColumns.length && !rightColumns.length) return '';

  const collisions = new Set(leftColumns.filter((column) => rightColumns.includes(column)));
  const prefix = (dataset: JoinDataset) => left.name === right.name && dataset.schema ? `${dataset.schema}.${dataset.name}` : dataset.name;
  const used = new Set<string>();
  const select = (side: 'left' | 'right', dataset: JoinDataset, columns: string[]) => columns.map((column) => {
    const source = `${quoteIdentifier(side)}.${quoteIdentifier(column)}`;
    const base = collisions.has(column) ? `${prefix(dataset)}.${column}` : column;
    let output = base;
    for (let suffix = 2; used.has(output); suffix++) output = `${base} ${suffix}`;
    used.add(output);
    return output === column ? source : `${source} AS ${quoteIdentifier(output)}`;
  });
  const on = keys.map((key) => `${quoteIdentifier('left')}.${quoteIdentifier(key.left)} = ${quoteIdentifier('right')}.${quoteIdentifier(key.right)}`).join(' AND ');
  return `SELECT ${[...select('left', left, leftColumns), ...select('right', right, rightColumns)].join(', ')} FROM ${relation(left)} AS "left" INNER JOIN ${relation(right)} AS "right" ON ${on}`;
}
