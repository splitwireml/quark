# Quark desktop, sub-project 2: backend parity

Date: 2026-10-08. Status: approved 2026-10-13; implemented on `rust-desktop/parity`, exit gate awaiting H2 and deviation sign-off (`2026-10-13-rust-desktop-parity-exit-deviations.md`). Implementation plan: `2026-10-08-rust-desktop-parity.md`, scheduled in `2026-10-08-rust-desktop-parity-orchestration.md`. Part 2 of the Rust desktop pivot. The pivot, all of its sub-projects and their status are in `2026-10-04-rust-desktop-pivot-design.md`.

Read with: the skeleton design `2026-10-02-rust-desktop-skeleton-design.md` (section 5 sets the parity rules, section 6 the caches), the signed-off deviations `2026-10-04-skeleton-exit-deviations.md`, and the planning notes `2026-10-05-sp2-backend-parity-notes.md`.

## 1. Context

Sub-project 1 shipped the walking skeleton: a Tauri 2 app whose Rust core (`crates/quark-core`) serves 13 of the 30 route entries in `backend/app.py`, with four efficiency upgrades, CI and installers on macOS, Windows and Ubuntu. Every other `/api/*` request answers 501.

This sub-project ports the remaining 17 route entries, so the desktop app never needs FastAPI. It also takes the efficiency upgrades the Python version skipped to get things working. Python marks those shortcuts with `ponytail:` comments, and section 6 lists each one with its replacement.

The parity oracle is `backend/app.py` at `0270c8d`. That file is identical on `main`, `rust-desktop/integration` and `rust-desktop/skeleton`. The `codex/chart-palettes` branch also edits it, but its change predates main's current scatter code (which already carries color, size and shape), so it does not move the oracle.

## 2. Decisions

Made during brainstorming on 2026-10-06 to 2026-10-08:

- **Goal:** the same API, with Python's shortcuts replaced by the best implementation that keeps the API contract. Efficiency upgrades are in scope wherever they apply.
- **Parity rule, refined:**
  - The API contract stays identical to Python: response shapes, field names, status codes, handled-error `detail` text, identifiers, values after JSON parsing, and the data folder format. The frontend does not change.
  - Internals and performance are free to change.
  - Three visible behaviour fixes are allowed (section 7).
  - One error-path difference, approved with the spec: an unreadable workbook returns 400 instead of Python's 500 (section 7).
  - Nothing else visible changes.
- **Tests:**
  - The existing pytest suite stays the API test suite and runs against the Rust server (`QUARK_TEST_BACKEND=rust`). It is the only suite that runs against both backends, which makes it the parity proof.
  - Every unit card also adds `cargo test` unit tests.
  - Sub-project 5 decides whether to port the suite to Rust or keep it as a test-only tool.
- **Parallel reads:** a full read pool per engine (section 5.1).
- **Code structure:** atomic design applied to the backend, with feature folders and a layer per file, enforced in CI (section 4). SP1's code is reshaped into this layout first.
- **XLSX:** pure Rust. `calamine` reads and `rust_xlsxwriter` writes. No DuckDB `excel` extension and no network.
- **Time zones:** ICU compiled into DuckDB through the `duckdb` crate's `icu` feature, proven first by a test build on every target. If that build fails on an OS, that OS falls back to UTC semantics in SQL, documented as a deviation.
- **Branches:**
  - `rust-desktop/integration` was fast-forwarded to the skeleton (`af7c0bd`) on 2026-10-08.
  - This sub-project lives on `rust-desktop/parity`, branched from integration.
  - SP1's remaining manual H2 steps run as part of this sub-project's exit gate (section 12).
- **Legacy routes** (`/api/nodes`, `/api/nodes/upload`, `DELETE /api/nodes/{node_id}`, `/api/nodes/{node_id}/datasets` and the dataset `/query` route) stay until sub-project 5, because the pytest suite creates its data through them.
- **Boundary with sub-project 3:**
  - Exports are delivered as an HTTP download, as in Python.
  - Attach takes a typed path, as in Python.
  - Native save and open dialogs belong to sub-project 3. If a webview cannot save the downloaded export, this sub-project adds a minimal Tauri download handler that saves into the Downloads folder.

