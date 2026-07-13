# DuckScope

Local-first DuckDB data viewer: FastAPI backend, Svelte 5 frontend, server-side paging/filtering/sorting, nullity gauges, and numeric distributions.

## Run for development

```bash
uv sync
(cd frontend && npm install)
uv run uvicorn backend.app:app --reload
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
uv run uvicorn backend.app:app --host 127.0.0.1 --port 8000
```

Open `http://127.0.0.1:8000`. FastAPI serves `frontend/dist` when built.

## Data

By default uploads and the node registry live in `./data`. Override with:

```bash
DUCKSCOPE_DATA_DIR=/absolute/path uv run uvicorn backend.app:app
```

Supported: CSV, TSV, Parquet, JSON, NDJSON, DuckDB/DB.

An attached DuckDB path is opened read-only. Only attach paths you trust; DuckScope is an intentionally local, single-user tool.

## Checks

```bash
uv run pytest -q
cd frontend && npm run check && npm run build
```

See [`docs/SPEC.md`](docs/SPEC.md) for behavior and limits.
