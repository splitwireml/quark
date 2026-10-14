# Python and Rust route timings

- Date 2026-10-14, commit bc9eeb8
- Machine: 4-core Linux x86_64 cloud container, 15 GB RAM, idle during the run. This is not the Mac the spec assumes, so the caps are provisional until the human confirms them (H0).
- Release build (`cargo build --release -p quark-core --bin quark-dev-server`, fat LTO, DuckDB compiled in release mode): 22m 25s wall, cold target dir
- 2,000,000 rows, pages of 100, median of 5 runs; the deep page is row 1,000,000 in a full run and mid-file in a smaller one; `n/a` means the route answers 501 or an export failed, or no export outlasted the timed page
- The export cases write 2M and 1,000,000 rows in a full run and every row of a smaller one; the page-during-export case always exports 2M rows so the export outlasts the page

| Case | Spec 9 cap | Python (ms) | Rust (ms) |
|---|---|---|---|
| Stats on a numeric column, filter on | 300 ms | 1500.7 | n/a |
| Category values, first page of a text column | 150 ms | 789.1 | n/a |
| Bar chart | 500 ms | 731.4 | n/a |
| Line chart | 500 ms | 738.1 | n/a |
| Histogram chart | 500 ms | 1077.7 | n/a |
| Metric chart | 500 ms | 362.8 | n/a |
| Scatter chart | 800 ms | 757.7 | n/a |
| Box chart | 800 ms | 1786.4 | n/a |
| Find Next after the first search on a View | 200 ms | 1004.4 | n/a |
| Join preview, two sources, one-to-one key | 1 s | 2482.8 | n/a |
| CSV export, 2M rows | 5 s | 42327.3 | n/a |
| XLSX export, 1,000,000 rows | 15 s | 208591.9 | n/a |
| Workbook confirm, 100,000 rows by 10 columns | 3 s | 11529.0 | n/a |
| Page at row 1,000,000 while a CSV export runs | 60 ms | n/a | n/a |
| page 1 | - | 884.5 | 9.2 |
| deep page | - | 1440.1 | 11.1 |
| page 1, qty > 10 | - | 820.8 | 14.7 |
| deep page, qty > 10 | - | 1557.5 | 8.9 |
| page 1, sorted by price | - | 1805.7 | 10.8 |
| deep page, sorted by price | - | 2347.9 | 8.0 |

## Spec 4.6 reshape check (`perf_acceptance`, release, 2,000,000 rows)

Result: FAILED (4 of 9 steps missed their cap; run took 10.2 s). Panic: `targets missed: ["first page, qty > 10 (live CSV, import running)", "after the import: page at offset 1,000,000, qty > 10", "sorted by price: page at offset 0", "sorted by price: page at offset 1000000"]`.

| Step | ms | Target |
|---|---|---|
| first page, qty > 10 (live CSV, import running) | 1868.27 | <= 750 ms MISSED |
| columnar import finished (background) | 3066.75 | recorded |
| after the import: page at offset 1,000,000, qty > 10 | 217.52 | <= 60 ms MISSED |
| sort by price: first request | 137.63 | recorded |
| sort by price: second request (starts the result build) | 67.87 | recorded |
| sorted result ready (background) | 835.70 | recorded |
| sorted by price: page at offset 0 | 6.15 | <= 5 ms MISSED |
| sorted by price: page at offset 500000 | 4.93 | <= 5 ms ok |
| sorted by price: page at offset 1000000 | 5.05 | <= 5 ms MISSED |

The two sorted-page misses are within 25% of the cap and may just be this machine; the two qty > 10 misses are far over.
