# Workbook reading rules

What DuckDB's `read_xlsx` does and what the Python backend (`_xlsx_table` in `backend/app.py`) turns it into.
W1 builds the Rust reader against this. The corpus in `tests/workbook_corpus.py` pins the second half
(`tests/test_workbook_corpus.py`, Python mode green, Rust mode xfail until W1/W3). The first half was observed
with a throwaway probe against DuckDB 1.5.4 with the `excel` extension; the probe is not committed.

## 1. `read_xlsx` itself

Every cell is read as a number, a string, or empty. Dates, datetimes and times are numbers (Excel serials)
whose cell style says "date"; booleans are numbers 0/1 with a boolean cell type.

**Header detection.** The first non-empty row is the header only when every cell in it is a non-empty string.
- `["a","b"]` over `["x","y"]`: header, columns `a`, `b`.
- `["a",1]`, `[1,"t"]`, a numeric row `[1,2,3]`, a date row, or `["a",<empty>,"c"]`: no header. The columns are
  named after the cell references of that row, `A1`, `B1`, `C1`, and the first row stays data.
- Leading empty rows are skipped. If the first non-empty row is row 2 and is not a header the names are `A2`, `B2`.

**Duplicate header names.** The first keeps its name, later ones get `_<n>` where `n` is the 0-based column position of the duplicate:
`["name","name","Name","x"]` gives `name`, `name_1`, `Name_2`, `x`. The comparison is case-insensitive
and the original spelling is kept.

**Empty rows and cells.**
- Empty cells are NULL.
- By default reading stops at the first fully empty row: `[a],[1],<empty>,[3]` yields one row. Rows after the gap are
  dropped, with no error. (`stop_at_empty = false` would keep them as a NULL row and `3`; the Python backend does
  not pass it, so Rust must not either.)
- A header-only sheet has zero rows. Without `all_varchar` its columns are `DOUBLE`; with it they are `VARCHAR`.
- A column with no data cells under a header keeps its header name.

**Text under `all_varchar = true`.** Every column is `VARCHAR`. The text of each cell kind:

| Cell | Text |
|---|---|
| integer `1`, `-3` | `1`, `-3` (no `.0`) |
| float `0.5`, `1e6` | `0.5`, `1000000` (shortest form, no exponent for whole values this size) |
| float beyond 15 digits | exponent form, `12345678901234567890` becomes `1.234567890123457e+19` |
| date | the serial, `2024-01-02` is `45293` |
| datetime | serial with fraction, `2024-01-02 12:00` is `45293.5`, `2025-12-31 23:59:59` is `46022.99998842592` |
| time | fraction of a day, `09:30:15` is `0.3960069444444445`, midnight is `0` |
| boolean | `1` / `0` |
| string | unchanged, including `=1+1`, `+SUM(A1)`, `-5x`, `007`, `1e3`, `0x10` (never evaluated or coerced) |
| empty | NULL |

**Declared types without `all_varchar`.** Python reads `SELECT * ... LIMIT 0` and takes the types from
`.description`, which follow the first data row of each column (W1's `declare()` does the same):
- numbers: `DOUBLE` (integers are not distinguished from floats);
- date-styled numbers: `DATE`; datetime-styled numbers: `TIMESTAMP`; time-styled numbers: `TIME`;
- booleans: `BOOLEAN`;
- strings: `VARCHAR`, including `007` and `0012` stored as text (the value keeps its zeros).

A later cell of another kind does not change the declared type. A full typed read of such a column fails
(`Could not convert string 'n/a' to DOUBLE` when a string appears after numbers), so the Python backend
never relies on a typed read for values.

What decides date, datetime or time is the cell style (number format), not the value. Probed with
`45293.5` (2024-01-02 12:00): format `General` gives `DOUBLE` (`45293.5`); `yyyy-mm-dd` gives `DATE`
(the fraction is dropped: `2024-01-02`); `yyyy-mm-dd hh:mm` gives `TIMESTAMP`. Under `all_varchar` the same cells
read `45293.5`, `45293.5`, `45293.5`, so the serial text keeps the fraction and the Python backend recovers it (rule 1 below).

## 2. What the Python backend does with it (`_xlsx_table`)

It reads the sheet twice: typed only for `description` (the declared types), and with `all_varchar = true` for
the values. For each column:
1. declared `DATE` with any fractional serial in the column becomes a `TIMESTAMP` (`1899-12-30 + serial days`);
2. declared `DATE` becomes a `DATE`; declared `TIMESTAMP` becomes a `TIMESTAMP`; declared `TIME` becomes a `TIME`
   (all by adding the serial to `1899-12-30`);
3. otherwise, if at least 90% of the non-empty, non-blank texts cast to `DOUBLE`, the column is `DOUBLE`
   (`TRY_CAST`; texts that fail become NULL);
4. otherwise, if at least 90% cast to `TIMESTAMP` and some text has a time part, `TIMESTAMP`;
5. otherwise, if at least 90% cast to `DATE`, `DATE`;
6. otherwise the column stays `VARCHAR`.

Consequences the corpus pins:
- A declared `BOOLEAN` is not consulted: `1`/`0` pass rule 3 and the column is `DOUBLE` (`1.0` / `0.0`).
- Text identifiers that are all digits (`007`, `0012`, `100`) pass rule 3 and become `DOUBLE`: leading zeros are
  lost. A text column survives only when under 90% of its values are numeric (`00-1`, `0A2`, `100`: `VARCHAR`).
- A column that is 7 numbers and 4 words (64% numeric) stays `VARCHAR` and keeps `1`..`7` as text.
- A header-only sheet has no values to cast, so all columns are `VARCHAR`.
- Integers and floats are both `DOUBLE`; the `/query` JSON shows `1.0`, never `1`.
- Datetime `2024-01-02 12:00` comes back as `2024-01-02T12:00:00`; the serial arithmetic is exact for fractions
  that are binary-friendly (`.5`, `.75`) and sub-second drift can appear otherwise, so the corpus avoids such values.
- The sheet becomes a table named after the sheet, quoted verbatim, so `O'Reilly` and `Café Müller` work.
- Formula-like text is data, not a formula, and is returned unchanged.

## 3. Out of the corpus

Formulas with cached values: `openpyxl` writes formulas without a cached result, so `read_xlsx` would see an
empty value. Files that carry cached results need a real spreadsheet application to produce; W1's own tests may
build them with `rust_xlsxwriter` (which can set a result), not this corpus.

## 4. Known gaps

calamine 0.36 exposes no number formats, so the Rust reader decides date, datetime or time from the value.
Two cases differ from `read_xlsx` (W4 records them as deviations):
- a datetime-styled column whose first value is at midnight is declared `DATE` (`read_xlsx` says `TIMESTAMP`);
- a time-styled value of 1 or more (a duration) is declared `TIMESTAMP` (`read_xlsx` says `TIME`).
