# Workbook Input Implementation Plan

> **For Hermes:** Use subagent-driven-development skill to implement this plan task-by-task.

**Goal:** Uploading an `.xlsx` workbook exposes every worksheet as a separate, queryable DuckDB dataset while retaining existing JSON, JSONL, and Parquet ingestion.

**Architecture:** Keep the existing in-memory DuckDB node per source. For `.xlsx`, use DuckDB's built-in `excel` extension to create one read-only view per worksheet. Use Python's stdlib `zipfile` + `xml.etree.ElementTree` only to read worksheet names from `xl/workbook.xml`; DuckDB then reads each sheet via `read_xlsx`. Add `.jsonl` as an alias for the existing JSON reader. Do not execute uploaded `.sql` files: SQL is executable program input, not a table format, and this local viewer does not yet have a sandboxed import contract.

**Tech Stack:** Python 3.11 stdlib, DuckDB 1.5 excel extension, FastAPI, pytest.

---

## Research Findings

- DuckDB's `excel` extension supports `.xlsx` and can autoload; it does **not** support legacy `.xls`.
- `read_xlsx(path, sheet = 'Sheet Name')` reads a named sheet.
- JSON and Parquet already route through `read_json_auto` and `read_parquet`.
- `.ndjson` already routes through `read_json_auto`; `.jsonl` needs only suffix acceptance.
- A `.sql` file has no default "table" semantics. Executing it would permit arbitrary DuckDB statements, so it remains deliberately unsupported.

### Task 1: Add workbook sheet views and JSONL suffix acceptance

**Objective:** Construct one DuckDB view per Excel worksheet, and accept `.jsonl` exactly like `.ndjson`.

**Files:**
- Modify: `backend/app.py`
- Test: `tests/test_backend.py`

**Step 1: Write failing test**

Create a two-sheet `.xlsx` fixture with DuckDB `COPY ... FORMAT XLSX`, upload it, and assert `GET /datasets` yields both named sheets and that querying each yields its own row. Add a `.jsonl` upload assertion alongside the existing supported format test.

**Step 2: Run targeted test to verify failure**

Run: `uv run pytest tests/test_backend.py -q -k 'supported_formats or workbook'`

Expected: failure because `.xlsx` and `.jsonl` are rejected.

**Step 3: Write minimum implementation**

- Add `.xlsx` and `.jsonl` to `SUPPORTED`; do not add `.xls`.
- Add a tiny `workbook_sheets(path)` helper that reads `xl/workbook.xml` via `zipfile.ZipFile` and `ElementTree` and returns sheet names. Reject malformed/empty workbooks through the existing `Could not open source` boundary.
- In `connect`, install/load DuckDB's `excel` extension only for `.xlsx`, then create one quoted view per sheet from `read_xlsx`.
- Preserve the existing generic `data` view for every other file type.

**Step 4: Run targeted test to verify pass**

Run: `uv run pytest tests/test_backend.py -q -k 'supported_formats or workbook'`

Expected: passing.

### Task 2: Explain the supported-input contract

**Objective:** Make the app's supported-input list accurate without inventing SQL execution.

**Files:**
- Modify: `README.md`
- Modify: `frontend/src/App.svelte`
- Modify: `docs/SPEC.md`

**Step 1: Update the supported-format contract**

State: CSV, TSV, Parquet, JSON/JSONL/NDJSON, XLSX (every worksheet is a dataset), and DuckDB/DB. State that legacy XLS and SQL scripts are not accepted in the README. Keep the onboarding change to its existing one static format-description string; do not add UI controls. In `docs/SPEC.md`, update the node and supported-file semantics and remove the stale spreadsheet-extension non-goal.

**Step 2: Run full verification**

Run:

```bash
uv run pytest -q
(cd frontend && npm run check && npm run build)
git diff --check
```

Expected: all checks pass.

## Task 3: Confirm selected workbook sheets before creating a node

**Objective:** After an XLSX upload, show a native accessible confirmation dialog listing its worksheets; create datasets only for the user-selected sheets after they explicitly continue.

**Files:**
- Modify: `backend/app.py`
- Modify: `tests/test_backend.py`
- Modify: `frontend/src/lib/types.ts`
- Modify: `frontend/src/lib/api.ts`
- Modify: `frontend/src/App.svelte`
- Modify: `frontend/src/app.css`

**Contract:**

- `POST /api/nodes/upload` keeps returning a node for every non-XLSX source. For XLSX, it stores the uploaded workbook temporarily and returns `{id, name, kind: "workbook", sheets: string[]}` without registering a node or opening worksheet views.
- `POST /api/nodes/upload/{id}/confirm` takes `{sheets: string[]}`. It requires a non-empty, duplicate-free subset of the staged workbook's discovered sheets, then registers one node whose datasets are exactly those sheets.
- `DELETE /api/nodes/upload/{id}` discards a staged workbook on Cancel. Unknown/expired staging IDs return 404.

**TDD:**

1. Write a failing backend test for XLSX upload preview, confirming only one selected sheet, rejection of unknown/empty/duplicate selections, and cancellation cleanup.
2. Run the targeted test and observe failure.
3. Implement the smallest staging dictionary plus confirm/delete handlers. Do not persist staging entries or add a general upload job system.
4. Extend the existing upload handler/UI to branch on the workbook preview response. Open one native `<dialog>` with checked sheet boxes, a Continue button disabled until at least one sheet is selected, and Cancel that deletes the staging upload. On successful confirm, close the dialog and select the new node through the existing path.
5. Run backend, frontend, and an actual XLSX upload → choose one sheet → confirm → query smoke flow.

## Deliberate exclusions

- No `pandas`, `openpyxl`, or spreadsheet parser dependency.
- No SQL script execution or SQL editor.
- No sheet picker: the existing dataset selector is already the correct UI.
