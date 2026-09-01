# Mutation Engine and Export Implementation Plan

> **For Hermes:** Use subagent-driven-development to implement this plan with disjoint backend/frontend ownership.

**Goal:** Insert type-aware computed columns from the grid and export the full current view or selected datasets as CSV/XLSX workbooks.

**Architecture:** Reuse Quark's existing read-only SQL pipeline. The frontend composes a new `SELECT` around `result.sql`, while DuckDB performs final expression/type validation. A single backend export endpoint executes read-only SQL against one or more existing node connections and writes CSV with stdlib or multi-sheet XLSX with openpyxl write-only mode.

**Tech Stack:** FastAPI, DuckDB, Pydantic, stdlib CSV/tempfiles, openpyxl, Svelte 5, native drag/drop and dialog controls.

---

### Task 1: Export contract and writer

**Ownership:** `backend/app.py`, `tests/test_backend.py`, `pyproject.toml`, `uv.lock` only. No commits.

**Behavior:**
- Add `POST /api/exports` with `{format, filename?, sheets:[{node_id,name,sql}]}`.
- CSV requires one sheet and returns every row with headers.
- XLSX accepts datasets from different nodes, creates one sanitized/uniquely named worksheet per request sheet, and preserves ordinary scalar/date values.
- Every SQL string passes the existing single read-only `SELECT` validator; unknown nodes and invalid payloads fail cleanly.
- Use a temporary file deleted by a response background task; partial files are deleted on failure.

**TDD:**
1. Add focused tests for full-row CSV, multi-node/multi-sheet XLSX, duplicate/invalid sheet-name normalization, read-only SQL rejection, and CSV cardinality validation.
2. Run the focused tests and observe failure.
3. Add the smallest endpoint/writer implementation and `openpyxl` dependency.
4. Run focused tests, then `uv run pytest -q`.

### Task 2: Mutation SQL builder and grid UI

**Ownership:** `frontend/**` only. No commits.

**Behavior:**
- Add a thin mint insertion target between adjacent columns; clicking opens the mutation menu at that boundary.
- Modes: blank `NULL`, repeating text literal, and formula.
- Formula UI accepts typing plus click/drag column pills. After the first column, show only relevant numeric, text, date, or logical controls and disable incompatible column pills.
- Include arithmetic and parentheses/percent helpers; text upper/lower/left/right/regex/concatenation/separators; date day/month/year/date-difference; comparison/logical helpers.
- Build a wrapped `SELECT` that inserts the new alias at the requested column index. Reject blank/duplicate names client-side. DuckDB remains the final parser/type checker.
- Add an Export button and native dialog. CSV exports the full current view. Excel can export the current view and/or checked datasets loaded across all sources into one workbook.
- Follow Atomic Design: insertion handle in atoms, dialogs/menus in organisms, page orchestration in `App.svelte`.

**Checks:**
1. Add a Node built-in test for SQL quoting, literal escaping, semicolon stripping, and insertion order; run it red before implementation.
2. Implement minimal helpers/components and API types.
3. Run the Node test, `npm run check`, and `npm run build`.

### Task 3: Integration and review

**Ownership:** Parent/reviewer only.

**Verification:**
- Run `uv run pytest -q` and frontend test/check/build.
- Start Quark and smoke: open a dataset, click a mint gap, add NULL/constant/formula columns, confirm a bad mixed-type formula is rejected without replacing the view, export CSV, export an XLSX with sheets from at least two sources, then inspect downloaded workbook sheet names and rows.
- Review spec compliance first, then code quality/accessibility/security, fix all critical/important findings, rerun checks, and inspect final diff.

### Task 4: Append-column target and spreadsheet cell edits

**Ownership:** `frontend/**` only. No commits.

**Behavior:**
- Render the same mint insertion target after the last visible column and insert at the full result column count.
- A focused cell starts editing when the user types; Enter, arrow keys, or blur confirms, and Escape cancels.
- Cell edits remain read-only source projections: wrap `result.sql`, number current-view rows, replace one typed value with DuckDB `cast_to_type`, and execute through the existing `/sql` endpoint.
- Preserve current-view filters/sorts in the wrapped SQL, reject invalid typed values without replacing the result, guard request races, and disable editing while a query is loading.
- Keyboard arrows navigate visible cells after successful confirmation; keep focus and selected state accessible.

**Checks:**
1. Add failing Node tests for append insertion and cell-edit SQL generation.
2. Implement the smallest App/DataGrid state and editor input.
3. Run mutation tests, Svelte check/build, backend suite, and Playwright smoke for typing, Enter/arrows/blur/Escape, invalid values, export persistence, and the final-column target.
