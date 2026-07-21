import json
import sys
import warnings
import zipfile
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent.parent))

import duckdb
import pytest

warnings.filterwarnings("ignore", message="Using `httpx` with `starlette.testclient` is deprecated.*")
from fastapi.testclient import TestClient

from backend.app import create_app, page_count, safe


@pytest.fixture
def client(tmp_path):
    with TestClient(create_app(tmp_path)) as client:
        yield client


def upload(client, name, content):
    response = client.post("/api/nodes/upload", files={"file": (name, content)})
    assert response.status_code == 201, response.text
    return response.json()


def dataset(client, node, name, schema="main"):
    datasets = client.get(f"/api/nodes/{node['id']}/datasets").json()
    return next(item for item in datasets if (item["schema"], item["name"]) == (schema, name))


def test_upload_list_datasets_delete_and_registry_restart(tmp_path):
    app = create_app(tmp_path)
    with TestClient(app) as client:
        node = upload(client, "people.csv", b"name,age\nAda,37\nBob,\n")
        assert node["name"] == "people.csv"
        assert node["kind"] == "upload"
        assert client.get("/api/nodes").json() == [node]
        datasets = client.get(f"/api/nodes/{node['id']}/datasets").json()
        assert datasets == [{
            "id": datasets[0]["id"], "name": "people", "schema": "main", "type": "VIEW",
            "columns": ["name", "age"],
        }]

    registry = json.loads((tmp_path / "registry.json").read_text())
    assert registry == [{**node, "dataset_name": "people"}]
    with TestClient(create_app(tmp_path)) as restarted:
        assert restarted.get("/api/nodes").json() == [node]
        assert restarted.delete(f"/api/nodes/{node['id']}").status_code == 204
        assert restarted.get("/api/nodes").json() == []
    assert Path(node["source"]).exists() is False


def test_flat_file_aliases_are_safe_and_persist_across_restart(tmp_path):
    cases = [
        ("Claims v1.csv", "claims_v1"),
        ("sales---Q2!!!.csv", "sales_q2"),
        ("123.csv", "data_123"),
        ("!!!.csv", "data"),
        ("Mixed.Case Name.csv", "mixed_case_name"),
    ]
    uploaded = []
    with TestClient(create_app(tmp_path)) as client:
        for filename, alias in cases:
            node = upload(client, filename, b"value\n1\n")
            uploaded.append((node, alias))
            assert [item["name"] for item in client.get(f"/api/nodes/{node['id']}/datasets").json()] == [alias]
            response = client.post(f"/api/nodes/{node['id']}/sql", json={"sql": f"SELECT * FROM {alias}"})
            assert response.status_code == 200, response.text
            assert response.json()["rows"] == [{"value": 1}]

    registry = json.loads((tmp_path / "registry.json").read_text())
    assert [node["dataset_name"] for node in registry] == [alias for _, alias in uploaded]
    with TestClient(create_app(tmp_path)) as restarted:
        for node, alias in uploaded:
            assert [item["name"] for item in restarted.get(f"/api/nodes/{node['id']}/datasets").json()] == [alias]
            response = restarted.post(f"/api/nodes/{node['id']}/sql", json={"sql": f"SELECT * FROM {alias}"})
            assert response.status_code == 200, response.text
            assert response.json()["rows"] == [{"value": 1}]


def test_legacy_registry_without_dataset_name_keeps_data_view(tmp_path):
    source = tmp_path / "uploads" / "legacy.csv"
    source.parent.mkdir()
    source.write_bytes(b"value\n1\n")
    node = {"id": "legacy", "name": "Claims v1.csv", "kind": "upload", "source": str(source)}
    (tmp_path / "registry.json").write_text(json.dumps([node]))

    with TestClient(create_app(tmp_path)) as client:
        assert [item["name"] for item in client.get("/api/nodes/legacy/datasets").json()] == ["data"]
        response = client.post("/api/nodes/legacy/sql", json={"sql": "SELECT * FROM data"})
        assert response.status_code == 200, response.text
        assert response.json()["rows"] == [{"value": 1}]
    assert json.loads((tmp_path / "registry.json").read_text()) == [node]


