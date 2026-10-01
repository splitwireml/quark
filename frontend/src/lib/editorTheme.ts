import { HighlightStyle, syntaxHighlighting } from '@codemirror/language';
import { EditorView } from '@codemirror/view';
import { tags } from '@lezer/highlight';

/* CodeMirror ships its own light colours, which would leave a white block sitting in the
   middle of a dark workspace. Everything here resolves to workspace tokens instead, so the
   editor follows the scheme without knowing which one is on. */
export const editorTheme = EditorView.theme({
  '&': { color: 'var(--ink)', backgroundColor: 'var(--surface)' },
  '.cm-content': { caretColor: 'var(--action)', fontFamily: 'var(--font-mono)' },
  '.cm-cursor, .cm-dropCursor': { borderLeftColor: 'var(--action)' },
  '&.cm-focused .cm-selectionBackground, .cm-selectionBackground, .cm-content ::selection':
    { backgroundColor: 'var(--action-tint)' },
  '.cm-activeLine': { backgroundColor: 'var(--surface-inset)' },
  '.cm-gutters': { backgroundColor: 'var(--surface-2)', color: 'var(--faint)', borderRight: '1px solid var(--line)' },
  '.cm-activeLineGutter': { backgroundColor: 'var(--surface-hover)', color: 'var(--muted)' },
  '.cm-selectionMatch': { backgroundColor: 'var(--action-tint)' },
  '.cm-matchingBracket, .cm-nonmatchingBracket': { backgroundColor: 'var(--action-tint)', outline: '1px solid var(--action-tint-border)' },
  '.cm-panels': { backgroundColor: 'var(--surface-2)', color: 'var(--ink)' },
  '.cm-searchMatch': { backgroundColor: 'var(--action-tint)', outline: '1px solid var(--action-tint-border)' },
  '.cm-tooltip': { backgroundColor: 'var(--surface)', border: '1px solid var(--line-strong)', borderRadius: 'var(--radius-xl)', boxShadow: 'var(--shadow-popover)' },
  '.cm-tooltip.cm-tooltip-autocomplete > ul > li': { fontFamily: 'var(--font-mono)', color: 'var(--ink)' },
  '.cm-tooltip.cm-tooltip-autocomplete > ul > li[aria-selected]': { backgroundColor: 'var(--action-tint)', color: 'var(--action-dark)' },
  '.cm-completionIcon': { color: 'var(--glyph)' }
});

const highlight = HighlightStyle.define([
  { tag: [tags.keyword, tags.operatorKeyword, tags.modifier], color: 'var(--code-keyword)' },
  { tag: [tags.string, tags.special(tags.string)], color: 'var(--code-string)' },
  { tag: [tags.number, tags.bool, tags.null], color: 'var(--code-number)' },
  { tag: [tags.name, tags.propertyName, tags.variableName], color: 'var(--code-name)' },
  { tag: [tags.function(tags.variableName), tags.typeName], color: 'var(--code-name)', fontWeight: '500' },
  { tag: [tags.comment, tags.lineComment, tags.blockComment], color: 'var(--code-comment)', fontStyle: 'italic' },
  { tag: [tags.operator, tags.punctuation, tags.separator], color: 'var(--muted)' },
  { tag: tags.invalid, color: 'var(--error)' }
]);

export const editorHighlight = syntaxHighlighting(highlight);
