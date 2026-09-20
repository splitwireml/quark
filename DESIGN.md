# DuckScope Design Direction

## Scene and strategy

A data analyst works in daylight on a laptop, scanning thousands of values for an hour without glare or visual noise. Use a **light, restrained** product surface with an instrument-panel character, one blue action accent, and amber only for data-quality warnings.

## Visual anchors

- Linear for disciplined information hierarchy.
- TablePlus for database navigation and compact data controls.
- Laboratory instrument panels for precise labels, ruled structure, and restrained status color.

## Typography

- UI: `'IBM Plex Sans', system-ui, sans-serif` (Google Fonts, weights 400/500/600).
- Data: `'IBM Plex Mono', ui-monospace, 'SFMono-Regular', Consolas, monospace` (weights 400/500) — column names, badges, kbd hints, SQL, counts, type labels.
- Default UI text: 12.5–14 px. Dense metadata: 9–11px. Page title: 22px.
- Use weight and spacing before adding more colors.
- Text has exactly three levels — ink, muted, faint — and each clears 4.5:1 on
  every surface listed above, in both schemes. A UI has room for about three
  AA-passing greys; anything meant to recede further does so through size,
  weight, or the mono face, never a fourth grey. `tests/theme.test.js` parses
  the tokens out of `app.css` and holds both schemes to this.

## Color tokens

- Canvas: `#EDF0F4`
- Surface: `#FFFFFF`
- Surface (toolbar/titlebar): `#F4F6F9`
- Surface (rail): `#FAFBFD`
- Surface (inset panels): `#F7F9FC`
- Ink: `#1F2533`
- Muted ink (secondary text): `#515D6B` — 5.87:1 on canvas
- Faint ink (tertiary text, metadata, eyebrows, placeholders): `#616E80` — 4.54:1 on canvas
- Glyph (decorative icon characters, non-text 3:1): `#7D8C9E`
- Disabled control text (exempt from contrast minimums): `#A8B2BE`
- Rule: `#E6EAF0`; strong rule (card border): `#D5DBE4`; control border: `#D7DEE7`
- Action (accent): `#1155F5`; action dark: `#0B3FBF`; action tint: `#EDF3FF`
- Warning (null %): `#B87415`
- Success (connection, timing): `#12B981`
- Error: `#FF5F57`
- Dark fill (primary buttons): `#1F2533`

## Dark scheme

- The product ships both schemes. The preference is **Match system / Light / Dark**, stored under `quark.color-scheme` and resolved in `index.html` before first paint so a chosen dark never opens as a white window. `Match system` follows `prefers-color-scheme` live; an explicit choice outranks the system for good.
- Dark redefines the same token names under `:root[data-theme='dark']` — no scheme-specific component CSS, and no second set of names to keep in step.
- Canvas: `#101319`; Surface: `#191E27`; toolbar/titlebar: `#161A22`; rail: `#13171E`; inset panels: `#151920`; hover: `#222834`.
- Ink: `#E9EDF3`; secondary: `#D5DCE6`; muted: `#A6B2C1`; faint: `#93A0B1`; glyph: `#78879A`; disabled: `#5C6776`.
- Rule: `#272E3A`; strong rule: `#343D4B`; control border: `#3A4451`.
- Action: `#5C8DFF`; action light (text on tint): `#A6C2FF`; action tint: `#17233C`; warning: `#E3A24B`; success: `#34D399`; error: `#FF7A73`.
- Filled buttons invert: `--ink-fill` is light and `--on-fill` is dark, so a primary button stays the highest-contrast control in either scheme.
- Charts keep their palette identity across schemes and change only lightness: every palette carries a `dark` counterpart in `chartThemes.ts`, and the resolved scheme picks one. The test asserts each dark colour stands off the dark surface at least as well as its light original does off white.
- SQL syntax is themed from the same tokens (`--code-keyword`, `--code-string`, `--code-number`, `--code-name`, `--code-comment`); CodeMirror's own light colours are never used.
- Switching scheme crossfades the workspace through a 180ms view transition rather than blanking it, and switches instantly under `prefers-reduced-motion`.

## Geometry and rhythm

- Radii: 3px (small chips/tags) → 4–5px (controls) → 7–8px (floating popovers) → 12px (app window card).
- No pills for buttons. Conditions are rectilinear tokens with full borders.
- 4px base spacing scale: 4, 8, 12, 16, 20, 24, 32.
- Desktop source rail: 212px. Context inspector: 368px (320px narrow).
- Keep the table flush and dominant. Avoid nested cards.

## Interaction model

- Source management lives behind one **Add source** action. Existing sources stay visible in the rail.
- Column headers expose sort, filter, and profile actions consistently. Numeric profiling is not hidden behind clicking the column name.
- Filters and column profiles use a right-side inspector on wide screens so table context remains visible.
- At narrow widths the source rail becomes a drawer, the inspector becomes a full-width overlay, and the query bar wraps instead of scrolling.
- Controls are at least 26–30px on pointer-first screens.
- Motion is limited to short opacity/transform transitions, with reduced-motion support.

## States

Every asynchronous surface needs default, loading, empty, error, success, disabled, hover, focus, and active states. Use skeleton/spinner states for table loading, preserve current data during refresh, and pair errors with a specific recovery action.

## Iconography

Plain glyph characters (▾ ⌕ × ↑ ↓ ✓ ⌄ ↕ ◧), no icon font. Important actions keep text labels; icon-only table actions require accessible labels and tooltips.

## Component architecture

`frontend/src/components/` follows atomic design: `atoms/` (Button, Chip, Checkbox, StatusDot, TextInput, Eyebrow, NullGauge), `molecules/` (SourceTreeItem, DatasetTab, PaginationControl, ColumnHeaderCell, DistributionRow, HistogramChart, SavedQueryCard/ListItem, FilterOperatorForm), `organisms/` (TitleBar, SourceRail, QueryConditionBar + its menu popovers, DataGridTable, InspectorPanel + FilterInspector/ProfileInspector, SqlEditorPanel, WorkbookDialog, ...), `templates/` (AppShell). `App.svelte` owns all state and API calls; components are presentational, wired through callback props.