def test_upload_supported_formats_and_rejects_unsupported(client, tmp_path):
    parquet = tmp_path / "x.parquet"
    db = tmp_path / "x.duckdb"
    with duckdb.connect() as con:
        con.execute("COPY (SELECT 1 AS n) TO ? (FORMAT PARQUET)", [str(parquet)])
    with duckdb.connect(str(db)) as con:
        con.execute("CREATE TABLE items AS SELECT 1 AS n")

    cases = {
        "x.csv": b"n\n1\n",
        "x.tsv": b"n\n1\n",
        "x.json": b'[{"n":1}]',
        "x.ndjson": b'{"n":1}\n',
        "x.jsonl": b'{"n":1}\n',
        "x.parquet": parquet.read_bytes(),
        "x.duckdb": db.read_bytes(),
        "x.db": db.read_bytes(),
    }
    for name, content in cases.items():
        node = upload(client, name, content)
        names = [dataset["name"] for dataset in client.get(f"/api/nodes/{node['id']}/datasets").json()]
        assert names == (["items"] if name.endswith((".duckdb", ".db")) else ["x"])

    response = client.post("/api/nodes/upload", files={"file": ("x.txt", b"no")})
    assert response.status_code == 400


# ponytail: fixture-only workbook edit; use DuckDB append mode when available.
def add_workbook_sheet(workbook, name):
    with zipfile.ZipFile(workbook) as source:
        files = {item.filename: source.read(item) for item in source.infolist()}
    files["xl/workbook.xml"] = files["xl/workbook.xml"].replace(
        b"</sheets>", f'<sheet name="{name}" sheetId="2" r:id="rId99"/></sheets>'.encode()
    )
    files["xl/_rels/workbook.xml.rels"] = files["xl/_rels/workbook.xml.rels"].replace(
        b"</Relationships>", b'<Relationship Id="rId99" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet2.xml"/></Relationships>'
    )
    files["[Content_Types].xml"] = files["[Content_Types].xml"].replace(
        b"</Types>", b'<Override PartName="/xl/worksheets/sheet2.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/></Types>'
    )
    files["xl/worksheets/sheet2.xml"] = files["xl/worksheets/sheet1.xml"].replace(b">1<", b">2<")
    with zipfile.ZipFile(workbook, "w") as destination:
        for filename, content in files.items():
            destination.writestr(filename, content)


def workbook_preview(client, tmp_path):
    workbook = tmp_path / "sheets.xlsx"
    with duckdb.connect() as con:
        con.execute("INSTALL excel; LOAD excel")
        con.execute("COPY (SELECT 1 AS value) TO ? (FORMAT xlsx, SHEET 'People')", [str(workbook)])
    # ponytail: fixture-only ZIP/XML edit until DuckDB writes multiple worksheets itself.
    add_workbook_sheet(workbook, "O'Reilly")
    return upload(client, "sheets.xlsx", workbook.read_bytes())


def test_upload_workbook_requires_sheet_confirmation(client, tmp_path):
    preview = workbook_preview(client, tmp_path)
    assert preview == {"id": preview["id"], "name": "sheets.xlsx", "kind": "workbook", "sheets": ["People", "O'Reilly"]}
    assert client.get("/api/nodes").json() == []

    node = client.post(f"/api/nodes/upload/{preview['id']}/confirm", json={"sheets": ["O'Reilly"]})
    assert node.status_code == 200, node.text
    node = node.json()
    assert node["kind"] == "upload"
    sheets = client.get(f"/api/nodes/{node['id']}/datasets").json()
    assert [sheet["name"] for sheet in sheets] == ["O'Reilly"]
    response = client.post(f"/api/nodes/{node['id']}/datasets/{sheets[0]['id']}/query", json={})
    assert response.status_code == 200, response.text
    assert response.json()["rows"] == [{"A1": 2.0}]
    assert json.loads((tmp_path / "registry.json").read_text()) == [{**node, "sheets": ["O'Reilly"]}]


def test_workbook_confirmation_validates_selection_and_cancel(client, tmp_path):
    preview = workbook_preview(client, tmp_path)
    confirm = f"/api/nodes/upload/{preview['id']}/confirm"
    for sheets in ([], ["People", "People"], ["missing"]):
        assert client.post(confirm, json={"sheets": sheets}).status_code == 422
    assert client.delete(f"/api/nodes/upload/{preview['id']}").status_code == 204
    assert client.post(confirm, json={"sheets": ["People"]}).status_code == 404
    assert client.delete(f"/api/nodes/upload/{preview['id']}").status_code == 404


