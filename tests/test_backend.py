import json
import sys
import warnings
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent.parent))

import duckdb
import pytest

warnings.filterwarnings("ignore", message="Using `httpx` with `starlette.testclient` is deprecated.*")
from fastapi.testclient import TestClient

from backend.app import create_app


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
        assert datasets == [{"id": datasets[0]["id"], "name": "data", "schema": "main", "type": "VIEW"}]

    registry = json.loads((tmp_path / "registry.json").read_text())
    assert registry == [node]
    with TestClient(create_app(tmp_path)) as restarted:
        assert restarted.get("/api/nodes").json() == [node]
        assert restarted.delete(f"/api/nodes/{node['id']}").status_code == 204
        assert restarted.get("/api/nodes").json() == []
    assert Path(node["source"]).exists() is False


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
        "x.parquet": parquet.read_bytes(),
        "x.duckdb": db.read_bytes(),
        "x.db": db.read_bytes(),
    }
    for name, content in cases.items():
        node = upload(client, name, content)
        names = [dataset["name"] for dataset in client.get(f"/api/nodes/{node['id']}/datasets").json()]
        assert names == (["items"] if name.endswith((".duckdb", ".db")) else ["data"])

    response = client.post("/api/nodes/upload", files={"file": ("x.txt", b"no")})
    assert response.status_code == 400


def test_attach_is_read_only_and_missing_paths_fail(client, tmp_path):
    db = tmp_path / "existing.duckdb"
    with duckdb.connect(str(db)) as con:
        con.execute("CREATE TABLE numbers AS SELECT 1 AS n")
    response = client.post("/api/nodes/attach", json={"path": str(db)})
    assert response.status_code == 201
    node = response.json()
    assert node["kind"] == "attached"
    datasets = client.get(f"/api/nodes/{node['id']}/datasets").json()
    assert datasets == [{"id": datasets[0]["id"], "name": "numbers", "schema": "main", "type": "TABLE"}]
    assert client.post("/api/nodes/attach", json={"path": str(tmp_path / 'missing.db')}).status_code == 404
    assert client.post("/api/nodes/attach", json={"path": str(tmp_path / 'bad.csv')}).status_code == 400
    assert client.delete(f"/api/nodes/{node['id']}").status_code == 204
    assert db.exists()


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


def test_query_pages_repeated_filters_ordered_multisort_and_null_metadata(client):
    node = upload(
        client,
        "items.csv",
        b"category,name,price,rank\nx,alpha,10,2\nx,alpine,20,1\nx,beta,,3\ny,alto,30,4\n",
    )
    url = f"/api/nodes/{node['id']}/datasets/{dataset(client, node, 'data')['id']}/query"
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
    assert columns["price"]["null_fraction"] == 0.25


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
    response = client.post(f"/api/nodes/{node['id']}/datasets/{dataset(client, node, 'data')['id']}/query", json=payload)
    assert response.status_code == 422


def test_filter_operators_and_bound_values(client):
    node = upload(client, "x.csv", b'name,value\nalpha,1\nalpine,2\nbeta,\n')
    url = f"/api/nodes/{node['id']}/datasets/{dataset(client, node, 'data')['id']}/query"
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
    assert response.json() == {"values": [
        {"value": "beta", "count": 2},
        {"value": "' OR true --", "count": 1},
        {"value": "O'Reilly", "count": 1},
    ]}
    assert client.get(base + "/columns/amount/values").status_code == 422
    assert client.get(base + "/columns/missing/values").status_code == 404
    assert client.get(f"/api/nodes/{node['id']}/datasets/missing/columns/category/values").status_code == 404


def test_in_filter_single_multi_and_validation(client):
    node = upload(client, "items.csv", b"category,kind\na,x\nb,x\nc,y\n' OR true --,z\n")
    url = f"/api/nodes/{node['id']}/datasets/{dataset(client, node, 'data')['id']}/query"

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
    assert sum(bin_["count"] for bin_ in floating.json()["histogram"]) == 1
    assert client.get(base + "/columns/day/stats").status_code == 422
    assert client.get(base + "/columns/missing/stats").status_code == 404


def test_missing_nodes_and_stale_registry_are_not_active(tmp_path):
    (tmp_path / "registry.json").write_text(json.dumps([{
        "id": "gone", "name": "gone.csv", "kind": "upload", "source": str(tmp_path / "gone.csv")
    }]))
    with TestClient(create_app(tmp_path)) as client:
        assert client.get("/api/nodes").json() == []
        assert client.get("/api/nodes/gone/datasets").status_code == 404
