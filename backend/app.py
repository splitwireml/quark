import base64
import datetime as dt
import json
import math
import os
import time
import uuid
from contextlib import asynccontextmanager
from decimal import Decimal
from pathlib import Path
from typing import Any, Literal

import duckdb
from fastapi import FastAPI, File, HTTPException, Query as QueryParam, UploadFile
from fastapi.responses import Response
from fastapi.staticfiles import StaticFiles
from pydantic import BaseModel, ConfigDict, Field

SUPPORTED = {".csv", ".tsv", ".parquet", ".json", ".ndjson", ".duckdb", ".db"}
NUMERIC = ("TINYINT", "SMALLINT", "INTEGER", "BIGINT", "HUGEINT", "UTINYINT", "USMALLINT", "UINTEGER", "UBIGINT", "UHUGEINT", "FLOAT", "REAL", "DOUBLE", "DECIMAL")
INTEGER = ("TINYINT", "SMALLINT", "INTEGER", "BIGINT", "HUGEINT", "UTINYINT", "USMALLINT", "UINTEGER", "UBIGINT", "UHUGEINT")
TEXT = ("VARCHAR", "CHAR", "TEXT")
OPERATORS = {"=", "!=", "in", "is_null", "not_null", "contains", "starts_with", "ends_with", ">", ">=", "<", "<="}


class AttachRequest(BaseModel):
    model_config = ConfigDict(extra="forbid")
    path: str


class Filter(BaseModel):
    model_config = ConfigDict(extra="forbid")
    column: str
    operator: str
    value: Any = None


class Sort(BaseModel):
    model_config = ConfigDict(extra="forbid")
    column: str
    direction: Literal["asc", "desc"]


class Query(BaseModel):
    model_config = ConfigDict(extra="forbid")
    page: int = Field(1, ge=1)
    page_size: int = Field(100, ge=1, le=1000)
    filters: list[Filter] = []
    sorts: list[Sort] = []


def quote(value: str) -> str:
    return '"' + value.replace('"', '""') + '"'


def safe(value: Any) -> Any:
    if isinstance(value, int) and not -(2**53 - 1) <= value <= 2**53 - 1:
        return str(value)
    if isinstance(value, float):
        return value if math.isfinite(value) else None
    if isinstance(value, Decimal):
        converted = float(value)
        return converted if math.isfinite(converted) else None
    if isinstance(value, (dt.date, dt.time, dt.datetime)):
        return value.isoformat()
    if isinstance(value, (bytes, bytearray, memoryview)):
        return bytes(value).hex()
    if isinstance(value, (list, tuple)):
        return [safe(item) for item in value]
    if isinstance(value, dict):
        return {str(key): safe(item) for key, item in value.items()}
    return value