def test_restart_discards_unconfirmed_workbook_upload(tmp_path):
    with TestClient(create_app(tmp_path)) as client:
        preview = workbook_preview(client, tmp_path)
        source = tmp_path / "uploads" / f"{preview['id']}.xlsx"
        assert source.exists()

    with TestClient(create_app(tmp_path)) as restarted:
        assert restarted.post(f"/api/nodes/upload/{preview['id']}/confirm", json={"sheets": ["People"]}).status_code == 404
    assert source.exists() is False


def test_attach_is_read_only_and_missing_paths_fail(client, tmp_path):
    db = tmp_path / "existing.duckdb"
    with duckdb.connect(str(db)) as con:
        con.execute("CREATE TABLE numbers AS SELECT 1 AS n")
    response = client.post("/api/nodes/attach", json={"path": str(db)})
    assert response.status_code == 201
    node = response.json()
    assert node["kind"] == "attached"
    datasets = client.get(f"/api/nodes/{node['id']}/datasets").json()
    assert datasets == [{
        "id": datasets[0]["id"], "name": "numbers", "schema": "main", "type": "TABLE", "columns": ["n"],
    }]
    assert client.post("/api/nodes/attach", json={"path": str(tmp_path / 'missing.db')}).status_code == 404
    assert client.post("/api/nodes/attach", json={"path": str(tmp_path / 'bad.csv')}).status_code == 400
    assert client.delete(f"/api/nodes/{node['id']}").status_code == 204
    assert db.exists()


def test_uploaded_sql_blocks_external_files_but_queries_registered_view(client, tmp_path):
    secret = tmp_path / "secret.txt"
    secret.write_text("private")
    node = upload(client, "items.csv", b"name\nAda\n")
    url = f"/api/nodes/{node['id']}/sql"

    assert client.post(url, json={"sql": "SELECT * FROM main.items"}).status_code == 200
    assert client.post(url, json={"sql": "SELECT content FROM read_text('/etc/hosts')"}).status_code == 422
    assert client.post(url, json={"sql": f"SELECT * FROM glob('{secret}')"}).status_code == 422


def test_attached_sql_blocks_external_files_but_queries_database_table(client, tmp_path):
    db = tmp_path / "existing.duckdb"
    secret = tmp_path / "secret.txt"
    secret.write_text("private")
    with duckdb.connect(str(db)) as con:
        con.execute("CREATE TABLE numbers AS SELECT 1 AS n")
    node = client.post("/api/nodes/attach", json={"path": str(db)}).json()
    url = f"/api/nodes/{node['id']}/sql"

    response = client.post(url, json={"sql": "SELECT * FROM numbers"})
    assert response.status_code == 200, response.text
    assert response.json()["rows"] == [{"n": 1}]
    assert client.post(url, json={"sql": f"SELECT content FROM read_text('{secret}')"}).status_code == 422


def test_dataset_ids_distinguish_schemas_and_slashes(client, tmp_path):
    db = tmp_path / "schemas.duckdb"
    with duckdb.connect(str(db)) as con:
        con.execute("CREATE SCHEMA other")
        con.execute("CREATE TABLE main.items AS SELECT 'main' AS source")
        con.execute("CREATE TABLE other.items AS SELECT 'other' AS source")
        con.execute("CREATE TABLE other.\"with/slash\" AS SELECT 'slash' AS source")

    node = client.post("/api/nodes/attach", json={"path": str(db)}).json()
    datasets = client.get(f"/api/nodes/{node['id']}/datasets").json()
    assert [(item["schema"], item["name"]) for item in datasets] == [
        ("main", "items"), ("other", "items"), ("other", "with/slash")
    ]
    assert len({item["id"] for item in datasets}) == 3
    for dataset, expected in zip(datasets, ("main", "other", "slash")):
        response = client.post(
            f"/api/nodes/{node['id']}/datasets/{dataset['id']}/query", json={}
        )
        assert response.status_code == 200, response.text
        assert response.json()["rows"] == [{"source": expected}]


def test_dataset_list_includes_columns_for_every_table(client, tmp_path):
    db = tmp_path / "columns.duckdb"
    with duckdb.connect(str(db)) as con:
        con.execute('CREATE TABLE another(id BIGINT, active BOOLEAN, note VARCHAR)')
        con.execute('CREATE TABLE "odd table"("first field" INTEGER, second VARCHAR)')

    node = client.post("/api/nodes/attach", json={"path": str(db)}).json()
    datasets = client.get(f"/api/nodes/{node['id']}/datasets").json()

    assert datasets == [
        {
            "id": datasets[0]["id"], "name": "another", "schema": "main", "type": "TABLE",
            "columns": ["id", "active", "note"],
        },
        {
            "id": datasets[1]["id"], "name": "odd table", "schema": "main", "type": "TABLE",
            "columns": ["first field", "second"],
        },
    ]


