import base64
import csv
import datetime as dt
import json
import math
import os
import re
import tempfile
import time
import uuid
import zipfile
from contextlib import asynccontextmanager
from decimal import Decimal
from functools import wraps
from pathlib import Path
from threading import Lock
from typing import Any, Literal
from xml.etree import ElementTree

import duckdb
from fastapi import FastAPI, File, HTTPException, Query as QueryParam, UploadFile
from fastapi.responses import FileResponse, JSONResponse, Response
from fastapi.staticfiles import StaticFiles
from openpyxl import Workbook
from openpyxl.cell import WriteOnlyCell
from pydantic import BaseModel, ConfigDict, Field
from starlette.background import BackgroundTask

SUPPORTED = {".csv", ".tsv", ".parquet", ".json", ".ndjson", ".jsonl", ".xlsx", ".duckdb", ".db"}
NUMERIC = ("TINYINT", "SMALLINT", "INTEGER", "BIGINT", "HUGEINT", "UTINYINT", "USMALLINT", "UINTEGER", "UBIGINT", "UHUGEINT", "FLOAT", "REAL", "DOUBLE", "DECIMAL")
INTEGER = ("TINYINT", "SMALLINT", "INTEGER", "BIGINT", "HUGEINT", "UTINYINT", "USMALLINT", "UINTEGER", "UBIGINT", "UHUGEINT")
TEXT = ("VARCHAR", "CHAR", "TEXT")
DATE = ("DATE", "TIME", "TIMESTAMP")
OPERATORS = {"=", "!=", "in", "is_null", "not_null", "contains", "starts_with", "ends_with", ">", ">=", "<", "<="}
ILLEGAL_XML = re.compile(r"[\x00-\x08\x0b-\x0c\x0e-\x1f\ud800-\udfff\ufffe\uffff]")
INVALID_SHEET_NAME = re.compile(r"[\\/*?:\[\]]")
CSV_FORMULA_PREFIXES = ("=", "+", "-", "@", "\t", "\r")
EXCEL_TEXT_LIMIT = 32767


class AttachRequest(BaseModel):
    model_config = ConfigDict(extra="forbid")
    path: str


class ProjectCreate(BaseModel):
    model_config = ConfigDict(extra="forbid")
    name: str = Field(min_length=1, max_length=200)


class WorkbookConfirm(BaseModel):
    model_config = ConfigDict(extra="forbid")
    sheets: list[str]


class Filter(BaseModel):
    model_config = ConfigDict(extra="forbid")
    column: str
    operator: str
    value: Any = None
    connector: Literal["and", "or"] = "and"


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
    dedupe_columns: list[str] = []


class SQLRequest(BaseModel):
    model_config = ConfigDict(extra="forbid")
    sql: str


class SQLQuery(SQLRequest, Query):
    pass


class ExportSheet(BaseModel):
    model_config = ConfigDict(extra="forbid")
    node_id: str
    name: str
    sql: str


class ExportRequest(BaseModel):
    model_config = ConfigDict(extra="forbid")
    format: Literal["csv", "xlsx"]
    filename: str | None = None
    sheets: list[ExportSheet] = Field(min_length=1, max_length=100)


class JoinReference(BaseModel):
    model_config = ConfigDict(extra="forbid")
    node_id: str
    dataset: str | None = None
    sql: str | None = None
    name: str | None = None


class JoinWorkspaceRequest(BaseModel):
    model_config = ConfigDict(extra="forbid")
    left: JoinReference
    right: JoinReference
    left_keys: list[str]
    right_keys: list[str]


def quote(value: str) -> str:
    return '"' + value.replace('"', '""') + '"'


def dataset_name(filename: str) -> str:
    name = re.sub(r"[^a-z0-9]+", "_", Path(filename).stem.lower()).strip("_")
    return f"data_{name}" if name[:1].isdigit() else name or "data"


def literal(value: Any) -> str:
    if value is None:
        return "NULL"
    if isinstance(value, bool):
        return str(value).upper()
    if isinstance(value, (int, Decimal)):
        return str(value)
    if isinstance(value, float):
        if math.isnan(value):
            return "'NaN'::DOUBLE"
        if math.isinf(value):
            return f"'{'-' if value < 0 else ''}Infinity'::DOUBLE"
        return repr(value)
    if isinstance(value, dt.datetime):
        return "TIMESTAMP '" + value.isoformat(sep=" ").replace("'", "''") + "'"
    if isinstance(value, dt.date):
        return f"DATE '{value.isoformat()}'"
    if isinstance(value, dt.time):
        return f"TIME '{value.isoformat()}'"
    return "'" + str(value).replace("'", "''") + "'"


def workbook_sheets(path: Path) -> list[str]:
    with zipfile.ZipFile(path) as workbook:
        root = ElementTree.fromstring(workbook.read("xl/workbook.xml"))
    sheets = [sheet.attrib["name"] for sheet in root.findall("{*}sheets/{*}sheet")]
    if not sheets:
        raise ValueError("Workbook has no sheets")
    return sheets


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


def export_nested(value: Any) -> Any:
    if isinstance(value, (bytes, bytearray, memoryview)):
        return bytes(value).hex()
    if isinstance(value, Decimal):
        return str(value)
    if isinstance(value, int) and not -(10**15 - 1) <= value <= 10**15 - 1:
        return str(value)
    if isinstance(value, float) and math.isnan(value):
        return "NaN"
    if isinstance(value, float) and math.isinf(value):
        return "-Infinity" if value < 0 else "Infinity"
    if isinstance(value, (dt.date, dt.time, dt.datetime)):
        return value.isoformat()
    if isinstance(value, dt.timedelta):
        return str(value)
    if isinstance(value, (list, tuple)):
        return [export_nested(item) for item in value]
    if isinstance(value, dict):
        return {str(key): export_nested(item) for key, item in value.items()}
    if not isinstance(value, (str, int, float, bool)) and value is not None:
        return str(value)
    return value


def export_value(value: Any) -> Any:
    if isinstance(value, (bytes, bytearray, memoryview)):
        value = bytes(value).hex()
    elif isinstance(value, (list, tuple, dict)):
        value = json.dumps(export_nested(value), ensure_ascii=False)
    elif isinstance(value, int) and not -(10**15 - 1) <= value <= 10**15 - 1:
        value = str(value)
    elif isinstance(value, Decimal) and (not value.is_finite() or len(value.as_tuple().digits) > 15):
        value = str(value)
    elif isinstance(value, float) and math.isnan(value):
        value = "NaN"
    elif isinstance(value, float) and math.isinf(value):
        value = "-Infinity" if value < 0 else "Infinity"
    elif isinstance(value, (dt.datetime, dt.time)) and value.tzinfo is not None:
        value = value.isoformat()
    elif not isinstance(value, (str, int, float, bool, Decimal, dt.date, dt.time, dt.timedelta)) and value is not None:
        value = str(value)
    return ILLEGAL_XML.sub("", value) if isinstance(value, str) else value


