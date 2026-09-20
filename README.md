# Quark

Local-first DuckDB data viewer: projects contain sources, every table or derived query is a versioned View, and FastAPI/Svelte provide server-side exploration without copying result data.

## Run for development

```bash
uv sync
(cd frontend && npm install)
uv run uvicorn backend.app:app --host 0.0.0.0 --reload
```

In another terminal:

```bash
cd frontend
npm run dev
```

Open `http://localhost:5173`.

## Production build

```bash
cd frontend && npm run build && cd ..
uv run uvicorn backend.app:app --host 0.0.0.0 --port 8000
```

Open `http://127.0.0.1:8000`. FastAPI serves `frontend/dist` when built.

## Docker

```bash
docker compose up -d --build
```

Open `http://127.0.0.1:8000`. Uploads persist in `./data`.

## Data

By default uploads and the node registry live in `./data`. Override with:

```bash
QUARK_DATA_DIR=/absolute/path uv run uvicorn backend.app:app --host 0.0.0.0
```

Supported: CSV, TSV, Parquet, JSON, JSONL/NDJSON, XLSX (choose worksheets before they become Views), DuckDB/DB. Legacy `.xls` files and `.sql` scripts are not accepted. New flat-file uploads use a safe filename stem as their SQL table name (`Claims v1.csv` → `claims_v1`); existing registrations keep their current names. First run: create a project, add a source, choose a View, then filter, profile, transform cells/columns, reorder or select columns by regex, join Views, aggregate, or create a read-only SQL View.

Every View starts at Version 1. Column/cell transformations, visibility, and ordering record one pending Version until **Stop recording** finalizes it. Joins, aggregates, and SQL create new Views; later changes append Versions to that derived View, never to its source. Versions store only replayable SQL/JSON metadata in the browser—never duplicate row data.

An attached DuckDB path is opened read-only. Only attach paths you trust; Quark is an intentionally local, single-user tool.

## Checks

```bash
uv run pytest -q
cd frontend && npm test && npm run check && npm run build
```

The scrolling browser regression suite starts its own frontend server and mocks every API (no user data is changed). Run `cd frontend && npm run test:scroll-browser` with Playwright installed, or set `PLAYWRIGHT_MODULE` to an existing Playwright module path.

The chart scrolling regression uses the same mocked-server setup: run `cd frontend && npm run test:chart-scroll-browser`; add `MINIMAL=1` for the 20-bar boundary case.

The scatter browser regression uses the same mocked-server setup: run `cd frontend && npm run test:scatter-browser`; add `SCATTER_SINGLE=1`, `SCATTER_EMPTY=1`, or `SCATTER_ERROR=1` for focused data, empty, and failed-request cases.

With the backend running, `cd frontend && npm run benchmark:arrow` compares JSON and Arrow against the registered AllSpecs source, including a 50-column projection. It checks cell parity and real scrolling, and reports payload bytes, fetch latency, decode plus viewport access, and scroll-to-paint medians. It does not change source data. The same `PLAYWRIGHT_MODULE` option applies; `QUARK_API_URL` and `QUARK_BENCH_SOURCE` can select another backend/source (the scroll test needs more than 11,000 rows).

Set `QUARK_SCROLL_MAX_MS=200` to make that benchmark fail if either the Arrow scroll-to-paint median or held-preview latency exceeds 200 ms. XLSX sources are imported into typed DuckDB tables once per connection so scrolling does not repeatedly parse Excel.

See [`docs/SPEC.md`](docs/SPEC.md) for behavior and limits.