## 3. Goals and non-goals

Goals:

1. Serve all 30 route entries from Rust under the parity rule of section 2.
2. Import and export XLSX offline.
3. Match Python's time-zone semantics through ICU.
4. Ship the efficiency upgrades of section 6, held to the performance gate of section 9.
5. Organize the code as section 4 describes and enforce it in CI.
6. Pass an exit gate that includes the H2 live test on all three OSes (section 12).

Non-goals:

- Opening files in place, native dialogs, drag and drop, file associations, single instance and the default-app card (sub-project 3).
- Signing, notarization, updates and the app icon (sub-project 4).
- Removing Python, pytest or Docker (sub-project 5).
- Frontend changes, except fixes the WebKit pass (section 8.5) shows are needed.
- Byte-level upload progress.
- Any change to response shapes.

## 4. Code structure

### 4.1 Layers

The backend follows the same atomic design as the frontend's `atoms/`, `molecules/`, `organisms/` and `templates/`, mapped onto data access, dependencies and thin endpoints:

| Layer | Atomic name | Lives in | Holds |
|---|---|---|---|
| Building blocks | Atoms | `shared/`, each feature's `model.rs` | Pure functions and types: quoting, literals, ids, value encoding, request and response structs. No DuckDB calls, no HTTP. |
| Data access | Molecules | each feature's `queries.rs`, `relation/` | One SQL statement family per function. Takes a connection and typed input, returns typed rows. No business rules. |
| Services | Organisms | each feature's service files | One use case per function: validation, limits, "Other" folding, error text. Composes queries. Never imports `axum`. |
| Routes | Templates | each feature's `routes.rs` | Handlers that take dependencies, call one service function and return its result. No SQL, no rules. |
| Dependencies | (wiring) | `http/deps.rs` | axum extractors, Rust's version of FastAPI's `Depends()`. |
| Runtime | (infrastructure) | `runtime/` | Engines, the read pool, cancellation, mounting, catalog, registry, caches, background workers. |

### 4.2 Layout

The folders follow the repository's `rust-skills` rule `proj-mod-by-feature`: folders by feature, and files by layer within each feature.

```
crates/quark-core/src/
├── lib.rs                 module list only
├── error.rs               ApiError, rendered as {"detail": ...}
├── shared/                atoms: quoting, literals, ids, naming, encode/ (values to JSON, Arrow IPC)
├── relation/              shared molecules: request types, filtered relation builder, describe, SQL guard
├── features/
│   ├── projects/  sources/  pages/               (SP1 code, reshaped; attach joins sources/)
│   ├── stats/  values/  find/  charts/  joins/  exports/  workbooks/
│   └── <feature>/
│       ├── mod.rs         re-exports and a one-line purpose
│       ├── model.rs       request and response types (serde)
│       ├── queries.rs     SQL molecules
│       ├── service.rs     or one file per use case (charts: bar.rs, line.rs, ...)
│       ├── routes.rs      handlers
│       └── tests.rs       unit tests
├── http/                  router (merges every feature's routes()), deps.rs, secure.rs, logging
└── runtime/               engine, pool, cancel, mount, catalog, registry, uploads, workers, cache/
```

### 4.3 Rules

1. **One direction.**
   - Routes call services, services call queries, and queries use `relation/` and `shared/`. Lower layers never import higher ones.
   - Routes take what they need from `http/deps.rs`, which draws on `runtime/`.
   - A service that changes app state, such as registering a source or opening a join workspace, receives a runtime handle from its route as an argument.
   - `runtime/` never imports `features/` or `http/`.
2. **No feature imports another feature.** Code two features need moves down into `relation/` or `shared/`.
3. **Routes are thin.** A handler declares its dependencies, calls one service function and returns. It contains no SQL and no business rules.
4. **Services are HTTP-free.** They take a connection and typed input and return typed output or an `ApiError`, so they are testable without a server.
5. **File size.**
   - Production files aim for 200 lines or fewer, with a hard cap of 250.
   - Unit tests live in a sibling `tests.rs`, declared as `#[cfg(test)] mod tests;`, with a cap of 400 lines.
   - A file over its cap is split by responsibility, never by line count alone.