def csv_value(value: Any) -> Any:
    value = export_value(value)
    return "'" + value if isinstance(value, str) and value.startswith(CSV_FORMULA_PREFIXES) else value


def xlsx_row(worksheet: Any, values: Any) -> list[Any]:
    row = []
    for value in values:
        value = export_value(value)
        if isinstance(value, str):
            if len(value) > EXCEL_TEXT_LIMIT:
                raise HTTPException(422, f"Excel cell text exceeds {EXCEL_TEXT_LIMIT} characters")
            cell = WriteOnlyCell(worksheet, value=value)
            cell.data_type = "s"
            value = cell
        row.append(value)
    return row


def export_sheet_name(name: str, used: set[str]) -> str:
    base = INVALID_SHEET_NAME.sub("_", ILLEGAL_XML.sub("", name)).strip().strip("'") or "Sheet"
    base = base[:31].rstrip("'") or "Sheet"
    if base.casefold() == "history":
        base += "_"
    candidate = base
    number = 2
    while candidate.casefold() in used:
        suffix = f" ({number})"
        candidate = (base[:31 - len(suffix)].rstrip("'") or "Sheet") + suffix
        number += 1
    used.add(candidate.casefold())
    return candidate


def export_filename(name: str | None, extension: str) -> str:
    basename = Path((name or "export").replace("\\", "/")).name
    stem = Path(basename).stem
    stem = re.sub(r"[\x00-\x1f\x7f]", "", stem).strip(" .") or "export"
    return stem + extension


def profile_kind(type_: str) -> str | None:
    type_upper = type_.upper()
    if type_upper.startswith(NUMERIC):
        return "numeric"
    if type_upper.startswith(TEXT) or type_upper.startswith("ENUM") or type_upper == "BOOLEAN":
        return "categorical"
    if type_upper.startswith(DATE):
        return "date"
    return None


def page_count(rows: int, page_size: int) -> int:
    return (rows + page_size - 1) // page_size


