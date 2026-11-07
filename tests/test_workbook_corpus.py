"""Pins how the Python backend reads xlsx workbooks: column names, DuckDB types and every row."""
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent.parent))

import pytest

from backend_client import make_client
from workbook_corpus import CASES

D, V, TS, TM = "DOUBLE", "VARCHAR", "TIMESTAMP", "TIME"

WIDE_COLUMNS = [(f"col{i}", D) for i in range(1, 51)]

# case -> sheet -> (columns as (name, DuckDB type), rows); every value is what the Python backend returned.
EXPECTED = {
    "integers_and_floats": {"Numbers": (
        [("whole", D), ("fraction", D), ("mixed", D)],
        [{"whole": 1.0, "fraction": 0.5, "mixed": 1.0},
         {"whole": 2.0, "fraction": 1.25, "mixed": 2.5},
         {"whole": -3.0, "fraction": 1000000.0, "mixed": 3.0}],
    )},
    "styled_dates_datetimes_and_times": {"Dates": (
        [("day", "DATE"), ("moment", TS), ("clock", TM)],
        [{"day": "2024-01-02", "moment": "2024-01-02T12:00:00", "clock": "09:30:15"},
         {"day": "2025-12-31", "moment": "2025-12-31T23:59:59", "clock": "00:00:00"}],
    )},
    "booleans": {"Flags": (
        [("flag", D), ("maybe", D)],
        [{"flag": 1.0, "maybe": 1.0}, {"flag": 0.0, "maybe": None}, {"flag": 1.0, "maybe": 0.0}],
    )},
    "leading_zero_identifiers": {"Ids": (
        [("code", D), ("sku", V), ("name", V)],
        [{"code": 7.0, "sku": "00-1", "name": "Bond"},
         {"code": 12.0, "sku": "0A2", "name": "Twelve"},
         {"code": 100.0, "sku": "100", "name": "Hundred"}],
    )},
    "mostly_text_numeric_column": {"Mixed": (
        [("value", V)],
        [{"value": value} for value in ["1", "2", "3", "4", "5", "6", "7", "n/a", "unknown", "x", "tbd"]],
    )},
    "empty_cells_and_empty_row": {"Gaps": (
        [("a", D), ("b", V), ("c", V)],
        [{"a": 1.0, "b": None, "c": "x"}],
    )},
    "numeric_first_row_has_no_header": {"Bare": (
        [("A1", D), ("B1", D), ("C1", D)],
        [{"A1": 1.0, "B1": 2.0, "C1": 3.0}, {"A1": 4.0, "B1": 5.0, "C1": 6.0}, {"A1": 7.0, "B1": 8.0, "C1": 9.0}],
    )},
    "header_only_sheet": {"Empty": ([("alpha", V), ("beta", V)], [])},
    "duplicate_header_names": {"Dupes": (
        [("name", V), ("name_1", V), ("Name_2", V), ("x", D)],
        [{"name": "a", "name_1": "b", "Name_2": "c", "x": 1.0}, {"name": "d", "name_1": "e", "Name_2": "f", "x": 2.0}],
    )},
    "apostrophe_and_non_ascii_sheet_names": {
        "O'Reilly": ([("book", V)], [{"book": "Learning SQL"}]),
        "Café Müller": (
            [("city", V), ("count", D)],
            [{"city": "Zürich", "count": 3.0}, {"city": "São Paulo", "count": 4.0}],
        ),
    },
    "fifty_columns": {"Wide": (
        WIDE_COLUMNS,
        [{f"col{i}": float(i) for i in range(1, 51)}, {f"col{i}": i / 2 for i in range(1, 51)}],
    )},
    "formula_like_text": {"Formulas": (
        [("text", V), ("other", V)],
        [{"text": "=1+1", "other": "plain"}, {"text": "+SUM(A1)", "other": "@cmd"}, {"text": "-5x", "other": "ok"}],
    )},
}


@pytest.fixture
def client(tmp_path):
    with make_client(tmp_path / "data") as client:
        yield client


@pytest.mark.parametrize("case", CASES)
def test_workbook_corpus_reads_names_types_and_rows_like_python(client, tmp_path, case):
    path = tmp_path / f"{case}.xlsx"
    sheets = CASES[case](path)
    response = client.post("/api/nodes/upload", files={"file": (path.name, path.read_bytes())})
    assert response.status_code == 201, response.text
    preview = response.json()
    assert preview["sheets"] == sheets

    response = client.post(f"/api/nodes/upload/{preview['id']}/confirm", json={"sheets": sheets})
    assert response.status_code == 200, response.text
    node = response.json()
    datasets = {item["name"]: item for item in client.get(f"/api/nodes/{node['id']}/datasets").json()}
    assert set(datasets) == set(EXPECTED[case])

    for sheet, (columns, rows) in EXPECTED[case].items():
        assert datasets[sheet]["columns"] == [name for name, _ in columns]
        query = client.post(f"/api/nodes/{node['id']}/datasets/{datasets[sheet]['id']}/query", json={})
        assert query.status_code == 200, query.text
        body = query.json()
        assert [(column["name"], column["type"]) for column in body["columns"]] == columns
        assert body["total_rows"] == len(rows)
        assert body["rows"] == rows
