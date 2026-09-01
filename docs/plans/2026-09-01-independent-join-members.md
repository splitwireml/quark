# Independent Join Members

**Goal:** Let both sides of a join independently choose any active source and one of its datasets, while keeping the join menu inside the viewport.

## Task 1 — State and flow

**Files:** `frontend/src/App.svelte`

- Replace the current-dataset-anchored left side with explicit left/right source IDs, dataset lists, selected dataset IDs, loading state, and request generations.
- Default the left member to the current source/dataset for convenience, but allow changing it freely.
- Preview and SQL generation must use the two selected member identities, even when neither is the current table.
- Reset keys/output columns when either member changes; seed the first common key once both datasets exist.

## Task 2 — Two member selectors

**Files:** `frontend/src/components/organisms/JoinMenuPopover.svelte`

- Render two equal member cards.
- Each card has a custom source dropdown followed by tightly hugged dataset/sheet pill buttons.
- Keep key, output-column, preview, Cartesian-risk, and run controls unchanged below the members.
- Reuse `SelectDropdown` and `Button`; no native selects or new dependency.

## Task 3 — Containment

**Files:** `frontend/src/components/organisms/JoinMenuPopover.svelte`, `frontend/src/components/molecules/SelectDropdown.svelte`, `frontend/src/components/molecules/MultiSelectDropdown.svelte`

- Anchor the join panel to the query bar rather than the trigger so it remains within the workspace viewport.
- Ellipsize long selected labels and constrain dropdown content instead of growing the grid or screen.

## Acceptance

- A join can be configured between two sources that are both different from the currently open source.
- Both source selectors show their datasets as compact pills underneath.
- Selected source/dataset names never widen the panel beyond the viewport.
- Existing cardinality preview, Cartesian marker, composite keys, run, paging, filtering, and profiling still work.
- `npm run check`, frontend tests, production build, backend tests, and a live browser smoke pass.
