# DuckScope Design Direction

## Scene and strategy

A data analyst works in daylight on a laptop, scanning thousands of values for an hour without glare or visual noise. Use a **light, restrained** product surface with an ink-and-paper character, one green action accent, and amber only for data-quality warnings.

## Visual anchors

- Linear for disciplined information hierarchy.
- TablePlus for database navigation and compact data controls.
- Laboratory instrument panels for precise labels, ruled structure, and restrained status color.

## Typography

- UI: `-apple-system, BlinkMacSystemFont, "Segoe UI", system-ui, sans-serif`.
- Data: `ui-monospace, "SFMono-Regular", Consolas, monospace`.
- Default UI text: 14 px. Dense metadata: 12 px minimum. Page title: 22 px.
- Use weight and spacing before adding more colors.

## Color tokens

- Canvas: `oklch(97.5% 0.008 88)`
- Surface: `oklch(99% 0.004 88)`
- Raised surface: `oklch(95.5% 0.01 88)`
- Ink: `oklch(24% 0.022 155)`
- Muted ink: `oklch(49% 0.018 155)`
- Rule: `oklch(84% 0.014 88)`
- Action: `oklch(45% 0.11 155)`
- Action tint: `oklch(92% 0.04 155)`
- Warning: `oklch(56% 0.12 62)`
- Error: `oklch(48% 0.16 28)`

## Geometry and rhythm

- 4 px corner radius for controls and panels; 2 px for tags.
- No pills. Conditions are rectilinear tokens with full borders.
- 4 px base spacing scale: 4, 8, 12, 16, 24, 32.
- Desktop source rail: 252 px. Context inspector: 320 to 360 px.
- Keep the table flush and dominant. Avoid nested cards.

## Interaction model

- Source management lives behind one **Add source** action. Existing sources stay visible in the rail.
- Column headers expose sort, filter, and profile actions consistently. Numeric profiling is not hidden behind clicking the column name.
- Filters and column profiles use a right-side inspector on wide screens so table context remains visible.
- At narrow widths the source rail becomes a drawer, the inspector becomes a full-width overlay, and the table retains horizontal scrolling.
- Controls are at least 36 px on pointer-first screens and 44 px on touch breakpoints.
- Motion is limited to 180 ms opacity and transform transitions, with reduced-motion support.

## States

Every asynchronous surface needs default, loading, empty, error, success, disabled, hover, focus, and active states. Use skeleton rows for table loading, preserve current data during refresh, and pair errors with a specific recovery action.

## Iconography

Use Google Material Symbols Outlined consistently. Important actions keep text labels; icon-only table actions require accessible labels and tooltips.