def test_query_pages_repeated_filters_ordered_multisort_and_null_metadata(client):
    node = upload(
        client,
        "items.csv",
        b"category,name,price,rank\nx,alpha,10,2\nx,alpine,20,1\nx,beta,,3\ny,alto,30,4\n",
    )
    url = f"/api/nodes/{node['id']}/datasets/{dataset(client, node, 'items')['id']}/query"
    response = client.post(url, json={
        "page": 1,
        "page_size": 1,
        "filters": [
            {"column": "category", "operator": "=", "value": "x"},
            {"column": "name", "operator": "starts_with", "value": "al"},
            {"column": "price", "operator": ">=", "value": 10},
            {"column": "price", "operator": "<=", "value": 20},
        ],
        "sorts": [
            {"column": "category", "direction": "asc"},
            {"column": "price", "direction": "desc"},
        ],
    })
    assert response.status_code == 200, response.text
    body = response.json()
    assert body["rows"] == [{"category": "x", "name": "alpine", "price": 20, "rank": 1}]
    assert (body["page"], body["page_size"], body["total_rows"], body["total_pages"]) == (1, 1, 2, 2)
    assert body["elapsed_ms"] >= 0
    columns = {column["name"]: column for column in body["columns"]}
    assert columns["price"]["numeric"] is True
    assert columns["name"]["numeric"] is False
    assert columns["price"]["null_fraction"] == 0.0


def test_sql_query_pages_with_metadata_and_safe_values(client):
    node = upload(
        client,
        "items.csv",
        b"name,value,day\nalpha,9007199254740993,2025-01-02\nbeta,,\n",
    )
    response = client.post(f"/api/nodes/{node['id']}/sql", json={
        "sql": 'SELECT name, value, day FROM "main"."items" ORDER BY name;',
        "page": 1,
        "page_size": 1,
    })
    assert response.status_code == 200, response.text
    body = response.json()
    assert body["sql"] == 'SELECT name, value, day FROM "main"."items" ORDER BY name;'
    assert body["rows"] == [{"name": "alpha", "value": "9007199254740993", "day": "2025-01-02"}]
    assert (body["page"], body["page_size"], body["total_rows"], body["total_pages"]) == (1, 1, 2, 2)
    columns = {column["name"]: column for column in body["columns"]}
    assert columns["name"] == {
        "name": "name", "type": "VARCHAR", "numeric": False,
        "profile_kind": "categorical", "null_fraction": 0.0,
    }
    assert columns["value"]["numeric"] is True
    assert columns["value"]["profile_kind"] == "numeric"
    assert columns["value"]["null_fraction"] == 0.5
    assert columns["day"]["profile_kind"] == "date"
    assert body["elapsed_ms"] >= 0


@pytest.mark.parametrize("sql", [
    "SELECT 1 AS value -- trailing",
    "SELECT 1 AS value; -- trailing",
    "SELECT 1 AS value /* trailing */",
    "SELECT 1 AS value; /* trailing */",
])
def test_sql_query_accepts_trailing_comments(client, sql):
    node = upload(client, "items.csv", b"name\nAda\n")
    response = client.post(f"/api/nodes/{node['id']}/sql", json={"sql": f"  {sql}  "})

    assert response.status_code == 200, response.text
    assert response.json()["rows"] == [{"value": 1}]
    assert response.json()["sql"] == sql


@pytest.mark.parametrize("sql", [
    "", "   ", "UPDATE data SET name = 'x'", "CREATE TABLE x(i INTEGER)",
    "DELETE FROM data", "INSERT INTO data VALUES ('x', 1, NULL)",
    "COPY data TO '/tmp/items.csv'", "ATTACH '/tmp/items.duckdb'", "PRAGMA version",
    "SELECT 1; SELECT 2",
])
def test_sql_query_rejects_non_select_blank_and_multiple_statements(client, sql):
    node = upload(client, "items.csv", b"name,value\na,1\n")
    response = client.post(f"/api/nodes/{node['id']}/sql", json={"sql": sql})
    assert response.status_code == 422
    assert "only one read-only SELECT" in response.json()["detail"]


