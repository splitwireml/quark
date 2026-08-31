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

## Color tokens

- Canvas: `#EDF0F4`
- Surface: `#FFFFFF`
- Surface (toolbar/titlebar): `#F4F6F9`
- Surface (rail): `#FAFBFD`
- Surface (inset panels): `#F7F9FC`
- Ink: `#1F2533`
- Muted ink: `#5A6673` / `#7A8593`
- Rule: `#E6EAF0`; strong rule (card border): `#D5DBE4`; control border: `#D7DEE7`
- Action (accent): `#1155F5`; action dark: `#0B3FBF`; action tint: `#EDF3FF`
- Warning (null %): `#B87415`
- Success (connection, timing): `#12B981`
- Error: `#FF5F57`
- Dark fill (primary buttons): `#1F2533`

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
