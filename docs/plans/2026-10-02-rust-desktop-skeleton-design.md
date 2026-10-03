# Quark desktop, sub-project 1: walking skeleton

Date: 2026-10-02. Status: design, awaiting review. Part 1 of the 5-part Rust desktop pivot.

## 1. Context

Quark is a local-first DuckDB data viewer. Today a Svelte 5 single-page app talks to a single-file FastAPI backend (`backend/app.py`, about 2,000 lines and 30 routes) that owns DuckDB connections and query safety. It runs as a web server (uvicorn or Docker) and is used from a browser.

Quark is becoming a desktop-only application: a Tauri 2 shell around the existing Svelte UI, with the backend rewritten in Rust, on macOS, Windows and Ubuntu. Once the Rust app reaches parity, FastAPI, the Dockerfile and compose are retired.

The pivot is split into five sub-projects. Each gets its own design, plan and implementation.

| # | Sub-project | Outcome |
|---|---|---|
| 1 | Walking skeleton (this document) | Tauri app and Rust core on three OSes; projects, upload, paging, filter, sort and dedupe end to end; efficiency upgrades; CI |
| 2 | Backend parity | Every remaining endpoint in Rust; offline XLSX; the pytest suite moved to Rust |
| 3 | Native files and OS integration | Open files in place, native dialogs, drag and drop, native export save, file associations, single instance, default-app card |
| 4 | Release | Signing, notarization, installers, updates, app icon, version scheme |
| 5 | Retire Python | Remove FastAPI, Docker and pytest; update README, PRODUCT.md and SPEC.md |

## 2. Decisions

Made during brainstorming on 2026-10-01 and 2026-10-02:

- **Scope:** Tauri 2, the existing Svelte UI, and the backend ported to Rust. No Python runtime ships with the app.
- **Desktop only:** no headless or web mode after parity.
- **Platforms:** macOS 12+ as a universal build (Apple Silicon and Intel), Windows 10 and 11 on x64, Ubuntu 22.04+ on x64 as a `.deb`.
- **Transport:** the UI reaches the backend over HTTP on `127.0.0.1` with a per-launch token. A custom URL scheme and Tauri IPC were considered; the custom scheme is the fallback (section 11).
- **Efficiency upgrades in this sub-project:** count and null cache, stale-query cancellation, columnar cache, result cache. ART indexes were measured and rejected (section 6.7). Parallel reads are deferred to sub-project 2.
- **Code standard:** Rust code follows the repository's `rust-skills` guidelines.
- **Bundle identifier:** `ml.splitwire.quark`. It is effectively permanent; changing it later moves the data folder and breaks file associations.
- **DuckDB engine:** pinned to the version the Python side uses (1.5.4) so both backends behave the same during the port.
- **Later wants:** file associations for `.xlsx`, `.csv`, `.json` and `.parquet`, and an in-app "make Quark the default" recommendation. Both belong to sub-project 3.

## 3. Goals and non-goals

Goals:

1. Prove the risky parts before the bulk port: the webview-to-backend transport on WKWebView (macOS), WebView2 (Windows) and WebKitGTK (Ubuntu); DuckDB's bundled C++ build on all three OSes; Arrow pages decoding in the existing UI; installers building in CI.
2. Ship a production-quality vertical slice: create a project, upload a file, open a View, scroll, filter, sort, dedupe, record Versions, quit, relaunch and find everything again.
3. Make that slice faster than today (section 6).
4. Put the safety net in place: the pytest suite runnable against Rust, Rust unit and integration tests, and CI on three OSes.

Non-goals are listed with their owning sub-project in section 12.

## 4. Architecture

### 4.1 Repository layout

```
Cargo.toml                     virtual workspace: members, shared dependency versions, lints
rust-toolchain.toml            pinned stable toolchain
crates/quark-core/             the backend, with no Tauri dependency
  src/lib.rs                   AppState and router(state)
  src/bin/quark-dev-server.rs  the router on a TCP port, for tests and browser development
src-tauri/                     desktop shell (package `quark`)
  tauri.conf.json, capabilities/, icons/ (placeholder)
frontend/                      Svelte UI; only src/lib/api.ts changes
backend/                       Python backend, unchanged until sub-project 5
.github/workflows/ci.yml       new
```

There are two crates so that `cargo test -p quark-core` compiles without Tauri or WebKitGTK. Tests stay fast on every OS and the shell stays thin. There is no third crate.

