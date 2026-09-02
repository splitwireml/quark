export type AggregateMetric = 'count' | 'distinct' | 'min' | 'max' | 'sum' | 'avg' | 'median' | 'stddev';

const expressions: Record<AggregateMetric, string> = {
  count: 'count({column})',
  distinct: 'count(DISTINCT {column})',
  min: 'min({column})',
  max: 'max({column})',
  sum: 'sum({column})',
  avg: 'avg({column})',
  median: 'median({column})',
  stddev: 'stddev_samp({column})'
};

function quoteIdentifier(value: string): string { return `"${value.replace(/"/g, '""')}"`; }

export function buildAggregateSql(
  sourceSql: string,
  indexes: string[],
  aggregates: { column: string; metrics: AggregateMetric[] }[]
): string {
  const source = sourceSql.trim().replace(/;$/, '');
  const effective = aggregates.filter(({ metrics }) => metrics.length > 0);
  if (!source || effective.length === 0) return '';
  const distribution = indexes.length === 0 && effective.length === 1 && effective[0].metrics.length === 1 && effective[0].metrics[0] === 'count';
  const groupIndexes = distribution ? [effective[0].column] : indexes;
  const metricSelect = effective.reduce<string[]>((all, { column, metrics }) => all.concat(metrics.map((metric) => {
    const expression = distribution ? 'count(*)' : expressions[metric].replace('{column}', quoteIdentifier(column));
    const alias = effective.length === 1 ? metric : `${column} ${metric}`;
    return `${expression} AS ${quoteIdentifier(alias)}`;
  })), []);
  const select = [...groupIndexes.map(quoteIdentifier), ...metricSelect];
  const groupBy = groupIndexes.length ? ` GROUP BY ${groupIndexes.map(quoteIdentifier).join(', ')}` : '';
  const orderBy = groupIndexes.length ? ` ORDER BY ${quoteIdentifier(groupIndexes[0])}` : '';
  return `SELECT ${select.join(', ')} FROM (${source}) AS filtered${groupBy}${orderBy}`;
}
