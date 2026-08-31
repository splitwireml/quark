# Cross-Source Join Cardinality Implementation Plan

> **For Hermes:** Use subagent-driven-development skill to implement this plan task-by-task.

**Goal:** Let the join menu select a dataset from any active source, compose multi-column keys with custom atomic dropdowns, and preview relationship/cardinality with a Cartesian-risk marker before running.

**Architecture:** Add one single-user ephemeral join workspace in FastAPI. Same-node previews reuse the existing connection; cross-node previews mount only the two selected datasets into an in-memory DuckDB connection under isolated schemas and expose that connection through the existing SQL/query/profile paths. The frontend requests an explicit preview, then uses the returned workspace dataset identities with the existing safe join SQL builder.

**Tech Stack:** FastAPI, Pydantic, DuckDB, Svelte 5 Atomic Design, Node test runner, pytest.

---

### Task 1: Lock the join workspace contract with backend tests

**Files:**
- Modify: `tests/test_backend.py`
- Modify: `backend/app.py`

**Contract:**
- `POST /api/join-workspaces`
- Request: `{left:{node_id,dataset}, right:{node_id,dataset}, left_keys:string[], right_keys:string[]}`
- Response: `{node_id,left:{schema,name},right:{schema,name},left_rows,right_rows,output_rows,relationship,cartesian_risk}`
- Empty keys return Cartesian product cardinality and `cartesian_risk:true` but no runnable join.
- Unequal key counts, duplicate/missing columns, nodes, or datasets return 4xx.
- One-to-one, one-to-many, many-to-one, and many-to-many are classified from composite-key uniqueness.

**Steps:**
1. Add failing same-source and cross-source tests using two small DuckDB/XLSX/CSV nodes.
2. Assert composite keys, exact output count, relationship, Cartesian risk, and workspace SQL access.
3. Run targeted tests and observe RED.
4. Implement the minimal workspace/mount helpers and endpoint.
5. Run targeted tests GREEN, then full backend suite.

### Task 2: Add reusable custom dropdown molecules

**Files:**
- Create: `frontend/src/components/molecules/SelectDropdown.svelte`
- Create: `frontend/src/components/molecules/MultiSelectDropdown.svelte`

**Steps:**
1. Build keyboard/focus-safe custom dropdowns with Button and Checkbox atoms.
2. Support outside click, Escape, selected label/count, disabled state, and clear selection.
3. Use no native `<select>` in the join organism.
4. Run `npm run check`.

### Task 3: Upgrade the join organism

**Files:**
- Modify: `frontend/src/components/organisms/JoinMenuPopover.svelte`
- Modify: `frontend/src/App.svelte`
- Modify: `frontend/src/lib/api.ts`
- Modify: `frontend/src/lib/types.ts`

**Steps:**
1. Add right-source and right-dataset custom selectors.
2. Replace key-pair native selects with left/right multi-select dropdowns; selected columns pair by order and must have equal non-zero lengths to run.
3. Add an explicit `Check join` action calling `/api/join-workspaces`.
4. Show left/right rows, expected output rows, relationship, and a subtle amber `Cartesian risk` marker for empty keys or many-to-many duplication.
5. Run the join through the preview's node/dataset identities and existing `buildJoinSql`.
6. Track the active SQL node so paging/filter/profile work on cross-source workspace results.
7. Keep same-source saved views; mark cross-source workspaces session-only.
8. Run frontend tests/check/build.

### Task 4: Integration verification and commit

**Files:** no new production files.

**Steps:**
1. Run `node --test frontend/tests/*.test.js`.
2. Run `uv run pytest -q`.
3. Run `cd frontend && npm run check && npm run build`.
4. Rebuild Docker Compose and wait for readiness.
5. Live-smoke a cross-source join and a composite-key same-source join.
6. Verify cardinality labels, Cartesian marker, custom dropdown keyboard behavior, and no console errors.
7. Commit separately from `dd9d443` with `feat: add cross-source join cardinality preview`.
