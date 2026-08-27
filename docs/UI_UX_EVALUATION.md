# DuckScope UI and UX Evaluation

**Reviewed:** 2026-07-13  
**Surfaces:** live application, desktop 1440 x 900, tablet 900 x 900, mobile 390 x 844  
**Code:** `frontend/src/App.svelte`, `frontend/src/app.css`, `docs/SPEC.md`

## Executive verdict

DuckScope has a sound functional skeleton: the table is fast, dense, server-paged, and backed by explicit loading, empty, and error states. The problem is not capability. The interface compresses capable behavior into text and controls that are too small, repeats navigation and connection concepts, and reveals advanced filtering in a way that nearly removes the table from view.

**Design health:** **24/40, Acceptable**  
**Technical UI audit:** **9/20, Poor**  
**Recommendation:** overhaul the presentation and interaction shell while preserving the backend contract and existing query functions.

## Evidence from live verification

- No browser console errors or page errors occurred during load, numeric profiling, or category-filter inspection.
- At 390 px, the document remains **720 px wide**. The left rail consumes 200 px and the main workspace starts at x=200, so core content is clipped off-screen.
- Computed minimum font size is **8 px** at desktop, tablet, and mobile.
- The loaded 100-row screen contains **983 rendered elements below 12 px** because the same tiny table typography repeats across cells and headers.
- **22 of 24 visible controls** measure below 44 px in at least one dimension.
- Source scan via `npx impeccable detect --json frontend/src/App.svelte` returned no findings. The live detector timed out while capturing the unusually tall table and also returned no findings. Treat that as a detector blind spot, not a clean bill of health.

## What works

1. **The core table model is right.** Sticky headers, a sticky first column, server paging, row-range feedback, multi-sort state, and null styling support real inspection work.
2. **System status is present.** Uploading, querying, category loading, statistics loading, empty results, and API failures all have explicit UI states.
3. **The implementation is lean.** The frontend has no runtime UI dependencies, no console errors, and no unnecessary client-side data grid abstraction.
4. **Data-quality features are valuable.** Nullity gauges and on-demand numeric statistics are meaningful differentiators, not decorative dashboard content.

## Nielsen heuristic score

| # | Heuristic | Score | Key issue |
|---|---|---:|---|
| 1 | Visibility of system status | 3/4 | Good loading and connection feedback; refresh state can still be clearer. |
| 2 | Match between system and real world | 2/4 | “Running nodes,” schema/type codes, and symbolic controls assume technical fluency. |
| 3 | User control and freedom | 3/4 | Filter cancel, removable chips, modal close, and page jump work; no clear-all or query reset action. |
| 4 | Consistency and standards | 3/4 | Components are visually consistent; numeric profiling uses a hidden interaction that differs from other column actions. |
| 5 | Error prevention | 3/4 | Invalid actions are disabled and pagination is clamped; attach-path errors are only discovered after submit. |
| 6 | Recognition rather than recall | 2/4 | Tiny symbol-only sort/filter actions and click-to-profile require guessing. |
| 7 | Flexibility and efficiency | 2/4 | Multi-sort and direct page jump help experts; there are no keyboard accelerators or focused table navigation. |
| 8 | Aesthetic and minimalist design | 2/4 | The table is focused, but duplicate node navigation and permanent setup controls waste space. |
| 9 | Error recognition and recovery | 3/4 | Errors include Retry; messages could be more specific to source, query, or filter context. |
| 10 | Help and documentation | 1/4 | File formats are hinted; advanced column behavior has no contextual explanation. |
| **Total** |  | **24/40** | **Acceptable, significant improvement needed.** |

## Cognitive load

The default browse state has moderate load. The open text-filter state is high load, with **5 of 8 checklist failures**:

- No single focus: source setup, query state, filter building, and table inspection compete.
- More than four choices appear in the operator list.
- Category selection and advanced filtering are shown simultaneously.
- The table is compressed to its 180 px minimum while filtering.
- Progressive disclosure stops halfway: the editor opens contextually, then exposes both simple and advanced modes at once.

Grouping, visible query chips, and preserved column context are good. The overhaul should keep those strengths and sequence the decisions.

## Priority issues

### P1: Mobile is structurally unusable

**Where:** `frontend/src/app.css`, `body { min-width: 720px; overflow: hidden; }`, permanent 200 px rail below 950 px.

**Impact:** A 390 px viewport shows the source rail and roughly half of the data workspace. Pagination and later columns are inaccessible without page-level overflow that the body itself hides.

**Fix:** Remove the body minimum width. Convert the rail to a drawer below 960 px, keep table-level horizontal scrolling, and turn the inspector into a full-width overlay on tablet and mobile screens.

### P1: Density has become illegibility

