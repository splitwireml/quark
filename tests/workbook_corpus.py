"""Workbook fixtures pinning how the backend reads xlsx files; one builder per case."""
import datetime as dt
from pathlib import Path

from openpyxl import Workbook


def _save(path: Path, sheets: dict[str, list[list]]) -> list[str]:
    """Write `sheets` (name -> rows, None leaves a cell empty) and return the sheet names in order."""
    workbook = Workbook()
    workbook.remove(workbook.active)
    for name, rows in sheets.items():
        sheet = workbook.create_sheet(name)
        for row_index, row in enumerate(rows, start=1):
            for column_index, value in enumerate(row, start=1):
                if value is not None:
                    sheet.cell(row=row_index, column=column_index, value=value)
    workbook.save(path)
    return list(sheets)


def integers_and_floats(path: Path) -> list[str]:
    return _save(path, {"Numbers": [["whole", "fraction", "mixed"], [1, 0.5, 1], [2, 1.25, 2.5], [-3, 1e6, 3]]})


def styled_dates_datetimes_and_times(path: Path) -> list[str]:
    rows = [
        ["day", "moment", "clock"],
        [dt.date(2024, 1, 2), dt.datetime(2024, 1, 2, 12, 0), dt.time(9, 30, 15)],
        [dt.date(2025, 12, 31), dt.datetime(2025, 12, 31, 23, 59, 59), dt.time(0, 0)],
    ]
    return _save(path, {"Dates": rows})


def booleans(path: Path) -> list[str]:
    return _save(path, {"Flags": [["flag", "maybe"], [True, True], [False, None], [True, False]]})


def leading_zero_identifiers(path: Path) -> list[str]:
    rows = [["code", "sku", "name"], ["007", "00-1", "Bond"], ["0012", "0A2", "Twelve"], ["100", "100", "Hundred"]]
    return _save(path, {"Ids": rows})


def mostly_text_numeric_column(path: Path) -> list[str]:
    values = [*range(1, 8), "n/a", "unknown", "x", "tbd"]
    return _save(path, {"Mixed": [["value"], *([value] for value in values)]})


def empty_cells_and_empty_row(path: Path) -> list[str]:
    return _save(path, {"Gaps": [["a", "b", "c"], [1, None, "x"], [None, None, None], [3, 4.5, None]]})


def numeric_first_row_has_no_header(path: Path) -> list[str]:
    return _save(path, {"Bare": [[1, 2, 3], [4, 5, 6], [7, 8, 9]]})


def header_only_sheet(path: Path) -> list[str]:
    return _save(path, {"Empty": [["alpha", "beta"]]})


def duplicate_header_names(path: Path) -> list[str]:
    return _save(path, {"Dupes": [["name", "name", "Name", "x"], ["a", "b", "c", 1], ["d", "e", "f", 2]]})


def apostrophe_and_non_ascii_sheet_names(path: Path) -> list[str]:
    return _save(path, {
        "O'Reilly": [["book"], ["Learning SQL"]],
        "Café Müller": [["city", "count"], ["Zürich", 3], ["São Paulo", 4]],
    })


def fifty_columns(path: Path) -> list[str]:
    header = [f"col{i}" for i in range(1, 51)]
    return _save(path, {"Wide": [header, list(range(1, 51)), [i / 2 for i in range(1, 51)]]})


def formula_like_text(path: Path) -> list[str]:
    workbook = Workbook()
    sheet = workbook.active
    sheet.title = "Formulas"
    sheet.append(["text", "other"])
    for row, (text, other) in enumerate([("=1+1", "plain"), ("+SUM(A1)", "@cmd"), ("-5x", "ok")], start=2):
        for column, value in enumerate((text, other), start=1):
            cell = sheet.cell(row=row, column=column)
            cell.value = value
            cell.data_type = "s"
    workbook.save(path)
    return ["Formulas"]


CASES = {
    "integers_and_floats": integers_and_floats,
    "styled_dates_datetimes_and_times": styled_dates_datetimes_and_times,
    "booleans": booleans,
    "leading_zero_identifiers": leading_zero_identifiers,
    "mostly_text_numeric_column": mostly_text_numeric_column,
    "empty_cells_and_empty_row": empty_cells_and_empty_row,
    "numeric_first_row_has_no_header": numeric_first_row_has_no_header,
    "header_only_sheet": header_only_sheet,
    "duplicate_header_names": duplicate_header_names,
    "apostrophe_and_non_ascii_sheet_names": apostrophe_and_non_ascii_sheet_names,
    "fifty_columns": fifty_columns,
    "formula_like_text": formula_like_text,
}
