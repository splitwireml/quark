# SQL Editor and Upload Aliases Implementation Plan

> **For Hermes:** Use subagent-driven-development skill to implement this plan task-by-task.

**Goal:** Fix the SQL error-state layout, trigger table IntelliSense after `FROM`, and name newly ingested single-table files from their filename instead of generic `main.data`.

**Architecture:** Keep CodeMirror and the current dataset metadata contract. Fix the CSS class collision at the editor host, then trigger CodeMirror's existing SQL completion source when the cursor enters a `FROM`/`JOIN` table position. For uploads, persist a sanitized filename-stem alias in each new node and use it as the DuckDB view name; old registry entries without an alias keep `data` so saved queries do not break.

**Tech Stack:** FastAPI, DuckDB, pytest, Svelte 5, CodeMirror 6, TypeScript, Docker Compose.

---

### Task 1: Persist readable dataset aliases for new uploads

**Objective:** `claims_v1.csv` exposes `claims_v1` while historical uploads continue exposing `data`.

**Files:**
- Modify: `backend/app.py`
- Test: `tests/test_backend.py`

**Steps:**
1. Add a focused failing test covering filename sanitization (`Claims v1.csv` → `claims_v1`, leading digits, punctuation/fallback), registry persistence/restart, executable SQL using the alias, and legacy nodes without an alias retaining `data`.
2. Run the focused test and confirm RED.
3. Add one small filename-stem sanitizer and store `dataset_name` on newly uploaded non-XLSX/non-database nodes.
4. Create the scan view with `quote(node.get("dataset_name", "data"))`; preserve workbook sheet names and attached database tables unchanged.
5. Run focused tests, then `uv run pytest -q` and `git diff --check`.

### Task 2: Repair SQL error layout and automatic table completion

**Objective:** SQL errors leave CodeMirror's gutter/content geometry unchanged, and typing after `FROM` or `JOIN` automatically opens dataset suggestions.

**Files:**
- Modify: `frontend/src/App.svelte`
- Modify: `frontend/src/app.css`
- Modify: `frontend/package.json`
- Modify: `frontend/package-lock.json`

**Steps:**
1. Reproduce the error state and record editor/gutter widths before and after an invalid query.
2. Replace the generic `error` class on `.sql-editor` with a SQL-specific class so global `.error div { flex: 1 }` cannot alter CodeMirror descendants.
3. Reuse CodeMirror's installed SQL completion data and call `startCompletion` after document changes matching a trailing `FROM`/`JOIN` table position; add `@codemirror/autocomplete` as a direct dependency because it is imported directly.
4. Keep schema metadata sourced from `datasets`, so suggestions show readable aliases and all current-node tables.
5. Run `npm run check`, `npm run build`, and `git diff --check`.

### Task 3: Integrate, live-smoke, rebuild Docker, and publish

**Objective:** Verify the real browser flow and replace the running container with the fixed build.

**Files:**
- Modify only if needed: `README.md`, `docs/SPEC.md`

**Steps:**
1. Run `uv run pytest -q`, `npm run check`, and `npm run build`.
2. Live browser smoke: invalid `SELECT` preserves normal gutter width; typing `SELECT * FROM ` automatically lists the readable dataset alias; executing a query through that alias succeeds.
3. Rebuild/restart with `docker compose up -d --build`; verify local and tailnet HTTP 200.
4. Run spec review, then code-quality/security review; fix important findings and re-review.
5. Commit only tracked implementation/plan files, push `main`, and leave pre-existing untracked drafts untouched.
