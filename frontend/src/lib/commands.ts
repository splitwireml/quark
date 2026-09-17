import type { IconName } from './icons';

export type ToolbarVisibility = 'show' | 'hover' | 'hide';
export type ActionMenuMode = 'simple' | 'comprehensive';
export const actionMenuStorageKey = 'quark.action-menu';
export type CommandPrefix = 'find' | 'column' | 'sidebar' | null;
export const combinationTimeoutMs = 3000;
export const toolbarStorageKey = 'quark.toolbar-visibility';
export const operations: { id: string; label: string; icon: IconName; key: string; direction?: string }[] = [
  { id: 'aggregate', label: 'Aggregate', icon: 'sigma', key: 'a', direction: '↑' },
  { id: 'joins', label: 'Join', icon: 'join', key: 'j', direction: '←' },
  { id: 'columns', label: 'Columns', icon: 'columns', key: 'c', direction: '↓' },
  { id: 'dedupe', label: 'Dedupe', icon: 'duplicate', key: 'd', direction: '→' },
  { id: 'find-column', label: 'Find column', icon: 'list', key: 'f' },
  { id: 'density', label: 'Row spacing', icon: 'spacing', key: 'r' },
  { id: 'fit', label: 'Fit columns', icon: 'fit-columns', key: 'w' },
  { id: 'sql', label: 'SQL', icon: 'braces', key: 's' },
];
export function operationsFor(mode: ActionMenuMode) {
  return mode === 'simple' ? operations.filter(({ id }) => id !== 'sql' && id !== 'find-column') : operations;
}
export const shortcuts = [
  ['⌘A', 'Action wheel', 'wheel'], ['⌘F → C', 'Find column', 'find-column'],
  ['⌘F → F', 'Find cell values', 'find-values'], ['⌘E → F', 'Filter column', 'filter'],
  ['⌘E → S', 'Sort column', 'sort'], ['⌘E → H', 'Hide column', 'hide'],
  ['⌘E → P', 'Pin / unpin column', 'pin'], ['⌘B → B', 'Toggle sidebar', 'sidebar'],
  ['⌘B → T', 'Choose source by number', 'sources'], ['⌘⇧V', 'Versions', 'versions'],
  ['⌘R', 'Refresh View', 'refresh'], ['⌘S', 'Save Version', 'save'],
  ['⌘Z', 'Undo', 'undo'], ['⌘⇧Z', 'Redo', 'redo'], ['⌘,', 'Settings', 'settings'],
  ['?', 'Command reference', 'help'],
];
export function commandFor(event: { key: string; metaKey: boolean; ctrlKey: boolean; altKey: boolean; shiftKey: boolean; isComposing?: boolean }, prefix: CommandPrefix, editable: boolean): string | null {
  if (event.isComposing || event.altKey) return null;
  const key = event.key.toLowerCase();
  const mod = event.metaKey || event.ctrlKey;
  if (editable) return null;
  if (prefix) {
    if (['shift', 'meta', 'control', 'alt'].includes(key)) return null;
    const choices: Record<string, Record<string, string>> = {
      find: { c: 'find-column', f: 'find-values' },
      column: { f: 'filter', s: 'sort', h: 'hide', p: 'pin' },
      sidebar: { b: 'sidebar', t: 'sources' },
    };
    return choices[prefix][key] ?? 'cancel';
  }
  if (!mod) return key === '?' ? 'help' : null;
  if (event.shiftKey) return key === 'v' ? 'versions' : key === 'z' ? 'redo' : null;
  return ({ a: 'wheel', f: 'prefix-find', e: 'prefix-column', b: 'prefix-sidebar', r: 'refresh', s: 'save', z: 'undo', ',': 'settings' } as Record<string, string>)[key] ?? null;
}