**Where:** `frontend/src/app.css`, widespread 8, 9, 10, and 11 px text; 24 to 34 px controls.

**Impact:** Users scanning data for long sessions must work harder, low-vision users lose access, and touch users receive undersized targets. Compact does not need to mean microscopic.

**Fix:** Raise UI text to 14 px, metadata and table text to 12 px minimum, row height to 34 to 36 px, pointer controls to 36 px, and touch controls to 44 px.

### P1: The filter editor displaces the task

**Where:** `App.svelte:403-433`, `.filter-editor` and `.category-picker`.

**Impact:** Opening a category filter creates a 289 px editor and shrinks the table to 180 px. The user loses the visual context needed to decide which filter to apply.

**Fix:** Move filtering to a right-side inspector. Lead with searchable category values. Put operator/value controls behind an “Advanced condition” disclosure. Keep the table and active query tokens visible.

### P2: Navigation and status are duplicated

**Where:** `App.svelte:334-360`.

**Impact:** The same nodes appear in both the left rail and top tabs. “Local” and “Backend connected” repeat the same reassurance. This costs vertical and horizontal space without adding orientation.

**Fix:** Keep source navigation in one rail. Replace the top node tabs with a compact breadcrumb or current-source label. Show one connection status in the rail footer.

### P2: Source setup competes with routine analysis

**Where:** `App.svelte:318-332`.

**Impact:** Upload and attach-path controls permanently occupy the prime top of the rail even after a source is open. A rare setup task visually outranks the frequent browse/filter/profile task.

**Fix:** Use one “Add source” button that reveals upload and attach options on demand. Keep existing sources visible.

### P2: Column actions are under-signaled

**Where:** `App.svelte:443-456`; numeric profile is triggered from the column title while sort/filter use tiny symbols.

**Impact:** Users must discover that numeric headers behave differently. The glyphs `↕` and `⌕` are small and visually weak.

**Fix:** Use one consistent column-action group with labeled tooltips and Material Symbols. Expose “Profile column” explicitly for numeric fields.

### P2: Statistics are readable but not diagnostic enough

**Where:** `App.svelte:488-508`.

**Impact:** Eight identical metric cards flatten hierarchy, and the histogram shows only minimum and maximum labels. Counts and bin ranges require hover, which is unavailable on touch.

**Fix:** Use a side inspector with a compact definition list, lead with null rate and distribution, add y-axis context or a visible selected-bin readout, and keep the table behind it.

### P3: The visual identity is generic

**Where:** global dark theme, Inter, green accent, uniform outlined panels.

**Impact:** The UI reads as a competent developer dashboard, not a distinctive local analytical instrument.

**Fix:** Use the restrained daylight direction in `DESIGN.md`: warm canvas, ink typography, green reserved for action/state, amber for data-quality signals, ruled table structure, and rectilinear controls.

## Technical audit score

| Dimension | Score | Key finding |
|---|---:|---|
| Accessibility | 2/4 | Good semantics and focus rules, but tiny text, undersized targets, and icon-only discovery gaps remain. |
| Performance | 4/4 | Lean Svelte UI, server paging, no runtime errors, no expensive decorative motion. |
| Responsive design | 0/4 | Fixed 720 px minimum width and permanent rail break mobile layout. |
| Theming | 1/4 | A few root tokens, but 81 unique hard-coded hex colors and 110 hex occurrences prevent systematic tuning. |
| Anti-patterns | 2/4 | Generic dark developer-tool styling, repeated metric cards, tiny controls, and permanent setup chrome. |
| **Total** | **9/20** | **Poor, major overhaul warranted.** |

## Persona red flags

### Power user: data analyst

- Can multi-sort and jump pages, but cannot operate column actions efficiently from the keyboard.
- The filter builder interrupts row comparison instead of supporting it beside the table.
- Tiny type reduces scan speed during long sessions.

### First-time technical user

- “Node,” “MAIN · VIEW,” null gauges, and clickable numeric headers have no explanation.
- Two locations for the same source make orientation less certain.
- Symbol-only column actions require trial and error.

### Tablet user

- The rail remains 200 px wide, leaving 672 px for the table.
- Nearly every control remains below recommended touch size.
- A filter editor or statistics dialog consumes most of the available workspace.

## Direction

The right move is not a feature rewrite. Keep the API, state, and query behavior. Recompose the shell around three layers:

1. **Sources:** one collapsible rail with progressive source setup.
2. **Data canvas:** dominant table, explicit query summary, consistent column actions.
3. **Context inspector:** filters and profiles in one reusable right-side region.

See `docs/plans/2026-07-13-ui-overhaul.md` for the implementation sequence and `docs/ui-overhaul-prototype/index.html` for the plain HTML/CSS/JavaScript vision.
