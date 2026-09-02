# Quark

Local-first DuckDB data viewer: FastAPI backend, Svelte 5 frontend, server-side paging/filtering/sorting/deduplication, metadata-only Versions and Views, nullity gauges, and on-demand numeric, categorical, and date profiles.

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

Supported: CSV, TSV, Parquet, JSON, JSONL/NDJSON, XLSX (choose worksheets before they become datasets), DuckDB/DB. Legacy `.xls` files and `.sql` scripts are not accepted. New flat-file uploads use a safe filename stem as their SQL table name (`Claims v1.csv` → `claims_v1`); existing registrations keep their current names. First run: add a source, choose its dataset, then filter, profile, transform cells/columns, reorder or select columns by regex, join datasets, aggregate, or create a read-only SQL View.

Every dataset starts at Version 1. Column/cell transformations, visibility, ordering, and joins record one pending Version until **Stop recording** finalizes it. Aggregates and SQL are Views and do not increment the Version number. Versions and Views store only replayable SQL/JSON metadata in the browser—never duplicate row data.

An attached DuckDB path is opened read-only. Only attach paths you trust; Quark is an intentionally local, single-user tool.

## Checks

```bash
uv run pytest -q
cd frontend && npm test && npm run check && npm run build
```

See [`docs/SPEC.md`](docs/SPEC.md) for behavior and limits.
