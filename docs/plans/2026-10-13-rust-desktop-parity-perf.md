# Python and Rust route timings

- Date 2026-10-14, commit 6a928c7 (`rust-desktop/parity`)
- Machine: idle 4-core Linux x86_64 container (15 GB RAM), nothing else building or running. This is not the Mac the spec assumes. The caps are provisional and were set for the Mac, because the human has not yet confirmed them (H0), so a miss here is a signal, not a verdict.
- Rust binary: `cargo build --release -p quark-core --bin quark-dev-server` (release profile, warm target dir)
- `tests/perf/compare.py --python --rust`: 2,000,000 rows, pages of 100, median of 5 runs; the deep page is row 1,000,000; `n/a` means the route answers 501 or an export failed, or no export outlasted the timed page
- The export cases write 2M and 1,000,000 rows; the page-during-export case always exports 2M rows so the export outlasts the page
- Marks: "OVER CAP" means the Rust median is above the spec 9 cap; "SLOWER" means Rust is not faster than Python

## Route comparison

| Case | Spec 9 cap | Python (ms) | Rust (ms) | Rust vs cap | Rust vs Python |
|---|---|---|---|---|---|
| Stats on a numeric column, filter on | 300 ms | 1103.1 | 343.5 | OVER CAP (1.15x cap) | 3.2x faster |
| Category values, first page of a text column | 150 ms | 892.4 | 7.2 | ok (0.05x cap) | 123.9x faster |
| Bar chart | 500 ms | 860.5 | 6.3 | ok (0.01x cap) | 136.6x faster |
| Line chart | 500 ms | 920.7 | 9.9 | ok (0.02x cap) | 93.0x faster |
| Histogram chart | 500 ms | 1259.1 | 11.3 | ok (0.02x cap) | 111.4x faster |
| Metric chart | 500 ms | 406.6 | 11.7 | ok (0.02x cap) | 34.8x faster |
| Scatter chart | 800 ms | 1064.6 | 11.6 | ok (0.01x cap) | 91.8x faster |
| Box chart | 800 ms | 2010.4 | 6.5 | ok (0.01x cap) | 309.3x faster |
| Find Next after the first search on a View | 200 ms | 1065.2 | 54.5 | ok (0.27x cap) | 19.5x faster |
| Join preview, two sources, one-to-one key | 1 s | 2939.6 | 2188.0 | OVER CAP (2.19x cap) | 1.3x faster |
| CSV export, 2M rows | 5 s | 47926.1 | 10827.5 | OVER CAP (2.17x cap) | 4.4x faster |
| XLSX export, 1,000,000 rows | 15 s | 230282.6 | 15536.8 | OVER CAP (1.04x cap) | 14.8x faster |
| Workbook confirm, 100,000 rows by 10 columns | 3 s | 12619.4 | 4490.7 | OVER CAP (1.50x cap) | 2.8x faster |
| Page at row 1,000,000 while a CSV export runs | 60 ms | 51602.6 | 22.3 | ok (0.37x cap) | 2314.0x faster |
| page 1 | - | 954.5 | 11.1 | no cap | 86.0x faster |
| deep page | - | 1660.4 | 11.8 | no cap | 140.7x faster |
| page 1, qty > 10 | - | 1126.7 | 13.5 | no cap | 83.5x faster |
| deep page, qty > 10 | - | 2054.0 | 14.5 | no cap | 141.7x faster |
| page 1, sorted by price | - | 2086.5 | 12.0 | no cap | 173.9x faster |
| deep page, sorted by price | - | 2429.1 | 14.4 | no cap | 168.7x faster |

Cases over their cap: stats (343.5 ms vs 300 ms), join preview (2188.0 ms vs 1 s), CSV export (10.8 s vs 5 s), XLSX export (15.5 s vs 15 s, marginal) and workbook confirm (4.5 s vs 3 s).
Cases where Rust is not faster than Python: none. Rust is faster in every row; the closest are join preview (1.3x) and stats (3.2x).

## perf_acceptance (`cargo test --release -p quark-core --test perf_acceptance -- --ignored --nocapture`, 2,000,000 rows)

This commit's `perf_acceptance.rs` still holds only the spec 4.6 reshape case (`two_million_row_csv_meets_the_targets`); the per-row spec 9 cases from E1 are not in `rust-desktop/parity` yet, so the route caps above come from `compare.py` alone.

Result: FAILED (5 of 9 steps missed their cap; run took 11.4 s). Panic: `targets missed: ["first page, qty > 10 (live CSV, import running)", "after the import: page at offset 1,000,000, qty > 10", "sorted by price: page at offset 0", "sorted by price: page at offset 500000", "sorted by price: page at offset 1000000"]`.

| Step | ms | Cap | Result |
|---|---|---|---|
| first page, qty > 10 (live CSV, import running) | 2013.16 | <= 750 ms | MISSED (2.7x) |
| columnar import finished (background) | 3575.95 | recorded | - |
| after the import: page at offset 1,000,000, qty > 10 | 265.61 | <= 60 ms | MISSED (4.4x) |
| sort by price: first request | 161.16 | recorded | - |
| sort by price: second request (starts the result build) | 90.99 | recorded | - |
| sorted result ready (background) | 1055.95 | recorded | - |
| sorted by price: page at offset 0 | 10.02 | <= 5 ms | MISSED (2.0x) |
| sorted by price: page at offset 500000 | 6.68 | <= 5 ms | MISSED (1.3x) |
| sorted by price: page at offset 1000000 | 6.08 | <= 5 ms | MISSED (1.2x) |

The two qty > 10 misses are far over their caps. The three sorted-page misses are 1.2x to 2.0x over and may partly be this container; they need confirming on the Mac.
