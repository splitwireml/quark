import type { VersionChange, ViewHistory } from './types';
import { quoteIdentifier } from './mutation-sql.ts';

// Column changes are already stored with each version. Replay the active branch to recover
// which columns were created and the expression the editor should open with.
export function columnFormulas(history: ViewHistory | undefined): Record<string, string> {
  if (!history) return {};
  const formulas: Record<string, string> = Object.create(null);
  const versions = [];
  let version = history.versions.find((item) => item.id === history.activeVersionId);
  while (version && versions.length < history.versions.length) {
    versions.unshift(version);
    version = history.versions.find((item) => item.id === version?.parentId);
  }
  const changes = [...versions.flatMap((item) => item.changes), ...history.pendingChanges];
  for (const change of changes) applyChange(formulas, change);
  return formulas;
}

function applyChange(formulas: Record<string, string>, change: VersionChange) {
  const details = change.details;
  if (!details) return;
  const value = (key: string) => typeof details[key] === 'string' ? details[key] as string : '';
  if (change.kind === 'add' && value('column') && value('expression')) formulas[value('column')] = value('expression');
  if (change.kind === 'modify' && value('column') && value('output') && value('expression')) {
    delete formulas[value('column')];
    formulas[value('output')] = value('expression');
  }
  if (change.kind === 'duplicate' && value('column') && value('copy')) {
    formulas[value('copy')] = formulas[value('column')] ?? quoteIdentifier(value('column'));
  }
  if (change.kind === 'rename' && value('from') && value('to') && formulas[value('from')]) {
    formulas[value('to')] = formulas[value('from')];
    delete formulas[value('from')];
  }
}