6. **Self-documenting names.** Every file starts with a one-line `//!` purpose. File names are nouns (`bar.rs`, `queries.rs`) and function names are verbs (`profile_column`, `count_rows`). Handler names match the route's action (`stats`, `visualize`).

### 4.4 Enforcement

`crates/quark-core/tests/architecture.rs` reads the source tree with the standard library only. It fails when:

- a file exceeds its line cap;
- a file under `shared/` imports `relation`, `features`, `http` or `runtime`;
- a file under `relation/` imports `features`, `http` or `runtime`;
- a file under `runtime/` imports `features` or `http`;
- a `queries.rs` or service file imports `axum` or `crate::http`;
- a `routes.rs` has a string literal containing `SELECT `;
- a feature imports another feature;
- a file has no `//!` line.

It runs in CI with the other `cargo test` targets.

### 4.5 Dependencies

| Extractor | Gives the handler | Fails with |
|---|---|---|
| `ApiJson<T>` | The parsed body; unknown fields rejected; 2 MB limit (SP1) | 422, or 413 over the limit |
| `Project` | The project named by `{project_id}` | 404 `Project not found` |
| `EngineFor` | The engine for `{node_id}`: a project workspace, a legacy node or a join workspace | 404 `Node not found` |
| `Reader` | A reader borrowed from that engine's pool, a blocking thread to run on, and a cancellation guard on cancellable routes | 422 or 500 as the work returns |
| `Format` | JSON or Arrow, from `Accept` | none |

A handler reads like `reader.run(move |conn| service::visualize(conn, &request))`. The repeated `blocking`, guard and ticket code in SP1's handlers moves into `Reader`.

### 4.6 Reshaping SP1's code

Before any route is ported, SP1's modules move into the layout with no behaviour change:

- **`shared/`:** `sql`, `literal`, `ids`, `naming`, `values` and `arrow`.
- **`relation/`:** the request types and builder from `query`, `guard`, and `describe`.
- **`runtime/`:** `engine`, `mount`, `cancel`, `cache/*` and `registry`. `state` is split by responsibility (catalog, engine resolution, uploads, background work).
- **`features/pages`, `projects`, `sources`:** `page` and the three `api/*` route files.
- **`http/`:** `api/mod.rs` and `secure`.

Seven files are over the 250-line cap and are split. Production lines today:

| File | Lines |
|---|---|
| `state.rs` | 630 |
| `cache/columnar.rs` | 365 |
| `values.rs` | 353 |
| `page.rs` | 343 |
| `query.rs` | 318 |
| `cache/results.rs` | 288 |
| `engine.rs` | 261 |

Inline test modules move to sibling `tests.rs` files.

The reshape is done when `cargo test`, clippy, fmt, the Rust-mode pytest skeleton suite and `perf_acceptance` give the same results as before, and the architecture test passes. Because every later card lands in its own feature folder, parallel workers never edit the same file.

## 5. Runtime changes

### 5.1 Read pool

- **Shape.**
  - Each engine has one primary connection and N reader connections cloned from it with `try_clone`, all on the same DuckDB database.
  - N is half the available cores, at least 2 and at most 4.
  - The primary alone runs catalog changes: mounting views, attaching columnar caches, and creating or dropping result tables. Those run under the engine's mount lock.
- **Borrowing.**
  - `Reader` waits for a free reader, then runs the work on a blocking thread and returns the reader to the pool.
  - A slow chart, stats call or export ties up one reader, never the engine. Pages keep being served while it runs.
- **Mounting first.** `EngineFor` returns an engine only after the views a request needs are mounted, so readers never race a mount.
- **Lock-down.** `enable_external_access`, the allow-lists and `temp_directory` are database-wide settings, so cloned readers inherit them. A test proves a reader cannot read a file outside the allow-list and cannot change the settings.
- **Caches outside the lock.** The count and null cache, the result cache and the response cache (section 5.3) leave `EngineInner` and get their own small locks. Those locks are held only to read or update an entry, never while a query runs.
- **Rebuild model unchanged.** A source change still builds a new engine. The old engine stays alive through its `Arc` until its readers finish.
- **DuckDB threads.** Concurrent queries share DuckDB's one thread pool per database, so total CPU use does not grow with N.

