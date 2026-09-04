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
  const metricSelect = effective.reduce<string[]>((all, { column, metrics }) => all.concat(metrics.map((metric) => {
    const expression = expressions[metric].replace('{column}', quoteIdentifier(column));
    const alias = effective.length === 1 ? metric : `${column} ${metric}`;
    return `${expression} AS ${quoteIdentifier(alias)}`;
  })), []);
  const select = [...indexes.map(quoteIdentifier), ...metricSelect];
  const groupBy = indexes.length ? ` GROUP BY ${indexes.map(quoteIdentifier).join(', ')}` : '';
  const orderBy = indexes.length ? ` ORDER BY ${quoteIdentifier(indexes[0])}` : '';
  return `SELECT ${select.join(', ')} FROM (${source}) AS filtered${groupBy}${orderBy}`;
}
