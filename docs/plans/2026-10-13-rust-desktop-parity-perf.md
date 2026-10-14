# Python and Rust route timings

> Smoke run: 20,000 rows on `target/debug/quark-dev-server`, not the 2,000,000-row release baseline.

- Date 2026-10-14, commit dc0fe00, Linux x86_64 with 4 cores
- 20,000 rows, pages of 100, median of 5 runs; the deep page is row 1,000,000 in a full run and mid-file in a smaller one; `n/a` means the route answers 501 or an export failed, or no export outlasted the timed page
- The export cases write 2M and 1,000,000 rows in a full run and every row of a smaller one; the page-during-export case always exports 2M rows so the export outlasts the page

| Case | Spec 9 cap | Python (ms) | Rust (ms) |
|---|---|---|---|
| Stats on a numeric column, filter on | 300 ms | 238.7 | n/a |
| Category values, first page of a text column | 150 ms | 201.3 | n/a |
| Bar chart | 500 ms | 194.7 | n/a |
| Line chart | 500 ms | 198.6 | n/a |
| Histogram chart | 500 ms | 194.6 | n/a |
| Metric chart | 500 ms | 124.7 | n/a |
| Scatter chart | 800 ms | 241.5 | n/a |
| Box chart | 800 ms | 290.3 | n/a |
| Find Next after the first search on a View | 200 ms | 130.1 | n/a |
| Join preview, two sources, one-to-one key | 1 s | 679.6 | n/a |
| CSV export, 2M rows | 5 s | 546.7 | n/a |
| XLSX export, 1,000,000 rows | 15 s | 4183.7 | n/a |
| Workbook confirm, 100,000 rows by 10 columns | 3 s | 11316.8 | n/a |
| Page at row 1,000,000 while a CSV export runs | 60 ms | 41953.4 | n/a |
| page 1 | - | 262.5 | 10.0 |
| deep page | - | 262.6 | 10.8 |
| page 1, qty > 10 | - | 292.0 | 10.0 |
| deep page, qty > 10 | - | 293.4 | 7.5 |
| page 1, sorted by price | - | 279.1 | 8.6 |
| deep page, sorted by price | - | 287.5 | 8.4 |