### 5.2 Cancellation

- Each reader has its own interrupt handle. The SP1 ticket logic (`run_guarded`, re-interrupting until the query stops) moves from the engine to the reader, so cancelling one request interrupts only the reader running it.
- **Cancellable routes**, where a dropped request interrupts its query:
  - pages (as in SP1)
  - stats
  - category values
  - charts
  - find
- **Run to completion:** exports and join previews. A user starts them deliberately and waits for the result.

### 5.3 Caches

- **Existing caches** keep their behaviour from SP1 (skeleton design, sections 6.2 to 6.5). The unused `generation` field is removed (deviation 13), because caches already live and die with their engine.
- **Response cache (new).**
  - **Covers:** stats, category values and chart responses.
  - **Capacity:** an LRU of 128 entries per engine, matching Python's profile cache.
  - **Key:** the route kind, the relation SQL, the bound values, the column or chart spec, and the size and modification time of every source file the engine reads, as Python's `profile()` fingerprints them.
  - **Skipped** when the result can change without a file change. The rules are ported from Python's `profile()`:
    - the SQL uses a function DuckDB reports as not `CONSISTENT`, or a table function or macro other than `query`;
    - it uses `current_*`, `localtime`, `localtimestamp`, `TABLESAMPLE` or `SAMPLE`;
    - the engine has stored views in an attached database;
    - the SQL nests `query()` or scans a file path directly.
- **Find cache (new).** The numbered relation behind find (section 6.3) is kept as a table in the engine's result-cache schema, keyed by the View's SQL. It follows the result cache's capacity and eviction.

### 5.4 ICU and time zones

- **Build.** The `duckdb` dependency gains the `icu` feature, which builds DuckDB with CMake (`bundled-cmake`) instead of `cc`. DuckDB is still compiled into the binary. The exports in section 6.2 also need the `appender-arrow` feature.
- **Test build first.** The first card of sprint 1 builds and bundles on macOS (universal), Windows and Ubuntu in CI, then records cold build time, binary size and CI cache size against SP1. It also checks that:
  - `current_setting('TimeZone')` is the machine's zone;
  - a TIMESTAMPTZ filter value without an offset is read in local time;
  - `date_trunc` and `strftime` on TIMESTAMPTZ use local time.
- **Fallback.** If the CMake build cannot be made to work on an OS, that OS keeps today's build and UTC semantics in SQL. This is recorded as a deviation.
- **Rendering.** Value rendering stays as SP1 defined it: TIMESTAMPTZ in the machine's zone with its offset.
- **TIMETZ (deviation 4).** DuckDB drops the offset when it exports TIMETZ to Arrow. So any query that returns TIMETZ cells to the client selects them as text, and the encoder renders Python's `isoformat()` form, for example `12:00:00+02:00`. Column metadata keeps DuckDB's type name.

### 5.5 Join workspaces

- **Same project.** A join between datasets of one project, or any join that uses SQL, runs in that project's workspace engine, as in Python.
- **Across sources.** A cross-source join gets its own in-memory engine with a read pool. Each side mounts the source's columnar cache when one is ready and the live scan otherwise, instead of re-reading the raw file.
- **Slots.** Up to 4 cross-source join workspaces stay open, least recently used first out. Their ids keep Python's form, `join_<uuid4 hex>`, and they are never listed. This is behaviour fix 3 (section 7).

## 6. Routes