@pytest.mark.parametrize("payload", [
    {"page": 0},
    {"page_size": 1001},
    {"filters": [{"column": "missing", "operator": "=", "value": "x"}]},
    {"filters": [{"column": "name", "operator": "bogus", "value": "x"}]},
    {"sorts": [{"column": "missing", "direction": "asc"}]},
    {"sorts": [{"column": "name", "direction": "sideways"}]},
])
def test_query_rejects_invalid_paging_and_metadata(client, payload):
    node = upload(client, "x.csv", b'name,value\na,1\n')
    response = client.post(f"/api/nodes/{node['id']}/datasets/{dataset(client, node, 'x')['id']}/query", json=payload)
    assert response.status_code == 422


def test_filter_operators_and_bound_values(client):
    node = upload(client, "x.csv", b'name,value\nalpha,1\nalpine,2\nbeta,\n')
    url = f"/api/nodes/{node['id']}/datasets/{dataset(client, node, 'x')['id']}/query"
    checks = [
        ({"column": "name", "operator": "contains", "value": "ph"}, 1),
        ({"column": "name", "operator": "ends_with", "value": "a"}, 2),
        ({"column": "name", "operator": "!=", "value": "alpha"}, 2),
        ({"column": "value", "operator": "is_null"}, 1),
        ({"column": "value", "operator": "not_null"}, 2),
        ({"column": "value", "operator": ">", "value": 1}, 1),
    ]
    for condition, expected in checks:
        response = client.post(url, json={"filters": [condition]})
        assert response.status_code == 200, response.text
        assert response.json()["total_rows"] == expected

    injection = client.post(url, json={"filters": [{"column": "name", "operator": "=", "value": "' OR true --"}]})
    assert injection.json()["total_rows"] == 0
    assert client.post(url, json={"filters": [{"column": "value", "operator": "contains", "value": "1"}]}).status_code == 422
    assert client.post(url, json={"filters": [{"column": "name", "operator": "="}]}).status_code == 422
    assert client.post(url, json={"filters": [{"column": "value", "operator": ">", "value": "not-a-number"}]}).status_code == 422
    assert client.post(f"/api/nodes/{node['id']}/datasets/missing/query", json={}).status_code == 404


def test_query_preserves_large_integers_and_filters_exactly(client, tmp_path):
    db = tmp_path / "integers.duckdb"
    with duckdb.connect(str(db)) as con:
        con.execute("""
            CREATE TABLE integers(signed BIGINT, unsigned UBIGINT, ordinary INTEGER);
            INSERT INTO integers VALUES
              (9007199254740993, 18446744073709551615, 42),
              (9007199254740992, 18446744073709551614, 43),
              (9007199254740994, 18446744073709551613, 44)
        """)
    node = client.post("/api/nodes/attach", json={"path": str(db)}).json()
    url = f"/api/nodes/{node['id']}/datasets/{dataset(client, node, 'integers')['id']}/query"

    response = client.post(url, json={"sorts": [{"column": "ordinary", "direction": "asc"}]})
    assert response.status_code == 200, response.text
    assert response.json()["rows"][0] == {
        "signed": "9007199254740993",
        "unsigned": "18446744073709551615",
        "ordinary": 42,
    }

    for condition in (
        {"column": "signed", "operator": "=", "value": "9007199254740993"},
        {"column": "unsigned", "operator": "in", "value": ["18446744073709551615"]},
    ):
        filtered = client.post(url, json={"filters": [condition]})
        assert filtered.status_code == 200, filtered.text
        assert filtered.json()["total_rows"] == 1
        assert filtered.json()["rows"][0]["ordinary"] == 42

    stats = client.get(url.removesuffix("/query") + "/columns/signed/stats")
    assert stats.status_code == 200, stats.text
    body = stats.json()
    assert body["kind"] == "numeric"
    assert (body["min"], body["max"]) == ("9007199254740992", "9007199254740994")
    assert body["histogram"] == [
        {"lower": "9007199254740992", "upper": "9007199254740993", "count": 2},
        {"lower": "9007199254740994", "upper": "9007199254740994", "count": 1},
    ]