def create_app(data_dir: str | Path | None = None) -> FastAPI:
    root = Path(data_dir or os.getenv("QUARK_DATA_DIR", "data")).expanduser().resolve()
    registry_path = root / "registry.json"
    projects_path = root / "projects.json"
    uploads = root / "uploads"
    projects: list[dict[str, str]] = [{"id": "default", "name": "Default", "node_id": "project_default"}]
    nodes: dict[str, dict[str, Any]] = {}
    registered_nodes: list[dict[str, Any]] = []
    pending_workbooks: dict[str, dict[str, Any]] = {}
    connections: dict[str, duckdb.DuckDBPyConnection] = {}
    project_workspaces: dict[str, dict[str, Any]] = {}
    retired_project_connections: list[duckdb.DuckDBPyConnection] = []
    project_workspaces_lock = Lock()
    # ponytail: global serialization is fine for local single-user Quark; use per-project locks only if concurrent analytics throughput matters.
    database_operations_lock = Lock()
    # ponytail: one process-global slot is intentional; Quark is local and single-user.
    join_workspace: dict[str, Any] = {}

    def serialized(endpoint):
        @wraps(endpoint)
        def wrapper(*args, **kwargs):
            with database_operations_lock:
                return endpoint(*args, **kwargs)

        return wrapper

    def public(node: dict[str, Any], project_scoped: bool = False) -> dict[str, str]:
        keys = ("id", "name", "kind") if project_scoped else ("id", "name", "kind", "source")
        result = {key: node[key] for key in keys}
        if project_scoped:
            result["project_id"] = node.get("project_id", "default")
        return result

    def project_for(project_id: str) -> dict[str, str]:
        project = next((item for item in projects if item["id"] == project_id), None)
        if project is None:
            raise HTTPException(404, "Project not found")
        return project

    def project_sources(project_id: str) -> list[dict[str, Any]]:
        project_for(project_id)
        return [node for node in nodes.values() if node.get("project_id", "default") == project_id]

    def source_for(project_id: str, source_id: str) -> dict[str, Any]:
        project_for(project_id)
        source = nodes.get(source_id)
        if source is None or source.get("project_id", "default") != project_id:
            raise HTTPException(404, "Source not found")
        return source

    def public_project(project: dict[str, str]) -> dict[str, Any]:
        return {**project, "source_count": len(project_sources(project["id"]))}

    def save_projects() -> None:
        root.mkdir(parents=True, exist_ok=True)
        temporary = projects_path.with_suffix(".tmp")
        temporary.write_text(json.dumps(projects[1:], indent=2))
        temporary.replace(projects_path)

    def invalidate_project(project_id: str) -> None:
        with project_workspaces_lock:
            workspace = project_workspaces.pop(project_id, None)
            if workspace is not None:
                # ponytail: retired workspaces are acceptable for local low-frequency source mutations; add refcounts only if churn matters.
                retired_project_connections.append(workspace["connection"])

    def save_registry() -> None:
        root.mkdir(parents=True, exist_ok=True)
        temporary = registry_path.with_suffix(".tmp")
        temporary.write_text(json.dumps(registered_nodes, indent=2))
        temporary.replace(registry_path)

    def _xlsx_typed_view(
        con: duckdb.DuckDBPyConnection, source_sql: str, sheet_sql: str, name: str, schema: str | None = None,
    ) -> None:
        """Keep Excel's styled date cells typed while safely coercing consistent text columns."""
        typed_expr = f"read_xlsx('{source_sql}', sheet = '{sheet_sql}')"
        raw_expr = f"read_xlsx('{source_sql}', sheet = '{sheet_sql}', all_varchar = true)"
        declared = {item[0]: str(item[1]).upper() for item in con.execute(f"SELECT * FROM {typed_expr} LIMIT 0").description}
        cols = [item[0] for item in con.execute(f"SELECT * FROM {raw_expr} LIMIT 0").description]
        selects = []
        for c in cols:
            cq = quote(c)
            numeric_ratio, date_ratio, timestamp_ratio, has_time, has_fraction = con.execute(f"""
                SELECT
                    COUNT(*) FILTER (WHERE TRY_CAST({cq} AS DOUBLE) IS NOT NULL) * 1.0 / NULLIF(COUNT(*) FILTER (WHERE {cq} IS NOT NULL AND TRIM({cq}) != ''), 0),
                    COUNT(*) FILTER (WHERE TRY_CAST({cq} AS DATE) IS NOT NULL) * 1.0 / NULLIF(COUNT(*) FILTER (WHERE {cq} IS NOT NULL AND TRIM({cq}) != ''), 0),
                    COUNT(*) FILTER (WHERE TRY_CAST({cq} AS TIMESTAMP) IS NOT NULL) * 1.0 / NULLIF(COUNT(*) FILTER (WHERE {cq} IS NOT NULL AND TRIM({cq}) != ''), 0),
                    COUNT(*) FILTER (WHERE regexp_matches(TRIM({cq}), '[T ]\\d{{1,2}}:')),
                    COUNT(*) FILTER (WHERE TRY_CAST({cq} AS DOUBLE) IS NOT NULL AND TRY_CAST({cq} AS DOUBLE) != floor(TRY_CAST({cq} AS DOUBLE)))
                FROM {raw_expr}
            """).fetchone()
            # ponytail: Excel stores styled dates as serials; its schema is the only signal allowed to reinterpret them.
            if declared.get(c) == "DATE" and has_fraction:
                selects.append(f"TIMESTAMP '1899-12-30' + TRY_CAST({cq} AS DOUBLE) * INTERVAL 1 DAY AS {cq}")
            elif declared.get(c) == "DATE":
                selects.append(f"CAST(TIMESTAMP '1899-12-30' + TRY_CAST({cq} AS DOUBLE) * INTERVAL 1 DAY AS DATE) AS {cq}")
            elif declared.get(c) == "TIMESTAMP":
                selects.append(f"TIMESTAMP '1899-12-30' + TRY_CAST({cq} AS DOUBLE) * INTERVAL 1 DAY AS {cq}")
            elif declared.get(c) == "TIME":
                selects.append(f"CAST(TIMESTAMP '1899-12-30' + TRY_CAST({cq} AS DOUBLE) * INTERVAL 1 DAY AS TIME) AS {cq}")
            elif numeric_ratio is not None and numeric_ratio >= 0.9:
                selects.append(f"TRY_CAST({cq} AS DOUBLE) AS {cq}")
            elif timestamp_ratio is not None and timestamp_ratio >= 0.9 and has_time:
                selects.append(f"TRY_CAST({cq} AS TIMESTAMP) AS {cq}")
            elif date_ratio is not None and date_ratio >= 0.9:
                selects.append(f"TRY_CAST({cq} AS DATE) AS {cq}")
            else:
                selects.append(cq)
        target = f"{quote(schema)}.{quote(name)}" if schema else quote(name)
        con.execute(f"CREATE VIEW {target} AS SELECT {', '.join(selects)} FROM {raw_expr}")

    def scan_expression(source: Path) -> str:
        source_sql = str(source).replace("'", "''")
        suffix = source.suffix.lower()
        if suffix in {".csv", ".tsv"}:
            delimiter = "\\t" if suffix == ".tsv" else ","
            return f"read_csv_auto('{source_sql}', delim='{delimiter}')"
        if suffix == ".parquet":
            return f"read_parquet('{source_sql}')"
        return f"read_json_auto('{source_sql}')"

    def connect(node: dict[str, Any]) -> duckdb.DuckDBPyConnection:
        source = Path(node["source"])
        suffix = source.suffix.lower()
        if suffix in {".duckdb", ".db"}:
            con = duckdb.connect(str(source), read_only=True)
            con.execute("SET enable_external_access = ?", [False])
            return con
        con = duckdb.connect()
        source_sql = str(source).replace("'", "''")
        if suffix == ".xlsx":
            con.execute("INSTALL excel; LOAD excel")
            for sheet in node["sheets"] if "sheets" in node else workbook_sheets(source):
                sheet_sql = sheet.replace("'", "''")
                _xlsx_typed_view(con, source_sql, sheet_sql, sheet)
        else:
            con.execute(
                f"CREATE VIEW {quote(node.get('dataset_name', 'data'))} AS SELECT * FROM {scan_expression(source)}"
            )
        con.execute("SET allowed_directories = ?", [[str(source.parent)]])
        con.execute("SET enable_external_access = ?", [False])
        return con

    def add(node: dict[str, Any]) -> dict[str, str]:
        try:
            con = connect(node)
            datasets_for(con)
        except Exception as exc:
            raise HTTPException(400, f"Could not open source: {exc}") from exc
        nodes[node["id"]] = node
        registered_nodes.append(node)
        connections[node["id"]] = con
        save_registry()
        invalidate_project(node.get("project_id", "default"))
        return public(node, "project_id" in node)

    def get_connection(node_id: str) -> duckdb.DuckDBPyConnection:
        if node_id == join_workspace.get("id"):
            return join_workspace["connection"]
        project = next((item for item in projects if item["node_id"] == node_id), None)
        if project is not None:
            return workspace_for(project["id"])["connection"]
        if node_id not in nodes or nodes[node_id].get("project_id", "default") != "default":
            raise HTTPException(404, "Node not found")
        return connections[node_id]

    def datasets_for(con: duckdb.DuckDBPyConnection) -> list[dict[str, Any]]:
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

    def dataset_for(con: duckdb.DuckDBPyConnection, dataset: str) -> dict[str, Any]:
        item = next((item for item in datasets_for(con) if item["id"] == dataset), None)
        if item is None:
            raise HTTPException(404, "Dataset not found")
        return item

    def metadata(con: duckdb.DuckDBPyConnection, dataset: str) -> tuple[str, list[tuple[str, str]]]:
        item = dataset_for(con, dataset)
        table = f'{quote(item["schema"])}.{quote(item["name"])}'
        columns = [(row[0], row[1]) for row in con.execute(f"DESCRIBE SELECT * FROM {table}").fetchall()]
        return table, columns

    def mount_dataset(
        con: duckdb.DuckDBPyConnection, schema: str, node: dict[str, Any], item: dict[str, Any],
    ) -> str:
        source = Path(node["source"])
        source_sql = str(source).replace("'", "''")
        target = f"{quote(schema)}.{quote(item['name'])}"
        con.execute(f"CREATE SCHEMA {quote(schema)}")
        if source.suffix.lower() in {".duckdb", ".db"}:
            alias = f"{schema}_database"
            con.execute(f"ATTACH '{source_sql}' AS {quote(alias)} (READ_ONLY)")
            con.execute(
                f"CREATE VIEW {target} AS SELECT * FROM {quote(alias)}.{quote(item['schema'])}.{quote(item['name'])}"
            )
        elif source.suffix.lower() == ".xlsx":
            con.execute("INSTALL excel; LOAD excel")
            _xlsx_typed_view(con, source_sql, item["name"].replace("'", "''"), item["name"], schema)
        else:
            con.execute(f"CREATE VIEW {target} AS SELECT * FROM {scan_expression(source)}")
        return target

    def workspace_for(project_id: str) -> dict[str, Any]:
        project_for(project_id)
        with project_workspaces_lock:
            if project_id in project_workspaces:
                return project_workspaces[project_id]
            sources = project_sources(project_id)
            con = duckdb.connect()
            try:
                if any(Path(node["source"]).suffix.lower() == ".xlsx" for node in sources):
                    con.execute("INSTALL excel; LOAD excel")
                paths = [str(Path(node["source"])) for node in sources]
                paths.extend(f"{node['source']}.wal" for node in sources if Path(node["source"]).suffix.lower() in {".duckdb", ".db"})
                if paths:
                    con.execute("SET allowed_paths = ?", [sorted(paths)])
                con.execute("SET enable_external_access = ?", [False])
            except Exception:
                con.close()
                raise
            workspace = {"connection": con, "views": [], "mounted_sources": set()}
            project_workspaces[project_id] = workspace
            return workspace

    def mount_project_source(project_id: str, source_id: str) -> list[dict[str, Any]]:
        node = source_for(project_id, source_id)
        project = project_for(project_id)
        workspace = workspace_for(project_id)
        if source_id in workspace["mounted_sources"]:
            return [view for view in workspace["views"] if view["source_id"] == source_id]
        con = workspace["connection"]
        source_con = connections[source_id]
        source = Path(node["source"])
        source_sql = str(source).replace("'", "''")
        source_views: list[dict[str, Any]] = []
        try:
            attached_alias = None
            if source.suffix.lower() in {".duckdb", ".db"}:
                node_encoded = base64.urlsafe_b64encode(source_id.encode()).rstrip(b"=").decode()
                attached_alias = f"database_{node_encoded}"
                con.execute(f"ATTACH '{source_sql}' AS {quote(attached_alias)} (READ_ONLY)")
            for item in datasets_for(source_con):
                encoded = base64.urlsafe_b64encode(
                    json.dumps([source_id, item["id"]], separators=(",", ":")).encode()
                ).rstrip(b"=").decode()
                schema = f"source_{encoded}"
                target = f"{quote(schema)}.{quote(item['name'])}"
                if attached_alias is not None:
                    con.execute(f"CREATE SCHEMA {quote(schema)}")
                    con.execute(
                        f"CREATE VIEW {target} AS SELECT * FROM "
                        f"{quote(attached_alias)}.{quote(item['schema'])}.{quote(item['name'])}"
                    )
                elif source.suffix.lower() == ".xlsx":
                    con.execute(f"CREATE SCHEMA {quote(schema)}")
                    _xlsx_typed_view(con, source_sql, item["name"].replace("'", "''"), item["name"], schema)
                else:
                    target = mount_dataset(con, schema, node, item)
                columns = [row[0] for row in source_con.execute(
                    f"DESCRIBE SELECT * FROM {quote(item['schema'])}.{quote(item['name'])}"
                ).fetchall()]
                view_id = base64.urlsafe_b64encode(
                    json.dumps([project_id, source_id, item["id"]], separators=(",", ":")).encode()
                ).rstrip(b"=").decode()
                source_views.append({
                    "id": view_id, "project_id": project_id,
                    "source_id": source_id, "source_name": node["name"],
                    "node_id": project["node_id"], "name": item["name"],
                    "schema": item["schema"], "type": item["type"], "columns": columns,
                    "sql": f"SELECT * FROM {target}",
                })
        except Exception:
            with project_workspaces_lock:
                if project_workspaces.get(project_id) is workspace:
                    project_workspaces.pop(project_id)
            con.close()
            raise
        workspace["views"].extend(source_views)
        workspace["mounted_sources"].add(source_id)
        return source_views

    def filtered_relation(
        table: str, columns: list[tuple[str, str]], request: Query, display_table: str | None = None,
    ) -> tuple[str, list[Any], str]:
        display_table = display_table or table
        column_types = dict(columns)
        clauses: list[str] = []
        display_clauses: list[str] = []
        values: list[Any] = []
        for condition in request.filters:
            if condition.column not in column_types or condition.operator not in OPERATORS:
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
                display_clauses.append(clauses[-1])
            elif condition.operator == "in":
                clauses.append(f"{column} IN ({', '.join('?' for _ in condition.value)})")
                display_clauses.append(f"{column} IN ({', '.join(literal(value) for value in condition.value)})")
                values.extend(condition.value)
            elif condition.operator in {"contains", "starts_with", "ends_with"}:
                clauses.append(f"{condition.operator}({column}, ?)")
                display_clauses.append(f"{condition.operator}({column}, {literal(str(condition.value))})")
                values.append(str(condition.value))
            else:
                clauses.append(f"{column} {condition.operator} ?")
                display_clauses.append(f"{column} {condition.operator} {literal(condition.value)}")
                values.append(condition.value)
        if len(set(request.dedupe_columns)) != len(request.dedupe_columns) or any(column not in column_types for column in request.dedupe_columns):
            raise HTTPException(422, "Invalid dedupe column")
        where = clauses[0] if clauses else ""
        display_where = display_clauses[0] if display_clauses else ""
        for condition, clause, display_clause in zip(request.filters[1:], clauses[1:], display_clauses[1:]):
            connector = condition.connector.upper()
            where = f"({where} {connector} {clause})"
            display_where = f"({display_where} {connector} {display_clause})"
        where = f" WHERE {where}" if where else ""
        display_where = f" WHERE {display_where}" if display_where else ""
        if request.dedupe_columns:
            keys = ", ".join(quote(column) for column in request.dedupe_columns)
            qualify = f" QUALIFY row_number() OVER (PARTITION BY {keys}) = 1"
            return f"(SELECT * FROM {table}{where}{qualify})", values, f"(SELECT * FROM {display_table}{display_where}{qualify})"
        return f"(SELECT * FROM {table}{where})", values, f"(SELECT * FROM {display_table}{display_where})"

    def controlled_query(
        table: str, columns: list[tuple[str, str]], request: Query, display_table: str | None = None,
    ) -> tuple[str, list[Any], str]:
        source, values, display_source = filtered_relation(table, columns, request, display_table)
        column_names = {name for name, _ in columns}
        if any(sort.column not in column_names for sort in request.sorts):
            raise HTTPException(422, "Invalid sort column")
        order = " ORDER BY " + ", ".join(
            f"{quote(sort.column)} {sort.direction.upper()}" for sort in request.sorts
        ) if request.sorts else ""
        return f"SELECT * FROM {source}{order}", values, f"SELECT * FROM {display_source}{order}"

    def read_only_sql(con: duckdb.DuckDBPyConnection, sql: str) -> str:
        try:
            statements = con.extract_statements(sql)
        except duckdb.Error as exc:
            raise HTTPException(422, f"Invalid SQL query: {exc}") from exc
        if len(statements) != 1 or statements[0].type != duckdb.StatementType.SELECT:
            raise HTTPException(422, "SQL accepts only one read-only SELECT query")
        return sql.strip()

    def sql_metadata(con: duckdb.DuckDBPyConnection, request: SQLRequest) -> tuple[str, list[tuple[str, str]]]:
        sql = read_only_sql(con, request.sql)
        try:
            columns = [(item[0], str(item[1])) for item in con.execute("DESCRIBE SELECT * FROM query(?)", [sql]).fetchall()]
        except duckdb.ParserException as exc:
            raise HTTPException(422, "SQL accepts only one read-only SELECT query") from exc
        except duckdb.Error as exc:
            raise HTTPException(422, f"Invalid SQL query: {exc}") from exc
        return sql, columns

    def query_response(
        con: duckdb.DuckDBPyConnection,
        sql: str,
        values: list[Any],
        page: int,
        page_size: int,
        started: float,
        response_sql: str,
    ) -> dict[str, Any]:
        source = f"({sql}) AS result"
        total_rows = con.execute(f"SELECT count(*) FROM {source}", values).fetchone()[0]
        result = con.execute(
            f"SELECT * FROM {source} LIMIT ? OFFSET ?",
            values + [page_size, (page - 1) * page_size],
        )
        columns = [(item[0], str(item[1])) for item in result.description]
        rows = [{name: safe(value) for (name, _), value in zip(columns, row)} for row in result.fetchall()]
        null_select = ", ".join(
            f"avg(CASE WHEN {quote(name)} IS NULL THEN 1.0 ELSE 0.0 END)" for name, _ in columns
        )
        fractions = con.execute(f"SELECT {null_select} FROM {source}", values).fetchone() if columns else []
        return {
            "columns": [{
                "name": name,
                "type": type_,
                "numeric": type_.upper().startswith(NUMERIC),
                "profile_kind": profile_kind(type_),
                "null_fraction": safe(fraction) or 0.0,
            } for (name, type_), fraction in zip(columns, fractions)],
            "rows": rows,
            "page": page,
            "page_size": page_size,
            "total_rows": safe(total_rows),
            "total_pages": safe(page_count(total_rows, page_size)),
            "elapsed_ms": round((time.perf_counter() - started) * 1000, 3),
            "sql": response_sql,
        }

    @asynccontextmanager
    async def lifespan(_: FastAPI):
        root.mkdir(parents=True, exist_ok=True)
        uploads.mkdir(exist_ok=True)
        if projects_path.exists():
            try:
                loaded_projects = json.loads(projects_path.read_text())
                for project in loaded_projects if isinstance(loaded_projects, list) else []:
                    if (
                        isinstance(project, dict)
                        and all(isinstance(project.get(key), str) for key in ("id", "name", "node_id"))
                        and project["id"] != "default"
                        and all(existing["id"] != project["id"] for existing in projects)
                    ):
                        projects.append({key: project[key] for key in ("id", "name", "node_id")})
            except (json.JSONDecodeError, OSError):
                pass
        stored: list[Any] = []
        if registry_path.exists():
            try:
                loaded = json.loads(registry_path.read_text())
                stored = loaded if isinstance(loaded, list) else []
            except (json.JSONDecodeError, OSError):
                pass
            for node in stored:
                if not isinstance(node, dict):
                    continue
                registered_nodes.append(node)
                if not Path(node.get("source", "")).is_file():
                    continue
                try:
                    connections[node["id"]] = connect(node)
                    datasets_for(connections[node["id"]])
                    nodes[node["id"]] = node
                except Exception:
                    connections.pop(node.get("id", ""), None)
        # ponytail: startup must not rewrite the registry; a temporarily unavailable bind mount must not erase sources.
        persisted_sources = {Path(node.get("source", "")).resolve() for node in stored if isinstance(node, dict)}
        for candidate in uploads.iterdir():
            if candidate.is_file() and candidate.resolve() not in persisted_sources:
                candidate.unlink()
        yield
        if join_workspace:
            join_workspace["connection"].close()
        for workspace in project_workspaces.values():
            workspace["connection"].close()
        for con in retired_project_connections:
            con.close()
        for con in connections.values():
            con.close()

    api = FastAPI(title="Quark", lifespan=lifespan)

    @api.exception_handler(duckdb.Error)
    def duckdb_error(_: Any, exc: duckdb.Error):
        return JSONResponse(status_code=422, content={"detail": f"Invalid filter value: {exc}"})

    @api.get("/api/nodes")
    def list_nodes():
        return [public(node) for node in nodes.values() if node.get("project_id", "default") == "default"]

    @api.get("/api/projects")
    def list_projects():
        return [public_project(project) for project in projects]

    @api.post("/api/projects", status_code=201)
    def create_project(request: ProjectCreate):
        name = request.name.strip()
        if not name:
            raise HTTPException(422, "Project name is required")
        project_id = uuid.uuid4().hex
        project = {"id": project_id, "name": name, "node_id": f"project_{project_id}"}
        projects.append(project)
        save_projects()
        return public_project(project)

    @api.get("/api/projects/{project_id}/sources")
    def list_project_sources(project_id: str):
        return [{"id": node["id"], "name": node["name"]} for node in project_sources(project_id)]

    @api.get("/api/projects/{project_id}/sources/{source_id}")
    @serialized
    def get_project_source(project_id: str, source_id: str):
        source = source_for(project_id, source_id)
        return {**public(source, True), "views": mount_project_source(project_id, source_id)}

    @api.get("/api/projects/{project_id}/sources/{source_id}/path")
    def get_project_source_path(project_id: str, source_id: str):
        return {"path": str(Path(source_for(project_id, source_id)["source"]).expanduser().resolve())}

    @api.get("/api/projects/{project_id}/views")
    @serialized
    def list_project_views(project_id: str):
        return [
            view
            for source in project_sources(project_id)
            for view in mount_project_source(project_id, source["id"])
        ]

    @api.post("/api/exports")
    @serialized
    def export(request: ExportRequest):
        if request.format == "csv" and len(request.sheets) != 1:
            raise HTTPException(422, "CSV export requires exactly one sheet")
        extension = ".csv" if request.format == "csv" else ".xlsx"
        media_type = "text/csv" if request.format == "csv" else "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"
        sheets = []
        for sheet in request.sheets:
            con = get_connection(sheet.node_id)
            sheets.append((sheet, con, read_only_sql(con, sheet.sql)))
        with tempfile.NamedTemporaryFile(dir=root, prefix=".export-", suffix=extension, delete=False) as temporary:
            path = Path(temporary.name)
        try:
            if request.format == "csv":
                _, con, sql = sheets[0]
                with path.open("w", encoding="utf-8", newline="") as output:
                    writer = csv.writer(output)
                    result = con.execute(sql)
                    writer.writerow([csv_value(column[0]) for column in result.description])
                    while rows := result.fetchmany(1000):
                        writer.writerows([csv_value(value) for value in row] for row in rows)
            else:
                workbook = Workbook(write_only=True)
                try:
                    used_names: set[str] = set()
                    for sheet, con, sql in sheets:
                        worksheet = workbook.create_sheet(export_sheet_name(sheet.name, used_names))
                        result = con.execute(sql)
                        worksheet.append(xlsx_row(worksheet, (column[0] for column in result.description)))
                        while rows := result.fetchmany(1000):
                            for row in rows:
                                worksheet.append(xlsx_row(worksheet, row))
                    workbook.save(path)
                except Exception:
                    for worksheet in workbook.worksheets:
                        try:
                            worksheet.close()
                        except Exception:
                            pass
                        writer = getattr(worksheet, "_writer", None)
                        if writer is not None:
                            try:
                                writer.cleanup()
                            except (FileNotFoundError, ValueError):
                                pass
                    workbook.close()
                    raise
        except duckdb.Error as exc:
            path.unlink(missing_ok=True)
            raise HTTPException(422, f"Invalid SQL query: {exc}") from exc
        except Exception:
            path.unlink(missing_ok=True)
            raise
        return FileResponse(
            path,
            media_type=media_type,
            filename=export_filename(request.filename, extension),
            background=BackgroundTask(path.unlink, missing_ok=True),
        )

    @api.post("/api/join-workspaces")
    @serialized
    def create_join_workspace(request: JoinWorkspaceRequest):
        if len(request.left_keys) != len(request.right_keys):
            raise HTTPException(422, "Join key counts must match")
        if len(set(request.left_keys)) != len(request.left_keys) or len(set(request.right_keys)) != len(request.right_keys):
            raise HTTPException(422, "Join keys must be unique")
        for ref in (request.left, request.right):
            if (ref.dataset is None) == (ref.sql is None):
                raise HTTPException(422, "Join reference requires exactly one of dataset or sql")

        uses_sql = request.left.sql is not None or request.right.sql is not None
        new_con = None
        if uses_sql:
            left_con = get_connection(request.left.node_id)
            right_con = get_connection(request.right.node_id)
            if left_con is not right_con:
                raise HTTPException(422, "SQL joins require one project workspace")
            con = left_con
            node_id = request.left.node_id

            def resolve(ref: JoinReference) -> tuple[str, list[tuple[str, str]], dict[str, str]]:
                if ref.sql is not None:
                    sql, columns = sql_metadata(con, SQLRequest(sql=ref.sql))
                    return f"query({literal(sql)})", columns, {"name": ref.name or "View", "sql": sql}
                assert ref.dataset is not None
                item = dataset_for(con, ref.dataset)
                table, columns = metadata(con, ref.dataset)
                return table, columns, {"schema": item["schema"], "name": item["name"]}

            left_table, left_columns, left_identity = resolve(request.left)
            right_table, right_columns, right_identity = resolve(request.right)
        else:
            if request.left.node_id not in nodes or request.right.node_id not in nodes:
                raise HTTPException(404, "Node not found")
            assert request.left.dataset is not None and request.right.dataset is not None
            left_con = get_connection(request.left.node_id)
            right_con = get_connection(request.right.node_id)
            left_item = dataset_for(left_con, request.left.dataset)
            right_item = dataset_for(right_con, request.right.dataset)
            left_original, left_columns = metadata(left_con, request.left.dataset)
            right_original, right_columns = metadata(right_con, request.right.dataset)
            if request.left.node_id == request.right.node_id:
                con = left_con
                node_id = request.left.node_id
                left_table, right_table = left_original, right_original
                left_identity = {"schema": left_item["schema"], "name": left_item["name"]}
                right_identity = {"schema": right_item["schema"], "name": right_item["name"]}
            else:
                new_con = duckdb.connect()
                con = new_con
                try:
                    left_table = mount_dataset(con, "left_source", nodes[request.left.node_id], left_item)
                    right_table = mount_dataset(con, "right_source", nodes[request.right.node_id], right_item)
                    directories = sorted({str(Path(nodes[ref.node_id]["source"]).parent) for ref in (request.left, request.right)})
                    con.execute("SET allowed_directories = ?", [directories])
                    con.execute("SET enable_external_access = ?", [False])
                except Exception:
                    con.close()
                    raise
                node_id = f"join_{uuid.uuid4().hex}"
                left_identity = {"schema": "left_source", "name": left_item["name"]}
                right_identity = {"schema": "right_source", "name": right_item["name"]}

        if any(key not in dict(left_columns) for key in request.left_keys) or any(
            key not in dict(right_columns) for key in request.right_keys
        ):
            if new_con is not None:
                new_con.close()
            raise HTTPException(422, "Join key not found")

        try:
            left_rows = con.execute(f"SELECT count(*) FROM {left_table}").fetchone()[0]
            right_rows = con.execute(f"SELECT count(*) FROM {right_table}").fetchone()[0]
            if not request.left_keys:
                output_rows = left_rows * right_rows
                relationship = "cartesian"
            else:
                def unique(table: str, keys: list[str]) -> bool:
                    fields = ", ".join(quote(key) for key in keys)
                    distinct = f"({fields})" if len(keys) > 1 else fields
                    where = " AND ".join(f"{quote(key)} IS NOT NULL" for key in keys)
                    non_null, distinct_count = con.execute(
                        f"SELECT count(*), count(DISTINCT {distinct}) FROM {table} WHERE {where}"
                    ).fetchone()
                    return non_null == distinct_count

                left_unique = unique(left_table, request.left_keys)
                right_unique = unique(right_table, request.right_keys)
                if left_unique:
                    relationship = "one_to_one" if right_unique else "one_to_many"
                else:
                    relationship = "many_to_one" if right_unique else "many_to_many"
                on = " AND ".join(
                    f"l.{quote(left)} = r.{quote(right)}"
                    for left, right in zip(request.left_keys, request.right_keys)
                )
                output_rows = con.execute(
                    f"SELECT count(*) FROM {left_table} l INNER JOIN {right_table} r ON {on}"
                ).fetchone()[0]
        except Exception:
            if new_con is not None:
                new_con.close()
            raise

        if new_con is not None:
            if join_workspace:
                join_workspace["connection"].close()
            join_workspace.clear()
            join_workspace.update({"id": node_id, "connection": new_con})
        return safe({
            "node_id": node_id, "left": left_identity, "right": right_identity,
            "left_rows": left_rows, "right_rows": right_rows, "output_rows": output_rows,
            "relationship": relationship,
            "cartesian_risk": relationship in {"cartesian", "many_to_many"},
        })

    @api.post("/api/projects/{project_id}/sources/upload", status_code=201)
    @api.post("/api/nodes/upload", status_code=201)
    async def upload(file: UploadFile = File(...), project_id: str | None = None):
        if project_id is not None:
            project_for(project_id)
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
            if suffix == ".xlsx":
                sheets = workbook_sheets(destination)
                pending_workbooks[node_id] = {"id": node_id, "name": name, "source": str(destination), "sheets": sheets}
                if project_id is not None:
                    pending_workbooks[node_id]["project_id"] = project_id
                return {
                    "id": node_id, "name": name, "kind": "workbook", "sheets": sheets,
                    **({"project_id": project_id} if project_id is not None else {}),
                }
            node = {"id": node_id, "name": name, "kind": "upload", "source": str(destination)}
            if project_id is not None:
                node["project_id"] = project_id
            if suffix not in {".duckdb", ".db"}:
                node["dataset_name"] = dataset_name(name)
            return add(node)
        except Exception:
            destination.unlink(missing_ok=True)
            raise
        finally:
            await file.close()

    @api.post("/api/projects/{project_id}/sources/upload/{stage_id}/confirm")
    @api.post("/api/nodes/upload/{stage_id}/confirm")
    def confirm_workbook(stage_id: str, request: WorkbookConfirm, project_id: str | None = None):
        if project_id is not None:
            project_for(project_id)
        stage = pending_workbooks.get(stage_id)
        if stage is None or stage.get("project_id", "default") != (project_id or "default"):
            raise HTTPException(404, "Workbook upload not found")
        if not request.sheets or len(set(request.sheets)) != len(request.sheets) or not set(request.sheets).issubset(stage["sheets"]):
            raise HTTPException(422, "Select one or more unique workbook sheets")
        node = add({**stage, "kind": "upload", "sheets": request.sheets})
        pending_workbooks.pop(stage_id, None)
        return node

    @api.delete("/api/projects/{project_id}/sources/upload/{stage_id}", status_code=204)
    @api.delete("/api/nodes/upload/{stage_id}", status_code=204)
    def discard_workbook(stage_id: str, project_id: str | None = None):
        if project_id is not None:
            project_for(project_id)
        stage = pending_workbooks.get(stage_id)
        if stage is None or stage.get("project_id", "default") != (project_id or "default"):
            raise HTTPException(404, "Workbook upload not found")
        pending_workbooks.pop(stage_id)
        Path(stage["source"]).unlink(missing_ok=True)
        return Response(status_code=204)

    @api.post("/api/projects/{project_id}/sources/attach", status_code=201)
    @api.post("/api/nodes/attach", status_code=201)
    def attach(request: AttachRequest, project_id: str | None = None):
        if project_id is not None:
            project_for(project_id)
        source = Path(request.path).expanduser().resolve()
        if source.suffix.lower() not in {".duckdb", ".db"}:
            raise HTTPException(400, "Only .duckdb and .db paths can be attached")
        if not source.is_file():
            raise HTTPException(404, "Database not found")
        node = {"id": uuid.uuid4().hex, "name": source.name, "kind": "attached", "source": str(source)}
        if project_id is not None:
            node["project_id"] = project_id
        return add(node)

    @api.delete("/api/projects/{project_id}/sources/{node_id}", status_code=204)
    @api.delete("/api/nodes/{node_id}", status_code=204)
    @serialized
    def delete_node(node_id: str, project_id: str | None = None):
        if project_id is not None:
            project_for(project_id)
        if node_id not in nodes:
            raise HTTPException(404, "Node not found")
        if nodes[node_id].get("project_id", "default") != (project_id or "default"):
            raise HTTPException(404, "Node not found")
        node = nodes.pop(node_id)
        connections.pop(node_id).close()
        if node["kind"] == "upload":
            Path(node["source"]).unlink(missing_ok=True)
        registered_nodes[:] = [record for record in registered_nodes if record.get("id") != node_id]
        save_registry()
        invalidate_project(node.get("project_id", "default"))
        return Response(status_code=204)

    @api.get("/api/nodes/{node_id}/datasets")
    @serialized
    def list_datasets(node_id: str):
        con = get_connection(node_id)
        datasets = datasets_for(con)
        for dataset in datasets:
            table = f'{quote(dataset["schema"])}.{quote(dataset["name"])}'
            dataset["columns"] = [row[0] for row in con.execute(f"DESCRIBE SELECT * FROM {table}").fetchall()]
        return datasets

    @api.post("/api/nodes/{node_id}/sql")
    @serialized
    def sql_query(node_id: str, request: SQLQuery):
        started = time.perf_counter()
        con = get_connection(node_id)
        sql, columns = sql_metadata(con, request)
        query_sql, values, display_sql = controlled_query("query(?)", columns, request, f"({sql})")
        try:
            return query_response(
                con, query_sql, [sql, *values], request.page, request.page_size, started,
                display_sql if request.filters or request.sorts or request.dedupe_columns else sql,
            )
        except duckdb.Error as exc:
            raise HTTPException(422, f"Invalid SQL query: {exc}") from exc

    @api.post("/api/nodes/{node_id}/datasets/{dataset}/query")
    @serialized
    def query(node_id: str, dataset: str, request: Query):
        started = time.perf_counter()
        con = get_connection(node_id)
        table, columns = metadata(con, dataset)
        query_sql, values, display_sql = controlled_query(table, columns, request)
        try:
            return query_response(con, query_sql, values, request.page, request.page_size, started, display_sql)
        except duckdb.Error as exc:
            raise HTTPException(422, f"Invalid filter value: {exc}") from exc

    def category_response(
        con: duckdb.DuckDBPyConnection,
        table: str,
        metadata_columns: list[tuple[str, str]],
        column: str,
        params: list[Any] | None = None,
        search: str = "",
        offset: int = 0,
        limit: int = 200,
    ) -> dict[str, Any]:
        columns = dict(metadata_columns)
        if column not in columns:
            raise HTTPException(404, "Column not found")
        if not columns[column].upper().startswith(TEXT):
            raise HTTPException(422, "Column is not text")
        field = quote(column)
        where = f"{field} IS NOT NULL AND contains(lower({field}), lower(?))"
        params = params or []
        total = con.execute(f"SELECT count(DISTINCT {field}) FROM {table} WHERE {where}", [*params, search]).fetchone()[0]
        rows = con.execute(f"""
            SELECT {field}, count(*) AS count
            FROM {table} WHERE {where}
            GROUP BY {field} ORDER BY count DESC, {field}
            LIMIT ? OFFSET ?
        """, [*params, search, limit, offset]).fetchall()
        return {
            "values": [{"value": safe(value), "count": safe(count)} for value, count in rows],
            "total": safe(total),
            "offset": offset,
            "limit": limit,
            "has_more": offset + len(rows) < total,
        }

    @api.get("/api/nodes/{node_id}/datasets/{dataset}/columns/{column}/values")
    @serialized
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
        return category_response(con, table, metadata_columns, column, search=search, offset=offset, limit=limit)

    @api.post("/api/nodes/{node_id}/sql/columns/{column}/values")
    @serialized
    def sql_category_values(
        node_id: str,
        column: str,
        request: SQLRequest,
        search: str = "",
        offset: int = QueryParam(0, ge=0),
        limit: int = QueryParam(200, ge=1, le=500),
    ):
        con = get_connection(node_id)
        sql, columns = sql_metadata(con, request)
        return category_response(con, "query(?)", columns, column, [sql], search, offset, limit)

    def profile_response(
        con: duckdb.DuckDBPyConnection,
        source: str,
        values: list[Any],
        metadata_columns: list[tuple[str, str]],
        column: str,
    ) -> dict[str, Any]:
        columns = dict(metadata_columns)
        if column not in columns:
            raise HTTPException(404, "Column not found")
        type_ = columns[column]
        kind = profile_kind(type_)
        if kind is None:
            raise HTTPException(422, "Column cannot be profiled")
        field = quote(column)
        if kind == "categorical":
            row_count, non_null, null_count, distinct_count = con.execute(f"""
                SELECT count(*), count({field}), count(*) - count({field}), count(DISTINCT {field})
                FROM {source}
            """, values).fetchone()
            top_values = con.execute(f"""
                SELECT {field}, count(*) AS count
                FROM {source} WHERE {field} IS NOT NULL
                GROUP BY {field} ORDER BY count DESC, {field}
                LIMIT 20
            """, values).fetchall()
            return {
                "kind": kind, "type": type_, "row_count": safe(row_count), "non_null_count": safe(non_null),
                "null_count": safe(null_count), "null_fraction": null_count / row_count if row_count else 0.0,
                "distinct_count": safe(distinct_count),
                "top_values": [{"value": safe(value), "count": safe(count)} for value, count in top_values],
            }
        if kind == "date":
            row_count, non_null, null_count, distinct_count, minimum, maximum = con.execute(f"""
                SELECT count(*), count({field}), count(*) - count({field}), count(DISTINCT {field}),
                       min({field}), max({field})
                FROM {source}
            """, values).fetchone()
            histogram = []
            if non_null:
                bin_count = min(20, max(1, math.ceil(math.sqrt(non_null))))
                bins = con.execute(f"""
                    WITH ranked AS (
                        SELECT {field}, ntile(?) OVER (ORDER BY {field}) AS bin
                        FROM {source} WHERE {field} IS NOT NULL
                    )
                    SELECT min({field}), max({field}), count(*)
                    FROM ranked GROUP BY bin ORDER BY bin
                """, [bin_count, *values]).fetchall()
                histogram = [{"lower": safe(lower), "upper": safe(upper), "count": safe(count)} for lower, upper, count in bins]
            year_counts = [] if type_.upper().startswith("TIME") else con.execute(f"""
                SELECT strftime({field}, '%Y'), count(*)
                FROM {source} WHERE {field} IS NOT NULL
                GROUP BY 1 ORDER BY 1
            """, values).fetchall()
            return {
                "kind": kind, "type": type_, "row_count": safe(row_count), "non_null_count": safe(non_null),
                "null_count": safe(null_count), "null_fraction": null_count / row_count if row_count else 0.0,
                "distinct_count": safe(distinct_count), "min": safe(minimum), "max": safe(maximum), "histogram": histogram,
                "year_counts": [{"year": year, "count": safe(count)} for year, count in year_counts],
            }
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
            FROM {source}
        """, values).fetchone()
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
                        FROM {source} WHERE {field} IS NOT NULL
                    )
                    SELECT min({field}), max({field}), count(*)
                    FROM ranked GROUP BY bin ORDER BY bin
                """, [bin_count, *values]).fetchall()
                histogram = [{"lower": safe(lower), "upper": safe(upper), "count": safe(count)} for lower, upper, count in bins]
            elif finite_min == finite_max:
                histogram = [{"lower": finite_min, "upper": finite_max, "count": safe(finite_count)}]
            else:
                width = (finite_max - finite_min) / bin_count
                counts = dict(con.execute(f"""
                    SELECT least(?, floor(({field} - ?) / ?)::INTEGER), count(*)
                    FROM {source} WHERE {field} IS NOT NULL AND isfinite({field}::DOUBLE)
                    GROUP BY 1 ORDER BY 1
                """, [bin_count - 1, finite_min, width, *values]).fetchall())
                histogram = [{
                    "lower": safe(finite_min + index * width),
                    "upper": safe(finite_max if index == bin_count - 1 else finite_min + (index + 1) * width),
                    "count": safe(counts.get(index, 0)),
                } for index in range(bin_count)]
        return {
            "kind": kind, "type": type_, "row_count": safe(row_count), "non_null_count": safe(non_null),
            "null_count": safe(null_count), "null_fraction": null_count / row_count if row_count else 0.0,
            "min": safe(minimum), "max": safe(maximum), "mean": safe(mean), "stddev": safe(stddev),
            "p25": safe(p25), "median": safe(median), "p75": safe(p75), "histogram": histogram,
        }

    @api.get("/api/nodes/{node_id}/datasets/{dataset}/columns/{column}/stats")
    @api.post("/api/nodes/{node_id}/datasets/{dataset}/columns/{column}/stats")
    @serialized
    def stats(node_id: str, dataset: str, column: str, request: Query | None = None):
        con = get_connection(node_id)
        table, columns = metadata(con, dataset)
        source, values, _ = filtered_relation(table, columns, request or Query(page=1, page_size=100))
        return profile_response(con, source, values, columns, column)

    @api.post("/api/nodes/{node_id}/sql/columns/{column}/stats")
    @serialized
    def sql_stats(node_id: str, column: str, request: SQLQuery):
        con = get_connection(node_id)
        sql, columns = sql_metadata(con, request)
        source, values, _ = filtered_relation("query(?)", columns, request)
        return profile_response(con, source, [sql, *values], columns, column)

    frontend = Path(__file__).parent.parent / "frontend" / "dist"
    if frontend.is_dir():
        api.mount("/", StaticFiles(directory=frontend, html=True), name="frontend")
    return api


app = create_app()
