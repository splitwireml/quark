# Projects, Sources, Views

**Goal:** Make projects the top-level tenant, expose every queryable relation as a versioned View, and allow joins between any Views in one project.

**Constraints:** Branch from `feat/versioning-engine`; preserve its dirty follow-up work; no new dependencies; keep legacy node APIs and registry files working; Atomic Design; metadata-only View/version storage; no authentication or multi-user machinery.

## Contract

- `GET /api/projects` and `POST /api/projects {name}` list/create persisted projects.
- Existing registry nodes without `project_id` belong to a stable `default` project without rewriting the registry at startup.
- Project source routes add/list sources inside one project; project View discovery returns each physical table/worksheet as a base View with stable project-workspace SQL.
- A project's `node_id` is a private DuckDB execution workspace containing all its sources under stable schemas.
- Join references may carry validated read-only `sql`; two Views in the same project workspace can therefore be previewed and joined regardless of whether either is base, SQL, aggregate, or a previous join.
- Browser storage contains one history per View. A derived View starts at Version 1; later changes append only to that View. Legacy dataset histories and saved Views migrate into independent View histories.

## Task 1 — Backend project tenancy and project workspace

**Owns:** `backend/app.py`, `tests/test_backend.py`

1. RED: cover project creation/persistence, source isolation, legacy default-project migration, base View discovery/query, and derived-SQL join preview.
2. Add the smallest projects registry beside the source registry.
3. Reuse source connections and mounting code to build one lazy in-memory DuckDB workspace per project; rebuild it only when project sources change.
4. Add project-scoped source/view routes while preserving current node routes.
5. Extend join references with optional read-only View SQL and enforce same-project execution for SQL View joins.

**Check:** focused new backend tests, then `uv run pytest -q`.

## Task 2 — View history model

**Owns:** `frontend/src/lib/types.ts`, `frontend/src/lib/versioning.ts`, `frontend/tests/versioning.test.js`, `frontend/tests/join-sql.test.js`

1. RED: prove every View starts at Version 1, derived Views have independent histories, versions remain scoped to one View, legacy histories migrate, and join SQL wraps arbitrary View SQL safely.
2. Replace dataset-owned nested Views with one flat `ViewHistory[]` model.
3. Keep only SQL/JSON metadata; preserve execution target and join lineage needed for replay.
4. Extend join SQL generation to use View SQL subqueries.

**Check:** `npm test`.

## Task 3 — Project and View UI

**Owns:** `frontend/src/App.svelte`, `frontend/src/lib/api.ts`, `frontend/src/components/organisms/ProjectsScreen.svelte`, `frontend/src/components/organisms/SourceRail.svelte`, `frontend/src/components/molecules/ViewTreeItem.svelte`, `frontend/src/components/organisms/TitleBar.svelte`, `frontend/src/components/organisms/DatasetTabsBar.svelte`, `frontend/src/components/organisms/VersionsViewsPane.svelte`, `frontend/src/components/organisms/JoinMenuPopover.svelte`, `frontend/src/components/organisms/WelcomeScreen.svelte`, `frontend/src/components/organisms/WorkbookDialog.svelte`, and wording-only consumers as required.

1. Open on a project picker with native create-project form; entering a project loads only its sources and exiting returns to the picker.
2. Make the source rail an advanced DB explorer: Sources contain base Views; derived Views have their own section; each View shows `vN` at the right edge.
3. Remove sheet/dataset product language and dataset tabs. Keep only Data/Versions workspace controls.
4. Creating SQL, aggregate, or join output adds a new selected View at Version 1. All later edits/finalization operate on that View's history.
5. Join picker lists every View in the current project and sends each View's current SQL to the backend preview.

**Check:** `npm test && npm run check && npm run build`.

## Task 4 — Integration

**Owns:** parent/reviewer only; no feature edits unless a check proves a gap.

- Run all backend/frontend checks.
- Build production assets and run a disposable-data live server.
- Browser-smoke: create two projects; upload sources into one; confirm isolation; open base Views; create an aggregate View; version it; join base + derived Views; exit and re-enter the project; verify View/version labels and browser console.
- Spec review, then code-quality review; fix critical/important findings and rerun checks.

## Task 5 — Layered lazy source loading

**Owns:** `backend/app.py`, `tests/test_backend.py`, `frontend/src/App.svelte`, `frontend/src/lib/api.ts`, `frontend/src/lib/types.ts`, `frontend/src/components/organisms/SourceRail.svelte`, and the existing `SourceTreeItem.svelte` molecule.

1. `GET /api/projects/{project_id}/sources` returns only source `id` and `name`; it does not create a project workspace, inspect schemas, expose file paths, or return rows.
2. `GET /api/projects/{project_id}/sources/{source_id}` mounts only that source and returns its non-path metadata plus associated base View metadata. `GET .../{source_id}/path` is the explicit disk-path layer.
3. Keep the all-Views endpoint only as a compatibility/explicit-load path; it may mount every source, but project entry must not call it.
4. Entering a project renders source summaries without selecting or querying one. Selecting a source fetches its details, reveals its Views, then queries only the selected source's first/preferred View.
5. Reuse `SourceTreeItem`; preserve request-race guards, persisted histories, source mutation refresh, legacy APIs, and same-project joins. A restored derived View may explicitly load all source metadata because old arbitrary SQL has no dependency manifest.

**Check:** focused API contract test proves the list/detail/path layers and one-source mounting, then `uv run pytest -q`; `npm test && npm run check && npm run build`; browser network smoke proves project entry sends no View or row payload.