def create_app(data_dir: str | Path | None = None) -> FastAPI:
    root = Path(data_dir or os.getenv("DUCKSCOPE_DATA_DIR", "data")).expanduser().resolve()
    registry_path = root / "registry.json"
    uploads = root / "uploads"
    nodes: dict[str, dict[str, str]] = {}
    connections: dict[str, duckdb.DuckDBPyConnection] = {}

    def public(node: dict[str, str]) -> dict[str, str]:
        return {key: node[key] for key in ("id", "name", "kind", "source")}

    def save_registry() -> None:
        root.mkdir(parents=True, exist_ok=True)
        temporary = registry_path.with_suffix(".tmp")
        temporary.write_text(json.dumps([public(node) for node in nodes.values()], indent=2))
        temporary.replace(registry_path)

    def connect(node: dict[str, str]) -> duckdb.DuckDBPyConnection:
        source = Path(node["source"])
        suffix = source.suffix.lower()
        if suffix in {".duckdb", ".db"}:
            return duckdb.connect(str(source), read_only=True)
        con = duckdb.connect()
        source_sql = str(source).replace("'", "''")
        if suffix in {".csv", ".tsv"}:
            delimiter = "\\t" if suffix == ".tsv" else ","
            scan = f"read_csv_auto('{source_sql}', delim='{delimiter}')"
        elif suffix == ".parquet":
            scan = f"read_parquet('{source_sql}')"
        else:
            scan = f"read_json_auto('{source_sql}')"
        con.execute(f"CREATE VIEW data AS SELECT * FROM {scan}")
        return con

    def add(node: dict[str, str]) -> dict[str, str]:
        try:
            con = connect(node)
            datasets_for(con)
        except Exception as exc:
            raise HTTPException(400, f"Could not open source: {exc}") from exc
        nodes[node["id"]] = node
        connections[node["id"]] = con
        save_registry()
        return public(node)

    def get_connection(node_id: str) -> duckdb.DuckDBPyConnection:
        if node_id not in nodes:
            raise HTTPException(404, "Node not found")
        return connections[node_id]

    def datasets_for(con: duckdb.DuckDBPyConnection) -> list[dict[str, str]]:
        rows = con.execute("""
            SELECT schema_name, table_name, 'TABLE' FROM duckdb_tables()
            WHERE NOT internal AND schema_name NOT IN ('information_schema', 'pg_catalog')
            UNION ALL
            SELECT schema_name, view_name, 'VIEW' FROM duckdb_views()
            WHERE NOT internal AND schema_name NOT IN ('information_schema', 'pg_catalog')
            ORDER BY 1, 2
        """).fetchall()
        return [{
            "id": base64.urlsafe_b64encode(json.dumps([schema, name], separators=(",", ":")).encode()).rstrip(b"=").decode(),
            "name": name,
            "schema": schema,
            "type": kind,
        } for schema, name, kind in rows]

    def metadata(con: duckdb.DuckDBPyConnection, dataset: str) -> tuple[str, list[tuple[str, str]]]:
        item = next((item for item in datasets_for(con) if item["id"] == dataset), None)
        if item is None:
            raise HTTPException(404, "Dataset not found")
        table = f'{quote(item["schema"])}.{quote(item["name"])}'
        columns = [(row[0], row[1]) for row in con.execute(f"DESCRIBE SELECT * FROM {table}").fetchall()]
        return table, columns

    @asynccontextmanager
    async def lifespan(_: FastAPI):
        root.mkdir(parents=True, exist_ok=True)
        uploads.mkdir(exist_ok=True)
        if registry_path.exists():
            try:
                stored = json.loads(registry_path.read_text())
            except (json.JSONDecodeError, OSError):
                stored = []
            for node in stored if isinstance(stored, list) else []:
                if not isinstance(node, dict) or not Path(node.get("source", "")).is_file():
                    continue
                try:
                    connections[node["id"]] = connect(node)
                    datasets_for(connections[node["id"]])
                    nodes[node["id"]] = node
                except Exception:
                    connections.pop(node.get("id", ""), None)
            save_registry()
        yield
        for con in connections.values():
            con.close()

    api = FastAPI(title="DuckScope", lifespan=lifespan)

    @api.get("/api/nodes")
    def list_nodes():
        return [public(node) for node in nodes.values()]

    @api.post("/api/nodes/upload", status_code=201)
    async def upload(file: UploadFile = File(...)):
        name = Path(file.filename or "").name
        suffix = Path(name).suffix.lower()
        if suffix not in SUPPORTED:
            raise HTTPException(400, "Unsupported file type")
        node_id = uuid.uuid4().hex
        destination = uploads / f"{node_id}{suffix}"
        try:
            with destination.open("wb") as output:
                while chunk := await file.read(1024 * 1024):
                    output.write(chunk)
            return add({"id": node_id, "name": name, "kind": "upload", "source": str(destination)})
        except Exception:
            destination.unlink(missing_ok=True)
            raise
        finally:
            await file.close()

    @api.post("/api/nodes/attach", status_code=201)
    def attach(request: AttachRequest):
        source = Path(request.path).expanduser().resolve()
        if source.suffix.lower() not in {".duckdb", ".db"}:
            raise HTTPException(400, "Only .duckdb and .db paths can be attached")
        if not source.is_file():
            raise HTTPException(404, "Database not found")
        return add({"id": uuid.uuid4().hex, "name": source.name, "kind": "attached", "source": str(source)})

    @api.delete("/api/nodes/{node_id}", status_code=204)
    def delete_node(node_id: str):
        if node_id not in nodes:
            raise HTTPException(404, "Node not found")
        node = nodes.pop(node_id)
        connections.pop(node_id).close()
        if node["kind"] == "upload":
            Path(node["source"]).unlink(missing_ok=True)
        save_registry()
        return Response(status_code=204)

    @api.get("/api/nodes/{node_id}/datasets")
    def list_datasets(node_id: str):
        return datasets_for(get_connection(node_id))

    @api.post("/api/nodes/{node_id}/datasets/{dataset}/query")
    def query(node_id: str, dataset: str, request: Query):
        started = time.perf_counter()
        con = get_connection(node_id)
        table, columns = metadata(con, dataset)
        column_types = dict(columns)
        column_names = set(column_types)
        clauses: list[str] = []
        values: list[Any] = []
        for condition in request.filters:
            if condition.column not in column_names or condition.operator not in OPERATORS:
                raise HTTPException(422, "Invalid filter column or operator")
            if condition.operator not in {"is_null", "not_null"} and condition.value is None:
                raise HTTPException(422, "Filter value is required")
            if condition.operator == "in" and (not isinstance(condition.value, list) or not condition.value):
                raise HTTPException(422, "IN filter requires a non-empty list")
            if condition.operator in {"contains", "starts_with", "ends_with"} and not column_types[condition.column].upper().startswith(TEXT):
                raise HTTPException(422, "Text operator requires a text column")
            column = quote(condition.column)
            if condition.operator in {"is_null", "not_null"}:
                clauses.append(f"{column} IS {'NOT ' if condition.operator == 'not_null' else ''}NULL")
            elif condition.operator == "in":
                clauses.append(f"{column} IN ({', '.join('?' for _ in condition.value)})")
                values.extend(condition.value)
            elif condition.operator in {"contains", "starts_with", "ends_with"}:
                clauses.append(f"{condition.operator}({column}, ?)")
                values.append(str(condition.value))
            else:
                clauses.append(f"{column} {condition.operator} ?")
                values.append(condition.value)
        for sort in request.sorts:
            if sort.column not in column_names:
                raise HTTPException(422, "Invalid sort column")
        where = " WHERE " + " AND ".join(clauses) if clauses else ""
        try:
            total_rows = con.execute(f"SELECT count(*) FROM {table}{where}", values).fetchone()[0]
            order = ""
            if request.sorts:
                order = " ORDER BY " + ", ".join(f"{quote(sort.column)} {sort.direction.upper()}" for sort in request.sorts)
            result = con.execute(
                f"SELECT * FROM {table}{where}{order} LIMIT ? OFFSET ?",
                values + [request.page_size, (request.page - 1) * request.page_size],
            )
            result_names = [item[0] for item in result.description]
            rows = [{name: safe(value) for name, value in zip(result_names, row)} for row in result.fetchall()]
        except duckdb.Error as exc:
            raise HTTPException(422, f"Invalid filter value: {exc}") from exc
        null_select = ", ".join(f"avg(CASE WHEN {quote(name)} IS NULL THEN 1.0 ELSE 0.0 END)" for name, _ in columns)
        fractions = con.execute(f"SELECT {null_select} FROM {table}").fetchone() if columns else []
        response_columns = [{
            "name": name,
            "type": type_,
            "numeric": type_.upper().startswith(NUMERIC),
            "null_fraction": safe(fraction) or 0.0,
        } for (name, type_), fraction in zip(columns, fractions)]
        return {
            "columns": response_columns,
            "rows": rows,
            "page": request.page,
            "page_size": request.page_size,
            "total_rows": total_rows,
            "total_pages": math.ceil(total_rows / request.page_size),
            "elapsed_ms": round((time.perf_counter() - started) * 1000, 3),
        }

    @api.get("/api/nodes/{node_id}/datasets/{dataset}/columns/{column}/values")
    def category_values(
        node_id: str,
        dataset: str,
        column: str,
        search: str = "",
        offset: int = QueryParam(0, ge=0),
        limit: int = QueryParam(200, ge=1, le=500),
    ):
        con = get_connection(node_id)
        table, metadata_columns = metadata(con, dataset)
        columns = dict(metadata_columns)
        if column not in columns:
            raise HTTPException(404, "Column not found")
        if not columns[column].upper().startswith(TEXT):
            raise HTTPException(422, "Column is not text")
        field = quote(column)
        where = f"{field} IS NOT NULL AND contains(lower({field}), lower(?))"
        total = con.execute(f"SELECT count(DISTINCT {field}) FROM {table} WHERE {where}", [search]).fetchone()[0]
        rows = con.execute(f"""
            SELECT {field}, count(*) AS count
            FROM {table} WHERE {where}
            GROUP BY {field} ORDER BY count DESC, {field}
            LIMIT ? OFFSET ?
        """, [search, limit, offset]).fetchall()
        return {
            "values": [{"value": safe(value), "count": count} for value, count in rows],
            "total": total,
            "offset": offset,
            "limit": limit,
            "has_more": offset + len(rows) < total,
        }

    @api.get("/api/nodes/{node_id}/datasets/{dataset}/columns/{column}/stats")
    def stats(node_id: str, dataset: str, column: str):
        con = get_connection(node_id)
        table, metadata_columns = metadata(con, dataset)
        columns = dict(metadata_columns)
        if column not in columns:
            raise HTTPException(404, "Column not found")
        type_ = columns[column]
        if not type_.upper().startswith(NUMERIC):
            raise HTTPException(422, "Column is not numeric")
        field = quote(column)
        row = con.execute(f"""
            SELECT count(*), count({field}), count(*) - count({field}),
                   count(*) FILTER (WHERE isfinite({field}::DOUBLE)),
                   min({field}) FILTER (WHERE isfinite({field}::DOUBLE)),
                   max({field}) FILTER (WHERE isfinite({field}::DOUBLE)),
                   avg({field}) FILTER (WHERE isfinite({field}::DOUBLE)),
                   stddev_samp({field}) FILTER (WHERE isfinite({field}::DOUBLE)),
                   quantile_cont({field}, 0.25) FILTER (WHERE isfinite({field}::DOUBLE)),
                   quantile_cont({field}, 0.5) FILTER (WHERE isfinite({field}::DOUBLE)),
                   quantile_cont({field}, 0.75) FILTER (WHERE isfinite({field}::DOUBLE))
            FROM {table}
        """).fetchone()
        row_count, non_null, null_count, finite_count, minimum, maximum, mean, stddev, p25, median, p75 = row
        histogram = []
        finite_min = float(minimum) if isinstance(minimum, Decimal) else minimum
        finite_max = float(maximum) if isinstance(maximum, Decimal) else maximum
        if finite_count and finite_min is not None and finite_max is not None:
            bin_count = min(20, max(1, math.ceil(math.sqrt(finite_count))))
            unsafe_integer = type_.upper().startswith(INTEGER) and (
                abs(int(minimum)) > 2**53 - 1 or abs(int(maximum)) > 2**53 - 1
            )
            if unsafe_integer:
                bins = con.execute(f"""
                    WITH ranked AS (
                        SELECT {field}, ntile(?) OVER (ORDER BY {field}) AS bin
                        FROM {table} WHERE {field} IS NOT NULL
                    )
                    SELECT min({field}), max({field}), count(*)
                    FROM ranked GROUP BY bin ORDER BY bin
                """, [bin_count]).fetchall()
                histogram = [{"lower": safe(lower), "upper": safe(upper), "count": count} for lower, upper, count in bins]
            elif finite_min == finite_max:
                histogram = [{"lower": finite_min, "upper": finite_max, "count": finite_count}]
            else:
                width = (finite_max - finite_min) / bin_count
                counts = dict(con.execute(f"""
                    SELECT least(?, floor(({field} - ?) / ?)::INTEGER), count(*)
                    FROM {table} WHERE {field} IS NOT NULL AND isfinite({field}::DOUBLE)
                    GROUP BY 1 ORDER BY 1
                """, [bin_count - 1, finite_min, width]).fetchall())
                histogram = [{
                    "lower": safe(finite_min + index * width),
                    "upper": safe(finite_max if index == bin_count - 1 else finite_min + (index + 1) * width),
                    "count": counts.get(index, 0),
                } for index in range(bin_count)]
        return {
            "type": type_, "row_count": row_count, "non_null_count": non_null,
            "null_count": null_count, "null_fraction": null_count / row_count if row_count else 0.0,
            "min": safe(minimum), "max": safe(maximum), "mean": safe(mean), "stddev": safe(stddev),
            "p25": safe(p25), "median": safe(median), "p75": safe(p75), "histogram": histogram,
        }

    frontend = Path(__file__).parent.parent / "frontend" / "dist"
    if frontend.is_dir():
        api.mount("/", StaticFiles(directory=frontend, html=True), name="frontend")
    return api


app = create_app()