### 4.2 Core modules

| Module | Responsibility |
|---|---|
| `state` | `AppState`: projects, sources, engines and background workers; loading and saving |
| `registry` | Reading and writing `projects.json` and `registry.json`, including legacy records |
| `engine` | One DuckDB database with its connection, generation counter and caches (section 4.3) |
| `mount` | Creating views for a source inside an engine: flat files, DuckDB files, columnar caches |
| `sql` | Identifier quoting, literal rendering, the SQL guard, the filter, sort and dedupe builder |
| `values` | DuckDB value to JSON value rules (Python's `safe()`) |
| `arrow` | Arrow IPC encoding with `quark` metadata and JSON-fallback columns |
| `cache` | Count and null cache, result cache, columnar cache |
| `api` | axum handlers and routes |
| `secure` | CORS, Host check and token check; used by the shell, tested without it |
| `error` | `ApiError`, rendered as a status code and `{"detail": ...}` |

These boundaries are a starting point; the plan may merge small modules.

### 4.3 Runtime model

- **Engines.** An engine is one DuckDB database the backend owns. It is either a project workspace (an in-memory database with one schema per mounted source dataset) or a legacy per-node database (default-project sources addressed by node id). Both use the same code, so every efficiency upgrade applies to both.
- **Serialization.** Each engine's DuckDB connection sits behind its own `std::sync::Mutex`, so one engine runs one query at a time, as today. Project and source metadata sit behind a separate catalog lock that is never held during a query, so an upload can register while a slow query runs (the existing pytest suite checks this). Handlers take locks inside `tokio::task::spawn_blocking` and never hold one across an `.await`. Locks recover from poisoning, so a panic in one request cannot wedge later ones. The connection lock carries a `ponytail:` comment naming its ceiling (one query at a time per engine) and its upgrade path (parallel reads, sub-project 2).
- **Background work.** Columnar imports and result materialization run on cloned connections or separate databases outside the lock, so a request never waits for a cache to finish building.
- **Rebuild, don't mutate.** When an engine's sources change, the engine is rebuilt. The old one stays alive through an `Arc` until work using it finishes. Each rebuild increments a generation number that is part of every cache key.

### 4.4 Main dependencies

`duckdb` pinned to `=1.10504.0` with features `bundled`, `parquet` and `json`; the Arrow IPC writer matching the Arrow version `duckdb` uses; `axum` with multipart; `tokio`; `tower-http` (CORS, panic catching); `serde` and `serde_json`; `thiserror`; `tracing`; `sha2`; `base64`; `uuid`; `chrono`. The shell uses `tauri` 2.12. Exact versions belong to the plan.

## 5. Backend slice

### 5.1 Endpoints

| Method | Path | Notes |
|---|---|---|
| GET | `/api/projects` | Default project first, then `projects.json`; each has `source_count` |
| POST | `/api/projects` | Name of 1 to 200 characters after trimming; 201 |
| GET | `/api/projects/{project_id}/sources` | `[{id, name}]` |
| GET | `/api/projects/{project_id}/sources/{source_id}` | The source plus its mounted `views` |
| GET | `/api/projects/{project_id}/views` | Mounts every source and returns all views |
| POST | `/api/projects/{project_id}/sources/upload` | Multipart field `file`; 201 |
| DELETE | `/api/projects/{project_id}/sources/{node_id}` | 204 |
| GET | `/api/nodes` | Legacy: default-project sources |
| POST | `/api/nodes/upload` | Legacy alias of upload into the default project |
| DELETE | `/api/nodes/{node_id}` | Legacy alias of delete |
| GET | `/api/nodes/{node_id}/datasets` | Datasets with their column names |
| POST | `/api/nodes/{node_id}/datasets/{dataset}/query` | Query builder over one dataset; JSON or Arrow |
| POST | `/api/nodes/{node_id}/sql` | One read-only SELECT plus filters, sorts and dedupe; JSON or Arrow |

In the last three routes, `node_id` accepts a project node id (`project_<id>`, meaning that project's workspace) or a legacy node id (default-project sources only), as today. The UI uses none of the four legacy routes (`/api/nodes`, `/api/nodes/upload`, `DELETE /api/nodes/{node_id}`, `/api/nodes/{node_id}/datasets`) or the dataset `/query` route. They are ported because most pytest tests create their data through them, which lets the existing tests check the Rust query builder. Whether they survive is decided in sub-project 2.

Every other `/api/*` request returns 501 with `{"detail": "Not in the desktop build yet"}`. The UI shows that through its existing error states.

Uploads accept `.csv`, `.tsv`, `.parquet`, `.json`, `.ndjson`, `.jsonl`, `.duckdb` and `.db`. An `.xlsx` upload returns 501 until sub-project 2. Anything else returns 400 "Unsupported file type". Upload bodies stream to disk in chunks with no size cap: axum's default 2 MB body limit is disabled on the upload routes only. A failed upload deletes its partial file.

### 5.2 What parity means

The Rust backend must match the Python backend on:

- **Responses:** shapes, field names, HTTP status codes, and `detail` messages for handled errors. Request-validation failures (unknown fields, wrong types, page below 1, page size outside 1 to 1000, project name length) return 422; their detail text may differ from pydantic's.
- **Identifiers:** node ids are uuid4 hex. Dataset ids are `base64url(json([schema, name]))` with compact JSON and no padding. View ids are `base64url(json([project_id, source_id, dataset_id]))`. Workspace schemas are `source_<base64url(json([source_id, dataset_id]))>`. Attached database aliases are `database_<base64url(source_id)>`. The UI stores view ids inside Versions, so these must be byte-identical.
- **Values:** compared after JSON parsing, not byte for byte.
- **Data folder formats** (section 5.3).

### 5.3 Data folder

The desktop app uses Tauri's `app_local_data_dir()`:

- macOS: `~/Library/Application Support/ml.splitwire.quark`
- Windows: `%LOCALAPPDATA%\ml.splitwire.quark` (Local rather than Roaming, because uploads can be gigabytes)
- Ubuntu: `~/.local/share/ml.splitwire.quark`

`QUARK_DATA_DIR` overrides it. The contents are unchanged from today: `projects.json` (every project except Default), `registry.json` (source records with id, name, kind, source path, and optional project_id, dataset_name and sheets) and `uploads/<node_id><ext>`.

At startup the backend reads both files, tolerating malformed content as today. A source is active when its file exists. The registry is never rewritten at startup, and files in `uploads/` that no registry entry references are deleted.

Sources open on first use rather than at launch. One consequence differs from Python: a file that exists but cannot be read now appears in the source list and errors when opened, where Python hid it. Uploads are still validated by opening them, as today: a file DuckDB cannot read returns 400 "Could not open source: ..." and is deleted.

An existing `./data` folder keeps working by pointing `QUARK_DATA_DIR` at it. There is no automatic migration.

### 5.4 DuckDB configuration

Every database the backend creates:

1. Sets `temp_directory` to `<app cache dir>/duckdb-tmp` (section 6.6).
2. Is locked before any user SQL reaches it, with an allow-list of exactly the files it may read, followed by `enable_external_access = false`. Per-node databases allow the source's folder through `allowed_directories`. Workspaces allow, through `allowed_paths`, the sorted source paths, the `.wal` sibling of each DuckDB file, and the columnar cache files that are ready.

Uploaded `.duckdb` and `.db` files are opened and attached read-only. Scan expressions are as today: `read_csv_auto(path, delim)` for CSV and TSV, `read_parquet` for Parquet, `read_json_auto` for JSON, NDJSON and JSONL.

### 5.5 SQL guard

Before user SQL is described or executed, `SELECT json_serialize_sql(?)` must report no error and exactly one statement. DuckDB serializes only SELECT statements, one entry per statement, so this rejects anything that is not a single SELECT, using DuckDB's own parser and no `unsafe` code.

Checked against DuckDB 1.5.4 on 21 statements, it agrees with Python's `extract_statements` plus SELECT-type check on all of them except the PRAGMA shorthand (`pragma version`), which it also rejects. That stricter behavior is accepted.

A rejected statement, an empty string, or more than one statement returns 422 "SQL accepts only one read-only SELECT query". A parse failure returns 422 "Invalid SQL query: Parser Error: <DuckDB's message>", which matches Python's text. Accepted SQL runs only inside `query(?)` with the text bound as a parameter, exactly as today. If a pytest guard case ever disagrees, the fallback is DuckDB's C API statement extraction in one small `unsafe` module.

### 5.6 Query building

Ported unchanged from `filtered_relation` and `controlled_query`:

- Operators: `=`, `!=`, `in`, `is_null`, `not_null`, `contains`, `starts_with`, `ends_with`, `>`, `>=`, `<`, `<=`, `between`, with today's validation and messages. Text operators apply only to text columns.
- Conditions fold left to right, each joined by its own `and` or `or` connector.
- Dedupe uses `QUALIFY row_number() OVER (PARTITION BY keys) = 1`. Keys must be unique, existing columns.
- Sorts apply in order. Unknown columns return 422.
- Values are bound parameters. Identifiers are validated against `DESCRIBE` output and double-quoted.
- The response's `sql` field is the display form, with literals inlined by `literal()`'s rules (NULL, TRUE and FALSE, integers, Python-style float repr, quoted strings), when filters, sorts or dedupe are present. Otherwise it is the original SQL, trimmed. Filter values arrive as JSON, which cannot carry NaN or infinity, so Python's `'NaN'::DOUBLE` and `'Infinity'::DOUBLE` branches are not needed.
- Each page reports total rows, the page itself (`LIMIT ? OFFSET ?`), the null fraction of every column, elapsed milliseconds to three decimals, and the page count. Section 6 caches parts of this.

### 5.7 Response encoding

**JSON** is the default: `{columns, rows, page, page_size, total_rows, total_pages, elapsed_ms, sql}`. Each entry of `columns` is `{name, type, numeric, profile_kind, null_fraction}`, with type names as DuckDB prints them.

**Arrow** is used when `Accept` contains `application/vnd.apache.arrow.stream`. The response is one IPC stream with `Vary: Accept`. Schema metadata under the key `quark` holds the same metadata minus `rows`, plus `json_columns`.

- Columns of type BOOLEAN, TINYINT through UBIGINT, FLOAT, REAL, DOUBLE, VARCHAR and BLOB keep DuckDB's native Arrow arrays. They stay native even when other columns on the same page need the fallback; Python converted the whole page in that case.
- Every other column becomes UTF-8 with one JSON text per cell, `null` included, and is listed in `json_columns`.

Value rules, shared by JSON rows and JSON-fallback cells:

| DuckDB value | JSON |
|---|---|
| Integer within plus or minus (2^53 - 1) | Number |
| Integer beyond that, including HUGEINT and UHUGEINT | Decimal string |
| FLOAT or DOUBLE that is NaN or infinite | `null` |
| DECIMAL | Number via f64; non-finite becomes `null` |
| DATE, TIME, TIMESTAMP | ISO 8601 as Python's `isoformat()`: `T` separator, microseconds only when non-zero |
| TIMESTAMPTZ | ISO 8601 in the machine's local time zone, with offset |
| BLOB | Lowercase hex |
| UUID, ENUM, JSON, BIT | String |
| INTERVAL | Total seconds as a number |
| LIST, ARRAY | Array, elements converted recursively |
| STRUCT, MAP | Object with string keys, values converted recursively |

The pytest suite referees edge cases.

### 5.8 Errors and logging

`ApiError { status, detail }` renders as `{"detail": ...}`.

- DuckDB errors while executing a query return 422 "Invalid filter value: ..." on `/query` and "Invalid SQL query: ..." on `/sql`, as today.
- A source that fails to open returns 400 "Could not open source: ...".
- Handler panics are caught by tower-http's `CatchPanicLayer` and return 500 `{"detail": "Internal error"}`. Release builds keep unwinding (no `panic = "abort"`) so one bad request cannot close the window.

Logging uses `tracing`: request path, status, duration and full error chains go to a log file in the OS log folder. Row values are never logged, and SQL text only at debug level.

## 6. Efficiency upgrades

### 6.1 Evidence

Measured on a synthetic CSV (2,000,000 rows, 10 columns, 234 MB) with DuckDB 1.5.4 on the Apple Silicon development Mac: filter `qty > 10`, a page of 100 rows at offset 1,000,000, best of three runs.

| Step | Live CSV (today) | Columnar copy |
|---|---|---|
| `count(*)` | 183 ms | 0.9 ms |
| Null fractions, 10 columns | 316 ms | 11.9 ms |
| Page | 665 ms | 45.5 ms |
| Page sorted by price | 267 ms | 101.6 ms |
| Total per page today (count, nulls and page) | 1,164 ms | 58 ms |

Importing the CSV into a DuckDB file took 1.26 s and 80 MB (34% of the CSV). Saving a sorted result took 138 ms once; any page from it afterwards took about 1 ms.

### 6.2 Count and null cache

- **Scope:** an LRU of 256 entries per engine.
- **Key:** the engine generation, the filtered relation's SQL without ORDER BY, and the bound values.
- **Value:** column metadata, null fractions and total rows.

Sorting and paging never change these values. After the first page, every page and every sort order of the same filter set skips both full scans. The cache is cleared when the engine is rebuilt.

### 6.3 Stale-query cancellation

Before running, each database request takes the connection's DuckDB interrupt handle and holds it in a guard.

- If the HTTP request is dropped before the query finishes (the UI aborted the fetch after a fast scroll, a filter change or a closed View), the guard's `Drop` interrupts the query. That frees the lock for the next request.
- A completed query disarms the guard.
- Background jobs (sections 6.4 and 6.5) are interrupted when superseded or evicted, and at shutdown.

### 6.4 Columnar cache

- **Applies to** CSV, TSV, JSON, NDJSON and JSONL sources. Parquet and DuckDB files are already columnar.
- **Location:** `<app cache dir>/columnar/<key>.duckdb`. The key is the SHA-256 of the absolute path, file size, modified time in nanoseconds, DuckDB version and scan expression, so a source edited in place gets a new key.
- **Building:** one background worker runs one import at a time, in FIFO order. A separate DuckDB database runs `CREATE TABLE data AS SELECT * FROM <scan expression>` into `<key>.duckdb.partial` and then renames it into place, so a crash never leaves a half-written file that looks valid.
- **Switching:** until its import finishes, a source is mounted live, exactly as today. When it finishes, every engine mounting that source is rebuilt (generation plus one) and its views read the cache, attached read-only. Rows, order, types and counts are identical, because CREATE TABLE AS keeps insertion order and the sniffed types. The property test in section 8.3 enforces this.
- **Restarts:** after a relaunch, a source with a valid cache mounts from the cache the first time it is used, with no live phase.
- **Failures:** a failed import is logged, the source stays live, and that key is not retried until the file changes.
- **Removal:** deleting a source deletes its cache entries. When the folder exceeds 10 GB, the least recently used entries are deleted. The cap carries a `ponytail:` comment; it becomes a setting only if needed.

The cache is a disposable, derived file in the OS cache folder. It never reaches the browser, is never shown as data, never touches the source file, and deleting it only costs a rebuild. Sub-project 5 adds this clarification to PRODUCT.md's "never duplicate the data" principle.

### 6.5 Result cache

- **Trigger:** the second request for the same key. The key is the engine generation, the full query SQL including ORDER BY, and the bound values. The first request is always served directly.
- **Building:** a background job on a cloned connection runs `CREATE TABLE quark_results.r_<hash> AS <query>` in the engine's database. It is a regular table in a dedicated schema, so future parallel readers can see it.
- **Serving:** pages are served directly until the table is ready. After that, each page is `SELECT * FROM quark_results.r_<hash> LIMIT ? OFFSET ?`. DuckDB's default insertion-order preservation keeps the sorted order, so that setting must never be turned off.
- **Skipped** for plain base-View scans: no filters, sorts or dedupe, and SQL equal to the View's own `SELECT * FROM ...`.
- **Limits:** at most 4 results per engine (LRU) and one build in flight per engine; a newer trigger interrupts an older build. Results live in memory and spill to the spill folder. All of them are dropped with the engine.

### 6.6 Always on

- **Spill folder:** every database sets `temp_directory` to `<app cache dir>/duckdb-tmp`, which is cleared at startup. Packaged apps start in a working directory they cannot write to, where DuckDB's default `.tmp` would make large sorts fail.
- **Native Arrow columns stay native** (section 5.7).
- **Release profile:** `opt-level = 3`, `lto = "fat"`, `codegen-units = 1`, `strip = true`. The panic strategy stays `unwind` (section 5.8).

### 6.7 Rejected: ART indexes

Measured on the columnar copy, with an ART index on each filtered column:

| Query | No index | ART index |
|---|---|---|
| `hash = x` (1 row) | 7.0 ms | 0.4 ms |
| `price > 999` (about 0.1%) | 3.7 ms | 89.2 ms |
| `cat IN (2 of 12)` | 3.4 ms | 12.2 ms |
| `contains(name, ...)` | 6.3 ms | 5.8 ms (same sequential scan; the gap is run-to-run noise) |

DuckDB's ART index helps only single-row lookups. It slowed the range and `IN` filters here, it cannot serve `contains`, and it is never used for ORDER BY. The min/max zone maps the columnar copy gets for free already keep filters at 3 to 8 ms. The cost that repeats is sorting, which section 6.5 addresses.

## 7. Desktop shell, transport and security

### 7.1 What a user sees

A user installs Quark once (`.dmg`, `.exe` or `.msi`, or `.deb`) and then clicks the Quark icon. Nobody types a command or installs Python, and the port and token are invisible.

### 7.2 Startup

In Tauri's `setup`, before the window opens:

1. Resolve the data folder (section 5.3) and the cache folder (`app_cache_dir()`). On Windows the cache folder is the same folder as local data, so the cache always uses its own subfolders, `columnar/` and `duckdb-tmp/`.
2. Load `AppState` (section 5.3) and start the background import worker.
3. Bind `127.0.0.1:0`, letting the OS pick the port, and generate a 256-bit token from the OS random source.
4. Serve the router, wrapped in the guards of section 7.3, on Tauri's tokio runtime.
5. Create the main window (title "Quark", 1440 by 900, minimum 1024 by 680) with an initialization script that runs before any page script:

```js
window.__QUARK_API__ = Object.freeze({ base: "http://127.0.0.1:<port>", token: "<hex>" });
```

On exit, background jobs are interrupted and engines closed.

### 7.3 Request guards

These live in the core `secure` module. From outermost to innermost:

1. **CORS.** Allowed origins are exactly `tauri://localhost` and `http://tauri.localhost`, plus `http://localhost:5173` in debug builds. Methods GET, POST and DELETE; headers `authorization`, `content-type` and `accept`; preflight max-age of 24 hours, so scrolling does not double every request.
2. **Host.** The Host header must equal `127.0.0.1:<port>`, otherwise 403. This blocks DNS-rebinding attacks.
3. **Token.** `Authorization: Bearer <token>`, compared in constant time, otherwise 401. A website cannot attach this header without a preflight our CORS refuses, and other users on the machine do not have the token.

Panic catching (section 5.8) sits inside these guards.

### 7.4 Webview lockdown

- Navigation is limited to the app's own origin. The UI has no external links, raw HTML or `window.open`, and this keeps the token from ever reaching another page.
- Content Security Policy: `default-src 'self'; connect-src 'self' http://127.0.0.1:*; img-src 'self' data: blob:; font-src 'self' data:; style-src 'self' 'unsafe-inline'`. Inline styles are allowed because Svelte and CodeMirror set them.
- The skeleton needs no Tauri plugins and no IPC commands, so capabilities stay minimal.

### 7.5 Frontend changes

Only `frontend/src/lib/api.ts` changes:

- Requests go to `(window.__QUARK_API__?.base ?? '') + path`, with `Authorization: Bearer <token>` added when `__QUARK_API__` is present. In a normal browser, relative `/api` paths work as today, through the Vite proxy to Python or to `quark-dev-server`.
- `exportData` reuses `responseFor` instead of keeping its own copy of the fetch and error parsing.
- `__QUARK_API__` gets a type declaration.

Versions, dashboards and settings stay in `localStorage`. The Tauri page origin is stable across launches, so they persist.

### 7.6 Developer workflow

- `cargo tauri dev` opens the app window with the Vite dev server (live UI reload) and the Rust backend.
- `quark-dev-server [--data-dir DIR] [--cache-dir DIR] [--port N]` serves the bare router on `127.0.0.1`. Defaults: port 8000, data folder `./data`, cache folder `<data dir>/cache`. It has no token and is never bundled. Its first line of output is always `QUARK_LISTENING 127.0.0.1:<port>`, which tells a test the port it got when it asked for port 0. It must not share a data folder with a running Python server.
- **Transport self-test.** Launching the app with `QUARK_SELF_TEST=1` makes the window, once its page has loaded, call `/api/projects` through the real webview, CORS, Host and token path. The app then exits with 0 on success, or 1 if that hasn't succeeded within 30 seconds. Agents and CI can verify the transport on each OS this way without clicking anything. The hook is inert unless the variable is set.
- The Python backend keeps working until sub-project 5.

## 8. Testing

### 8.1 pytest as the parity check

- Setting `QUARK_TEST_BACKEND=rust` switches the `client` fixture to a spawned `quark-dev-server` (`--data-dir <tmp> --port 0`) and an `httpx.Client` pointed at it. The binary path comes from `QUARK_DEV_SERVER`, defaulting to `target/debug/quark-dev-server`. Without the variable, tests use Python's `TestClient` as today.
- The 18 inline `TestClient(create_app(...))` uses become a `make_client(path)` factory. In Rust mode, each use starts a fresh server on the same folder, which is a real restart.
- Tests that patch Python internals (`monkeypatch` of `backend_app.duckdb`, `patch.object(backend_app, ...)`) are marked `python_only`. Tests that only use Python helpers to build expected values (`safe`, `ARROW_MEDIA_TYPE`) run in both modes.
- Tests whose routes all fall within section 5.1 are marked `skeleton`. The gate is `QUARK_TEST_BACKEND=rust uv run pytest -m skeleton` passing. The plan lists the exact set.
- Markers are registered in `pyproject.toml`. No assertion changes.

### 8.2 Rust tests

Unit tests, in `#[cfg(test)]` modules:

- The SQL guard, as a table of the 21 checked statements.
- The filter, sort and dedupe builder: SQL text and bound values.
- `literal()` rendering, including Python's float repr.
- The value rules of section 5.7.
- Id encodings, byte-equal to Python's.
- Registry reading and writing, including legacy records and malformed files.
- The engine version: `SELECT version()` equals the `duckdb` version in `uv.lock`.

Integration tests, in `crates/quark-core/tests/`, drive the router in process with `tower::ServiceExt::oneshot` unless noted:

- **Guards:** allowed and refused origins, preflight, wrong Host returns 403, missing or wrong token returns 401.
- **Arrow:** responses decode, with `quark` metadata and correct `json_columns`.
- **Upload:** a file larger than 2 MB streams through.
- **Count and null cache:** a second page skips the count query, observed through a test-only counter.
- **Cancellation, over a real TCP connection:** a client that aborts a long `range()` query frees the backend for the next request within 100 ms.
- **Columnar cache:** build; rebuild after a modified-time change; eviction at the cap; deletion with its source.
- **Result cache:** triggered by the second request; LRU of 4; superseded builds interrupted; dropped when the engine is rebuilt.

### 8.3 Cache property test

Over a fixed set of filter, sort, dedupe and offset combinations on a generated file, pages served through the caches must equal pages from direct, uncached queries. This holds before the columnar switch, after it, and when pages come from materialized results. Sort keys in this test are unique, because ties have no defined order in DuckDB, today or after this change.

### 8.4 Frontend

`npm test`, `npm run check` and `npm run build` keep passing. One new Node test covers `api.ts` URL and header behavior with and without `__QUARK_API__`.

### 8.5 Manual smoke test

On each OS, using the CI-built installer:

1. Install and launch from the icon.
2. Create a project, upload the 2M-row CSV and open its View.
3. Scroll to the end, sort, add and remove filters, and dedupe.
4. Record a Version and Stop recording.
5. Quit and relaunch. The project, source, View and Versions are back, and deep pages are fast at once because the columnar cache is reused.
6. Open an unported feature such as column stats. The UI shows the 501 message and keeps working.

The Tauri UI test driver does not support macOS, so UI checks stay manual for this sub-project.

### 8.6 Performance acceptance

On the development Mac, with a release build and the 2M-row CSV:

- The first page of a new CSV takes about 0.5 s, today's cost, paid once.
- After the columnar import, any page takes at most 60 ms.
- The second and later pages of a sorted query take at most 5 ms.
- `npm run benchmark:arrow` passes with `QUARK_SCROLL_MAX_MS=200` against `quark-dev-server`.

These run locally as an ignored Rust test plus the existing benchmark, not in CI, because runner speed varies.

## 9. CI and builds

One workflow, `.github/workflows/ci.yml`, runs on pull requests, on pushes to `main` and to `rust-desktop/**` branches (the pivot is not merged anywhere yet, so its branches need CI of their own), and on manual runs.

| Job | Runs on | Steps |
|---|---|---|
| core | macos-latest, windows-latest, ubuntu-22.04 | `cargo test -p quark-core`; build `quark-dev-server`; `QUARK_TEST_BACKEND=rust uv run pytest -m skeleton` |
| lint | ubuntu-22.04 | `cargo fmt --check`; `cargo clippy --workspace -- -D warnings` with workspace-level lints (WebKitGTK headers installed) |
| python | ubuntu-22.04 | `uv run pytest` in Python mode |
| frontend | ubuntu-22.04 | `npm ci`, `npm test`, `npm run check`, `npm run build` |
| bundle | macos-latest, windows-latest, ubuntu-22.04 | Pushes to main and manual runs only; unsigned installers uploaded as artifacts; then the built app runs the transport self-test (under `xvfb-run` on Ubuntu) |

The core job runs on all three OSes because Windows paths (backslashes, drive letters in `allowed_paths`) are the likeliest thing to break.

Bundle targets:

- **macOS:** `universal-apple-darwin`, built as `.app` and `.dmg`, minimum macOS 12.0, with `MACOSX_DEPLOYMENT_TARGET=12.0` so the DuckDB C++ build matches.
- **Windows:** NSIS `.exe` and `.msi`. The installer downloads the WebView2 runtime only when it is missing.
- **Ubuntu:** `.deb`, built on 22.04 so it also runs on 24.04 and 26.04. There is no AppImage, because AppImages cannot register file associations, which sub-project 3 needs.

Build cost and housekeeping:

- `Swatinem/rust-cache` caches each OS and target. A cold DuckDB compile takes roughly 5 to 10 minutes per target, and the universal macOS build compiles it twice, which is why installers build only on main.
- `rust-toolchain.toml` pins stable Rust, and `rust-version` is declared.
- The app version is `0.1.0`.
- The icon is a neutral placeholder generated with `cargo tauri icon`. The repository has no brand assets, and PRODUCT.md forbids inventing them, so the real icon is a design task before sub-project 4.

## 10. Exit criteria

The skeleton is done when:

1. CI is green on all three OSes: core tests, `pytest -m skeleton` against Rust, the Python suite in Python mode, and the frontend checks.
2. CI has built installers for all three OSes, the transport self-test (section 7.6) passes on each, and the manual smoke test (section 8.5) passes on each.
3. The performance acceptance (section 8.6) is met.
4. The cancellation test (section 8.2) passes.

## 11. Risks and fallbacks

| Risk | How it shows | Fallback |
|---|---|---|
| WKWebView blocks fetches from `tauri://localhost` to `http://127.0.0.1` | First macOS smoke test | A custom `quark://` scheme into the same router; only one adapter file changes |
| hyper does not notice an aborted HTTP/1.1 request while its handler waits | Cancellation test | A newer request on the same engine interrupts older in-flight queries |
| The universal macOS build is too slow or breaks | Bundle job | Separate arm64 and x64 `.dmg` files |
| The Arrow IPC writer's version differs from the Arrow version `duckdb` uses | Build | Enable IPC on the exact Arrow version `duckdb` depends on |
| The JSON-based SQL guard disagrees with a pytest guard case | pytest in Rust mode | DuckDB C API statement extraction in one small `unsafe` module |
| Unsigned installers trigger Gatekeeper and SmartScreen warnings | Expected | Resolved in sub-project 4 |
| The columnar cache uses a lot of disk | Cache folder size | 10 GB LRU cap; the OS may also clear its cache folder, and caches rebuild on demand |

## 12. Deferred

- **Sub-project 2, backend parity:** column stats, category values, charts, joins, exports, cell find and attach; XLSX, including an offline replacement for `INSTALL excel`; ICU versus Rust-side time-zone handling; parallel reads; whether the legacy routes survive; moving the remaining tests to Rust.
- **Sub-project 3, native files and OS integration:** opening files in place; native open and save dialogs; drag and drop from the OS; exports saved by the backend; file associations for `.xlsx`, `.csv`, `.json` and `.parquet`; single instance with open-file routing; which project a double-clicked file lands in; the "make Quark the default" card (macOS and Ubuntu can set it directly; Windows can only open its Settings page).
- **Sub-project 4, release:** Apple Developer ID signing and notarization, Windows code signing, the update channel, the real icon, the version scheme.
- **Sub-project 5, retire Python:** delete the Python backend, pytest and Docker files; update README, PRODUCT.md (Tauri and Rust, and the cache clarification) and SPEC.md.

## 13. Open questions carried forward

None of these block this sub-project.

- The "recommendation widget" is read as an in-app card suggesting Quark as the default app for its file types, the way browsers do. To be confirmed in sub-project 3.
- Which project a double-clicked file lands in.
- Whether `.tsv`, `.jsonl`, `.ndjson` and `.duckdb` appear under "Open with" without being offered as a default.
- When the default-app card appears, and how dismissing it persists.