Each feature reproduces its Python handler exactly: request validation, limits and constants, `detail` messages, response shapes, and the `safe()` value rules (shared with SP1's encoder). Line numbers refer to `backend/app.py` at `0270c8d`. The upgrades column lists what changes inside.

| Feature | Routes | Python | Upgrades |
|---|---|---|---|
| `stats` | `GET` and `POST /api/nodes/{node_id}/datasets/{dataset}/columns/{column}/stats`, `POST /api/nodes/{node_id}/sql/columns/{column}/stats` | 1339 to 1450, 1918 to 1974 | Read pool, response cache, cancellation. Counts come from the count and null cache when present. Each profile kind runs as few full scans as its shape allows. |
| `values` | `GET /api/nodes/{node_id}/datasets/{dataset}/columns/{column}/values`, `POST /api/nodes/{node_id}/sql/columns/{column}/values` | 1278 to 1337 | Read pool, response cache, cancellation. The total and the page come from one scan (`count(*) OVER ()`). |
| `find` | `POST /api/nodes/{node_id}/sql/find` | 1237 to 1263 | The numbered relation is materialized once per View and reused across Find Next (section 5.3). Cancellable. |
| `charts` | `POST /api/nodes/{node_id}/datasets/{dataset}/visualize`, `POST /api/nodes/{node_id}/sql/visualize` | 1452 to 1916 | One service file per chart kind. See section 6.1. |
| `joins` | `POST /api/join-workspaces` | 998 to 1113 | Left and right row counts come from the count cache. Each uniqueness check is a single scan. Cross-source sides read the columnar copies (section 5.5). Several workspaces stay open (section 7). |
| `exports` | `POST /api/exports` | 935 to 996, helpers 253 to 375 | See section 6.2. |
| `workbooks` | the `.xlsx` branch of both upload routes, `POST .../sources/upload/{stage_id}/confirm`, `DELETE .../sources/upload/{stage_id}` and their `/api/nodes/upload/...` twins | 225 to 231, 464 to 503, 1115 to 1175 | Pure-Rust import once into a cache file. See section 6.3. |
| `sources` | `POST /api/projects/{project_id}/sources/attach`, `POST /api/nodes/attach` | 1177 to 1190 | None needed. Joins the existing `sources` feature as `attach.rs`. `~` is expanded and the path resolved as Python's `expanduser().resolve()` does. |
| `sources` | `GET /api/projects/{project_id}/sources/{source_id}/path` | 922 to 924 | None. |

### 6.1 Charts

The chart dispatcher builds the filtered relation once, then calls the service for the chart kind.

| Kind | Python today | Rust |
|---|---|---|
| Metric | One query; a formula goes through the read-only guard and refuses `?` placeholders | Same |
| Bar and pie | Four queries when grouped | One ranking query and one grid query; "Other" folded in the service |
| Histogram | Bins from the profile; grouped counts only for equal-width numeric bins | Bins from the cached profile. Grouped counts for every bin type, by bucketing on the edge list (behaviour fix 2) |
| Box | Quantiles computed twice (whiskers query, then outliers query) | One pass computing quantiles, whiskers and outliers |
| Scatter | `ORDER BY random()`, so a new sample on every refetch | Repeatable sample: the same View and data return the same points, at no more cost than Python (behaviour fix 1) |
| Line | One query per series plus one for "Other" | One query ranking series and bucketing time together |

Scale domains for scatter, as in Python, come from the whole filtered relation, never from the sample. The sample technique is chosen by measurement. A reservoir sample with a fixed seed is used if a test proves it repeatable under the engine's thread settings. Otherwise the sample orders by a hash of the row.

### 6.2 Exports

- **Validation and guard.** Every sheet's SQL passes the SP1 read-only guard. Sheets run in request order, each on a reader from its own engine's pool, without cancellation.
- **CSV.**
  - Header row, then rows streamed from Arrow batches through Python's `csv_value` rules: the export value rules, plus a `'` before any text starting with `=`, `+`, `-`, `@`, a tab or a carriage return (`CSV_FORMULA_PREFIXES`).
  - Text output follows Python's `csv` module: `True` and `False` for booleans, Python's float `repr`, `str()` of decimals and dates.
- **XLSX.**
  - `rust_xlsxwriter` in constant-memory mode, one worksheet per sheet. Sheet names follow `export_sheet_name`: sanitized, at most 31 characters, `History` avoided, case-insensitive uniqueness with ` (n)` suffixes.
  - Strings are written as text cells, never as formulas.
  - A text cell over Excel's 32,767-character limit returns 422 `Excel cell text exceeds 32767 characters`. On any failure the partial file is deleted.
- **Parquet and JSON.**
  - A separate writer database, allowed to write only into the exports folder, receives the rows as Arrow batches through the appender, then runs DuckDB's `COPY` exactly as Python's `export_relation` does.
  - Python copies rows 1,000 at a time through `executemany`; Arrow batches replace that.
  - Duplicate column names return Python's 422.
- **Response.**
  - The file streams from `<app cache dir>/exports/` and is deleted when the response ends or fails. Startup sweeps leftovers.
  - Media types come from `EXPORT_TYPES` and the file name from `export_filename`.
  - `Content-Disposition` follows Starlette's `FileResponse` rule: `attachment; filename="<name>"` when URL-quoting leaves the name unchanged, and `attachment; filename*=utf-8''<quoted>` otherwise.
- **Errors.** A DuckDB error during an export returns 422 `Invalid SQL query: ...`, as in Python.

### 6.3 Workbooks

The importer (calamine to typed tables in a cache file) lives in `runtime/cache/`, beside the columnar importer. That lets an engine re-import an evicted workbook without `runtime/` importing a feature. `features/workbooks/` holds staging, confirmation and discard.

- **Upload.**
  - An `.xlsx` upload streams to `uploads/<id>.xlsx` like every upload, then reads the sheet names in workbook order with calamine.
  - It returns Python's staging response `{id, name, kind: "workbook", sheets}`, plus `project_id` on the project route, with status 201.
  - The pending upload is held in memory. A restart discards it, and SP1's orphan sweep deletes the file.
- **Confirm.**
  - Python's validation: 404 `Workbook upload not found`, and 422 `Select one or more unique workbook sheets`.
  - Then it imports each selected sheet:
    1. calamine streams the sheet's cells into an all-text staging table. For each column it records the type `read_xlsx` would declare (DATE, TIMESTAMP, TIME or other).
    2. Python's type-guessing SQL from `_xlsx_table` runs on the staging table unchanged: the 0.9 ratios for numbers, timestamps and dates, and Excel serial dates reinterpreted only where the declared type says so.
    3. The typed tables, one per sheet and named after it, go into one cache file per workbook under the columnar cache folder. They are written to `.partial` first and renamed into place, like SP1's columnar cache.
  - The source is registered with its `sheets`, as in Python's registry, and the node is returned with status 200.
- **Mounting.** Node engines and project workspaces attach the workbook's cache file read-only. The workbook is not parsed again. If the cache file was evicted, it is imported again on first use.
- **Discard.** `DELETE` removes the pending upload and its file, with status 204, or 404 as in Python.
- **Fidelity to `read_xlsx`.**
  - These rules must match what `read_xlsx` does:
    - header detection (for example, a sheet whose first row is a number gets a column named `A1`);
    - generated column names;
    - empty rows and cells;
    - the text each cell becomes in all-text mode;
    - the declared types.
  - Sprint 1's XLSX test build records these rules by comparing Python's `read_xlsx` with calamine on generated workbooks.
  - A corpus of workbooks then becomes pytest cases that run on both backends. It covers numbers, dates, times, datetimes, booleans, mixed columns, empty cells, cached formula results, shared strings, unicode and apostrophe sheet names, and header-only sheets.
- **Unreadable workbook.** This is the approved error-path difference (section 7). A file that is not a valid workbook, or that has no sheets, returns 400 `Could not open source: <reason>` and is deleted. Python answers a plain-text 500 here.

## 7. Behaviour fixes

Each fix replaces a Python shortcut. For each one, the existing test that pins Python's behaviour is marked `python_only`, and a new `rust_only` test asserts the new behaviour. The Python backend does not change.

| # | Python today | Rust | Tests |
|---|---|---|---|
| 1 | Scatter takes a new random sample on every refetch, so points jump | The same View and data always return the same sample | New `rust_only` test: two calls return identical points |
| 2 | Grouped histograms skip the group for date and huge-integer columns (app.py 1602) | Grouped counts work for every bin type | `test_visualize_histogram_skips_series_for_a_date_column` becomes `python_only`; a new `rust_only` test checks date series share the bin edges |
| 3 | One global join slot: a second cross-source join invalidates the first (app.py 407) | Up to 4 join workspaces, least recently used evicted | `test_cross_node_join_workspace_replaces_previous_without_listing_it` becomes `python_only`; a new `rust_only` test checks two joins stay queryable, a fifth evicts the oldest (404), and none are listed |

**Error-path difference (approved with the spec on 2026-10-13).**

- **Case:** an upload named `.xlsx` that is not a valid workbook, or that has no sheets.
- **Python:** raises an unhandled exception, so it answers a plain-text 500.
- **Rust:** answers 400 `Could not open source: <reason>`, the same error every other unreadable upload gets, and deletes the file.
- **Tests:** a new `rust_only` test pins the Rust answer. If this is rejected, Rust answers 500 `{"detail": "Internal error"}`.

## 8. Testing

### 8.1 pytest against Rust

- **Scope.** Every test not marked `python_only` runs in Rust mode. CI drops `-m skeleton`.
- **Progress list.**
  - `tests/rust_pending.txt` lists, one per line, the test functions that do not pass on Rust yet. In Rust mode, `conftest.py` marks them `xfail(strict=True)`, so CI stays green while the list shrinks. A listed test that starts passing fails the run, which forces the list to stay accurate.
  - Each card deletes the lines for the tests it makes pass. An empty file means parity.
- **Audit.** Tests that call Python functions directly (for example `test_safe_serializes_unsafe_aggregate_integers` calls Python's `safe()`) become `python_only`, and a Rust unit test covers the same rule.
- **Markers.** A new `rust_only` marker is skipped in Python mode, mirroring `python_only`.
- **Fixtures.** The test fixtures keep creating workbooks with Python's DuckDB `excel` extension. That is test tooling only and never reaches the app.

### 8.2 Rust unit tests

Every card adds unit tests in its feature's `tests.rs`. Services are tested by calling them with an in-memory connection, without a server. Queries get tests where their SQL carries logic, such as bucketing, ranking and folding.

### 8.3 Concurrency

Read-pool tests prove:

- two slow queries on one engine run at the same time;
- a page is served while an export runs;
- cancelling one request interrupts only its own reader;
- a mount and a read never interleave wrongly;
- readers keep the lock-down.

Stress tests stay at 4 copies or fewer.

### 8.4 Architecture

The architecture test (section 4.4) runs with every `cargo test`.

### 8.5 WebKit pass

A scripted Playwright WebKit pass runs over each ported feature, against `quark-dev-server` and the built frontend:

- the stats panel;
- the category filter list;
- every chart kind on the Chart and Dashboard canvases;
- find;
- the join flow;
- export downloads;
- workbook upload and confirmation.

SP1 showed that frontend state bugs show up only in hands-on use and in WebKit. Each pass is a script under `frontend/tests/*-browser.mjs`.

## 9. Performance

- **Baseline first.** A sprint 1 card measures Python and Rust (release build) over HTTP on the same synthetic 2M-row, 254 MB CSV that `perf_acceptance` uses, on a quiet machine. A script reports the median of several runs per case.
- **Gate.** Every case must be faster on Rust than on Python and within its cap. The caps below are provisional. They are confirmed or adjusted once, after the baseline, with the user's sign-off.

| Case | Cap |
|---|---|
| Stats on a numeric column, filter on | 300 ms |
| Category values, first page of a text column | 150 ms |
| Bar, line, histogram or metric chart | 500 ms |
| Scatter or box chart | 800 ms |
| Find Next after the first search on a View | 200 ms |
| Join preview, two 2M-row sources, one-to-one key | 1 s |
| CSV export, 2M rows | 5 s |
| XLSX export, 1,000,000 rows | 15 s |
| Workbook confirm, 100,000 rows by 10 columns | 3 s |
| Page at row 1,000,000 while a CSV export runs | 60 ms |

- **Where it lives.** The release-mode checks join `tests/perf_acceptance.rs` (ignored by default, run by hand), and the comparison script lives beside it. As in SP1, results are recorded in the run log, along with the margins under load (deviations 21 and 22).

## 10. Carried over from sub-project 1

| Item | Resolution |
|---|---|
| Deviation 4, TIMETZ offset | Fixed (section 5.4) |
| Deviation 13, unused `generation` | Removed (section 5.3) |
| Deviation 14, failed imports remembered in memory | Kept as accepted |
| Deviation 16, only page routes cancel | Extended (section 5.2) |
| Deviations 21 and 22, perf margins | New targets measured on a quiet machine; margins recorded |
| Deviation 24, self-test log-directory message | Fixed if the dependency allows it cheaply; otherwise kept as accepted |
| Deviation 25, CI cache near 10 GB | Cache keys revisited after the ICU build |
| `.wal` files left by evictions before `2a80026` | Swept at startup |
| The 501 fallback and its message | Replaced by FastAPI's answers once every route is served: 404 `{"detail": "Not Found"}` for an unknown path, and 405 `{"detail": "Method Not Allowed"}` for a wrong method on a known path |
| H2 manual steps | Run in this sub-project's exit gate (section 12) |

## 11. Sprints

The plan turns these into unit cards (about 150 lines and 3 files or fewer each). Every sprint ends green and demonstrable.

| # | Sprint | Cards | Green when |
|---|---|---|---|
| 0 | Reshape: SP1 code into the layout, the dependency extractors, the architecture test | 5, one after another | Behaviour unchanged, every suite green, layout enforced |
| 1 | Foundations: ICU test build and switch, read pool, response cache, pending-list harness, perf baselines, XLSX test build | 6 | Pages served during a slow query; time zones match Python; caps confirmed |
| 2 | Stats, category values, find | 5 | Profile panel, filter list and find work in the app |
| 3 | Charts: metric, bar and pie, histogram, box, scatter, line | 7 | Chart and Dashboard canvases work |
| 4 | Joins, attach, source path | 3 | Join flow works with several join Views open |
| 5 | Exports: CSV, XLSX, Parquet, JSON | 4 | Exports download on each OS |
| 6 | Workbooks | 4 | `.xlsx` upload, confirmation and offline queries work |
| 7 | Exit: perf gate, WebKit passes, SP1 leftovers, docs, H2 | 5 | Exit gate (section 12) |

- **Size.** About 39 cards, plus fix units.
- **Order.** Sprints 0 and 1 run first and in order: the reshape enables parallel work, and the foundations settle the riskiest items.
- **Parallel work.** Sprints 2 to 6 touch separate feature folders, so their cards can share batches.
- **Machine limits.** One workflow at a time, `jobs = 4`, and stress tests at 4 copies or fewer.

## 12. Exit gate

Sub-project 2 is done when:

1. `tests/rust_pending.txt` is empty and the full pytest suite passes in both modes. Rust mode skips `python_only`; Python mode skips `rust_only`.
2. `cargo test`, clippy, fmt and the architecture test pass in CI on all three OSes.
3. The performance gate (section 9) passes on a quiet machine, with the results recorded.
4. The WebKit passes (section 8.5) pass.
5. A bundle run builds the installers on all three OSes, and the transport self-test passes on each.
6. The H2 live test passes on macOS, Windows and Ubuntu with this sub-project's installers. It covers SP1's remaining steps (a full Windows run, creating a project and uploading into it on macOS, launching from the launcher icon on Ubuntu, re-checking SP1's H2 fixes) and every feature this sub-project ports, including an export saved to disk on each OS.
7. Every deviation from this spec is listed and signed off, as in SP1.
8. The pivot spec shows sub-project 2 as done.

## 13. Risks

| Risk | Mitigation |
|---|---|
| The CMake ICU build fails or is slow on one target, most likely universal macOS or Windows | Sprint 1 builds it before anything depends on it; per-OS fallback (section 5.4) |
| calamine and `read_xlsx` disagree on edge cases | Sprint 1 test build first; corpus tests run against both backends |
| Reads racing catalog changes in the read pool | Only the primary changes the catalog, under the mount lock; concurrency tests (section 8.3) |
| CI time and cache size grow with the CMake build | Measured in sprint 1; cache keys revisited |
| A webview cannot save a blob download | Checked in the WebKit pass and H2; minimal download handler (section 2) |
| Parallel agent builds overload the development Mac | One workflow at a time, `jobs = 4`, small stress tests |

## 14. Branches

- `rust-desktop/integration` holds sub-project 1 at `af7c0bd`.
- This sub-project lives on `rust-desktop/parity`, branched from integration.
- There is no merge into integration or main, and no pull request, without the user's go-ahead.