def test_category_values_are_distinct_counted_safe_and_text_only(client, tmp_path):
    db = tmp_path / "categories.duckdb"
    with duckdb.connect(str(db)) as con:
        con.execute("""
            CREATE TABLE items(category VARCHAR, amount INTEGER);
            INSERT INTO items VALUES
              ('beta', 1), ('beta', 2), ('O''Reilly', 3),
              (''' OR true --', 4), (NULL, 5)
        """)
    node = client.post("/api/nodes/attach", json={"path": str(db)}).json()
    base = f"/api/nodes/{node['id']}/datasets/{dataset(client, node, 'items')['id']}"

    response = client.get(base + "/columns/category/values")
    assert response.status_code == 200, response.text
    assert response.json() == {
        "values": [
            {"value": "beta", "count": 2},
            {"value": "' OR true --", "count": 1},
            {"value": "O'Reilly", "count": 1},
        ],
        "total": 3,
        "offset": 0,
        "limit": 200,
        "has_more": False,
    }
    assert client.get(base + "/columns/category/values", params={"search": "' OR true --"}).json()["values"] == [
        {"value": "' OR true --", "count": 1}
    ]
    assert client.get(base + "/columns/amount/values").status_code == 422
    assert client.get(base + "/columns/missing/values").status_code == 404
    assert client.get(f"/api/nodes/{node['id']}/datasets/missing/columns/category/values").status_code == 404


def test_category_values_are_paged_and_searchable(client, tmp_path):
    db = tmp_path / "many-categories.duckdb"
    with duckdb.connect(str(db)) as con:
        con.execute("CREATE TABLE items AS SELECT printf('value-%03d', i) AS category FROM range(601) t(i)")
    node = client.post("/api/nodes/attach", json={"path": str(db)}).json()
    url = f"/api/nodes/{node['id']}/datasets/{dataset(client, node, 'items')['id']}/columns/category/values"

    first = client.get(url).json()
    assert len(first["values"]) == 200
    assert first == {
        "values": [{"value": f"value-{i:03d}", "count": 1} for i in range(200)],
        "total": 601,
        "offset": 0,
        "limit": 200,
        "has_more": True,
    }
    second = client.get(url, params={"offset": 200}).json()
    assert second["values"][0]["value"] == "value-200"
    assert second["values"][-1]["value"] == "value-399"
    assert (second["total"], second["offset"], second["limit"], second["has_more"]) == (601, 200, 200, True)

    searched = client.get(url, params={"search": "VALUE-59", "limit": 3}).json()
    assert searched == {
        "values": [{"value": f"value-{i}", "count": 1} for i in range(590, 593)],
        "total": 10,
        "offset": 0,
        "limit": 3,
        "has_more": True,
    }
    assert client.get(url, params={"offset": -1}).status_code == 422
    assert client.get(url, params={"limit": 0}).status_code == 422
    assert client.get(url, params={"limit": 501}).status_code == 422


def test_in_filter_single_multi_and_validation(client):
    node = upload(client, "items.csv", b"category,kind\na,x\nb,x\nc,y\n' OR true --,z\n")
    url = f"/api/nodes/{node['id']}/datasets/{dataset(client, node, 'items')['id']}/query"

    checks = [
        ({"column": "category", "operator": "in", "value": ["a"]}, 1),
        ({"column": "category", "operator": "in", "value": ["a", "b"]}, 2),
        ({"column": "category", "operator": "in", "value": ["' OR true --"]}, 1),
    ]
    for condition, expected in checks:
        response = client.post(url, json={"filters": [condition]})
        assert response.status_code == 200, response.text
        assert response.json()["total_rows"] == expected

    combined = client.post(url, json={"filters": [
        {"column": "category", "operator": "in", "value": ["a", "b", "c"]},
        {"column": "kind", "operator": "=", "value": "x"},
    ]})
    assert combined.status_code == 200, combined.text
    assert combined.json()["total_rows"] == 2
    for value in ([], "a", None):
        response = client.post(url, json={"filters": [{"column": "category", "operator": "in", "value": value}]})
        assert response.status_code == 422


