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

/** The last selected column is aggregated; earlier columns are group-by indexes. */
export function buildAggregateSql(sourceSql: string, columns: string[], metrics: AggregateMetric[]): string {
  const source = sourceSql.trim().replace(/;$/, '');
  const field = columns[columns.length - 1];
  if (!source || !field || metrics.length === 0) return '';
  const distribution = columns.length === 1 && metrics.length === 1 && metrics[0] === 'count';
  const indexes = columns.length > 1 ? columns.slice(0, -1) : distribution ? columns : [];
  const select = [
    ...indexes.map(quoteIdentifier),
    ...metrics.map((metric) => `${metric === 'count' && distribution ? 'count(*)' : expressions[metric].replace('{column}', quoteIdentifier(field))} AS ${quoteIdentifier(metric)}`)
  ];
  const groupBy = indexes.length ? ` GROUP BY ${indexes.map(quoteIdentifier).join(', ')}` : '';
  const orderBy = indexes.length ? ` ORDER BY ${quoteIdentifier(indexes[0])}` : '';
  return `SELECT ${select.join(', ')} FROM (${source}) AS filtered${groupBy}${orderBy}`;
}