def test_numeric_stats_histogram_and_json_safe_values(client, tmp_path):
    db = tmp_path / "types.duckdb"
    with duckdb.connect(str(db)) as con:
        con.execute("""
            CREATE TABLE values AS
            SELECT * FROM (VALUES
              (DATE '2025-01-02', DECIMAL '1.25', 1.0),
              (DATE '2025-01-03', DECIMAL '2.50', 'NaN'::DOUBLE),
              (NULL, NULL, 'Infinity'::DOUBLE)
            ) t(day, amount, floating)
        """)
    node = client.post("/api/nodes/attach", json={"path": str(db)}).json()
    base = f"/api/nodes/{node['id']}/datasets/{dataset(client, node, 'values')['id']}"
    query = client.post(base + "/query", json={"page_size": 10})
    assert query.status_code == 200, query.text
    rows = query.json()["rows"]
    assert rows[0]["day"] == "2025-01-02"
    assert rows[0]["amount"] == 1.25
    assert rows[1]["floating"] is None
    assert rows[2]["floating"] is None

    stats = client.get(base + "/columns/amount/stats")
    assert stats.status_code == 200, stats.text
    body = stats.json()
    assert body["kind"] == "numeric"
    assert body["type"].startswith("DECIMAL")
    assert body["row_count"] == 3
    assert body["non_null_count"] == 2
    assert body["null_count"] == 1
    assert body["null_fraction"] == pytest.approx(1 / 3)
    assert body["min"] == 1.25
    assert body["max"] == 2.5
    assert body["p25"] <= body["median"] <= body["p75"]
    assert 1 <= len(body["histogram"]) <= 20
    assert sum(bin_["count"] for bin_ in body["histogram"]) == 2
    floating = client.get(base + "/columns/floating/stats")
    assert floating.status_code == 200
    assert floating.json()["kind"] == "numeric"
    assert sum(bin_["count"] for bin_ in floating.json()["histogram"]) == 1
    date_stats = client.get(base + "/columns/day/stats")
    assert date_stats.status_code == 200, date_stats.text
    assert date_stats.json()["kind"] == "date"
    assert date_stats.json()["histogram"] == [
        {"lower": "2025-01-02", "upper": "2025-01-02", "count": 1},
        {"lower": "2025-01-03", "upper": "2025-01-03", "count": 1},
    ]
    assert client.get(base + "/columns/missing/stats").status_code == 404


def test_safe_serializes_unsafe_aggregate_integers():
    assert safe(2**53 - 1) == 2**53 - 1
    assert safe(2**53) == "9007199254740992"
    assert page_count(9007199254740993, 1) == 9007199254740993
    assert page_count(10, 3) == 4


def test_missing_nodes_and_stale_registry_are_not_active(tmp_path):
    (tmp_path / "registry.json").write_text(json.dumps([{
        "id": "gone", "name": "gone.csv", "kind": "upload", "source": str(tmp_path / "gone.csv")
    }]))
    with TestClient(create_app(tmp_path)) as client:
        assert client.get("/api/nodes").json() == []
        assert client.get("/api/nodes/gone/datasets").status_code == 404


def test_profile_kinds_and_categorical_and_date_stats(client, tmp_path):
    db = tmp_path / "profiles.duckdb"
    with duckdb.connect(str(db)) as con:
        con.execute("""
            CREATE TYPE mood AS ENUM ('happy', 'sad');
            CREATE TABLE profiles(category VARCHAR, flag BOOLEAN, mood mood, day DATE, moment TIMESTAMP, empty_day DATE, amount INTEGER);
            INSERT INTO profiles VALUES
              ('red', true, 'happy', DATE '2025-01-01', TIMESTAMP '2025-01-01 10:00:00', NULL, 1),
              ('red', false, 'happy', DATE '2025-01-02', TIMESTAMP '2025-01-02 10:00:00', NULL, 2),
              ('blue', NULL, 'sad', NULL, NULL, NULL, 3),
              (NULL, true, NULL, DATE '2025-01-04', TIMESTAMP '2025-01-04 10:00:00', NULL, 4)
        """)
    node = client.post("/api/nodes/attach", json={"path": str(db)}).json()
    base = f"/api/nodes/{node['id']}/datasets/{dataset(client, node, 'profiles')['id']}"

    query = client.post(base + "/query", json={}).json()
    kinds = {column["name"]: column["profile_kind"] for column in query["columns"]}
    assert kinds == {
        "category": "categorical", "flag": "categorical", "mood": "categorical",
        "day": "date", "moment": "date", "empty_day": "date", "amount": "numeric",
    }

    category = client.get(base + "/columns/category/stats")
    assert category.status_code == 200, category.text
    assert category.json() == {
        "kind": "categorical", "type": "VARCHAR", "row_count": 4, "non_null_count": 3, "null_count": 1,
        "null_fraction": 0.25, "distinct_count": 2,
        "top_values": [{"value": "red", "count": 2}, {"value": "blue", "count": 1}],
    }
    day = client.get(base + "/columns/day/stats")
    assert day.status_code == 200, day.text
    assert day.json()["kind"] == "date"
    assert day.json()["distinct_count"] == 3
    assert (day.json()["min"], day.json()["max"]) == ("2025-01-01", "2025-01-04")
    assert sum(bin_["count"] for bin_ in day.json()["histogram"]) == 3
    empty_day = client.get(base + "/columns/empty_day/stats")
    assert empty_day.json()["kind"] == "date"
    assert (empty_day.json()["min"], empty_day.json()["max"], empty_day.json()["histogram"]) == (None, None, [])


def test_query_dedupes_filtered_multi_column_rows_and_validates_keys(client):
    node = upload(client, "dedupe.csv", b"group,kind,score,label\na,x,1,first\na,x,3,second\na,y,2,third\nb,x,4,fourth\n")
    url = f"/api/nodes/{node['id']}/datasets/{dataset(client, node, 'dedupe')['id']}/query"
    response = client.post(url, json={
        "filters": [{"column": "group", "operator": "in", "value": ["a", "b"]}],
        "dedupe_columns": ["group", "kind"],
        "sorts": [{"column": "score", "direction": "desc"}],
        "page_size": 10,
    })
    assert response.status_code == 200, response.text
    assert response.json()["rows"] == [
        {"group": "b", "kind": "x", "score": 4, "label": "fourth"},
        {"group": "a", "kind": "y", "score": 2, "label": "third"},
        {"group": "a", "kind": "x", "score": 1, "label": "first"},
    ]
    assert (response.json()["total_rows"], response.json()["total_pages"]) == (3, 1)
    assert client.post(url, json={"dedupe_columns": ["group", "group"]}).status_code == 422
    assert client.post(url, json={"dedupe_columns": ["missing"]}).status_code == 422


def test_builder_query_returns_equivalent_executable_sql(client):
    node = upload(
        client,
        "dedupe.csv",
        b"group,kind,score,label\na,O'Reilly,1,first\na,O'Reilly,3,second\na,x,2,third\nb,O'Reilly,4,fourth\n",
    )
    url = f"/api/nodes/{node['id']}/datasets/{dataset(client, node, 'dedupe')['id']}/query"
    built = client.post(url, json={
        "filters": [{"column": "kind", "operator": "=", "value": "O'Reilly"}],
        "dedupe_columns": ["group", "kind"],
        "sorts": [{"column": "score", "direction": "desc"}],
        "page_size": 10,
    })
    assert built.status_code == 200, built.text
    body = built.json()
    assert "?" not in body["sql"]

    executed = client.post(f"/api/nodes/{node['id']}/sql", json={"sql": body["sql"], "page_size": 10})
    assert executed.status_code == 200, executed.text
    assert executed.json()["rows"] == body["rows"]
    assert executed.json()["total_rows"] == body["total_rows"]


def test_filtered_deduped_query_metadata_and_stats_share_rows(client):
    node = upload(client, "scoped.csv", b"group,category,amount\nscope,alpha,1\nscope,,2\nother,beta,100\nother,beta,200\n")
    base = f"/api/nodes/{node['id']}/datasets/{dataset(client, node, 'scoped')['id']}"
    request = {
        "filters": [{"column": "group", "operator": "=", "value": "scope"}],
        "dedupe_columns": ["group"],
    }

    query = client.post(base + "/query", json=request)
    assert query.status_code == 200, query.text
    body = query.json()
    assert body["total_rows"] == 1
    assert {column["name"]: column["null_fraction"] for column in body["columns"]}["category"] == 0.0

    stats = client.post(base + "/columns/category/stats", json=request)
    assert stats.status_code == 200, stats.text
    assert stats.json()["row_count"] == 1
    assert stats.json()["null_count"] == 0
    assert stats.json()["top_values"] == [{"value": "alpha", "count": 1}]
    amount = client.post(base + "/columns/amount/stats", json=request)
    assert amount.status_code == 200, amount.text
    assert (amount.json()["min"], amount.json()["max"]) == (1, 1)


def test_stats_invalid_filter_value_returns_422(tmp_path):
    with TestClient(create_app(tmp_path), raise_server_exceptions=False) as stats_client:
        node = upload(stats_client, "values.csv", b"value\n1\n2\n")
        base = f"/api/nodes/{node['id']}/datasets/{dataset(stats_client, node, 'values')['id']}"
        response = stats_client.post(base + "/columns/value/stats", json={
            "filters": [{"column": "value", "operator": ">", "value": "not-a-number"}],
        })
    assert response.status_code == 422
