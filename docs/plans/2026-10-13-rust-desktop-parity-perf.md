# Python and Rust route timings

- Date 2026-10-14, commit 6cb43cb (`rust-desktop/parity`), exit performance run (E1)
- Machine: a 4-core Linux x86_64 container (15 GB RAM), not the Mac the spec assumes. The caps are provisional and were set for the Mac, because the human has not yet confirmed them (H0), so a miss here is a signal, not a verdict. "Idle" means nothing else building or running. "Under load" means two busy loops (`while :; do :; done`) on 2 of the 4 cores.
- Rust binary: `cargo build --release -p quark-core --bin quark-dev-server` (release profile, warm target dir)
- Earlier per-route numbers are replaced: since L10 every case times a request the response cache has not seen, so the earlier figures (which included cache hits) are not comparable.

## Route comparison

`tests/perf/compare.py --python --rust --rust-binary .../quark-dev-server`, idle machine. Medians in ms.

<!-- compare.py:table:start -->
- Date 2026-10-14, commit 6cb43cb, Linux x86_64 with 4 cores
- 2,000,000 rows, pages of 100, median of 5 runs, every timed request new to its backend; the deep page is row 1,000,000 in a full run and mid-file in a smaller one; `n/a` means the route answers 501 or an export failed, or no export outlasted the timed page
- The export cases write 2M and 1,000,000 rows in a full run and every row of a smaller one; the page-during-export case always exports 2M rows so the export outlasts the page

| Case | Spec 9 cap | Python (ms) | Rust (ms) |
|---|---|---|---|
| Stats on a numeric column, filter on | 300 ms | 1255.7 | 309.2 |
| Category values, first page of a text column | 150 ms | 863.3 | 17.5 |
| Bar chart | 500 ms | 802.0 | 22.7 |
| Line chart | 500 ms | 1055.9 | 101.1 |
| Histogram chart | 500 ms | 1209.8 | 355.0 |
| Metric chart | 500 ms | 448.3 | 13.7 |
| Scatter chart | 800 ms | 937.2 | 71.3 |
| Box chart | 800 ms | 2156.9 | 307.9 |
| Find Next after the first search on a View | 200 ms | 1242.6 | 55.0 |
| Join preview, two sources, one-to-one key | 1 s | 2802.2 | 289.6 |
| CSV export, 2M rows | 5 s | 50320.8 | 10389.0 |
| XLSX export, 1,000,000 rows | 15 s | 238101.8 | 15568.0 |
| Workbook confirm, 100,000 rows by 10 columns | 3 s | 12228.8 | 4401.4 |
| Page at row 1,000,000 while a CSV export runs | 60 ms | 47498.9 | 14.6 |
| page 1 | - | 1036.0 | 9.6 |
| deep page | - | 1637.9 | 11.8 |
| page 1, qty > 10 | - | 1021.1 | 13.8 |
| deep page, qty > 10 | - | 1639.3 | 8.6 |
| page 1, sorted by price | - | 1882.1 | 14.2 |
| deep page, sorted by price | - | 2452.2 | 17.3 |
<!-- compare.py:table:end -->

Rust is faster than Python in every row (smallest ratios: workbook confirm 2.8x, stats 4.1x, CSV export 4.8x).

## perf_acceptance, idle and under load

Command: `CARGO_TARGET_DIR=/home/user/quark-wt/release-target cargo test --release -p quark-core --test perf_acceptance -- --ignored --nocapture --test-threads=1`, 2,000,000 rows (254 MB). Each figure is the median the test reports. Margin is measured/cap, so above 1.00x is a miss. Idle run: 116.8 s, 6 passed, 5 failed. Under load: 117.6 s, 6 passed, 5 failed.

| Case | Cap (ms) | Idle (ms) | Idle margin | Idle | Load (ms) | Load margin | Load |
|---|---|---|---|---|---|---|---|
| Bar chart | 500 | 23.2 | 0.05x | pass | 32.1 | 0.06x | pass |
| Line chart | 500 | 145.9 | 0.29x | pass | 199.4 | 0.40x | pass |
| Histogram chart | 500 | 403.4 | 0.81x | pass | 433.2 | 0.87x | pass |
| Metric chart | 500 | 15.3 | 0.03x | pass | 13.7 | 0.03x | pass |
| Scatter chart | 800 | 94.0 | 0.12x | pass | 115.4 | 0.14x | pass |
| Box chart | 800 | 371.0 | 0.46x | pass | 589.4 | 0.74x | pass |
| Category values, first page | 150 | 21.5 | 0.14x | pass | 25.5 | 0.17x | pass |
| Find Next after the first search | 200 | 51.9 | 0.26x | pass | 88.1 | 0.44x | pass |
| Join preview, one-to-one key | 1000 | 317.5 | 0.32x | pass | 545.7 | 0.55x | pass |
| Stats, numeric column, filter on | 300 | 338.1 | 1.13x | MISS | 433.3 | 1.44x | MISS |
| CSV export, 2M rows | 5000 | 11253.3 | 2.25x | MISS | 9325.7 | 1.87x | MISS |
| XLSX export, 1,000,000 rows | 15000 | 15568.5 | 1.04x | MISS | 16423.0 | 1.09x | MISS |
| Workbook confirm, 100,000 x 10 | 3000 | 4693.1 | 1.56x | MISS | 5442.2 | 1.81x | MISS |
| Page at row 1,000,000 during export | 60 | 10.1 | 0.17x | pass | 20.8 | 0.35x | pass |

Spec 4.6 reshape case (`two_million_row_csv_meets_the_targets`): FAILED in both runs, 5 of 9 steps missed.

| Step | Cap | Idle (ms) | Load (ms) | Result |
|---|---|---|---|---|
| first page, qty > 10 (live CSV, import running) | <= 750 ms | 2200.08 | 2971.24 | MISSED (idle 2.9x, load 4.0x) |
| columnar import finished (background) | recorded | 3372.17 | 2938.26 | - |
| after the import: page at offset 1,000,000, qty > 10 | <= 60 ms | 289.69 | 304.82 | MISSED (idle 4.8x, load 5.1x) |
| sort by price: first request | recorded | 159.12 | 251.45 | - |
| sort by price: second request (starts the result build) | recorded | 85.34 | 103.55 | - |
| sorted result ready (background) | recorded | 1037.57 | 1312.26 | - |
| sorted by price: page at offset 0 | <= 5 ms | 12.07 | 10.11 | MISSED (idle 2.4x, load 2.0x) |
| sorted by price: page at offset 500000 | <= 5 ms | 9.96 | 22.25 | MISSED (idle 2.0x, load 4.5x) |
| sorted by price: page at offset 1000000 | <= 5 ms | 9.18 | 12.84 | MISSED (idle 1.8x, load 2.6x) |

## Cases over their cap

Per-route cases (`perf_acceptance`), idle and under load:

- Stats, numeric column, filter on: 338.1 ms idle (1.13x), 433.3 ms loaded (1.44x), cap 300 ms. `compare.py` agrees (309.2 ms, 1.03x).
- CSV export, 2M rows: 11.3 s idle (2.25x), 9.3 s loaded (1.87x), cap 5 s. `compare.py`: 10.4 s. The loaded run was faster than the idle one, which was not investigated.
- XLSX export, 1,000,000 rows: 15.6 s idle (1.04x), 16.4 s loaded (1.09x), cap 15 s. `compare.py`: 15.6 s. Marginal when idle.
- Workbook confirm, 100,000 x 10: 4.7 s idle (1.56x), 5.4 s loaded (1.81x), cap 3 s. `compare.py`: 4.4 s.

Spec 4.6 steps (idle and loaded): first page with qty > 10 while the import runs (2.2 s against 750 ms), page at row 1,000,000 with qty > 10 after the import (290 ms against 60 ms), and the three sorted-by-price pages (9 to 22 ms against 5 ms).

Near misses on the margin: histogram chart passes with 0.81x idle and 0.87x loaded, and box chart with 0.74x loaded.

Cases where Rust is not faster than Python: none, in the `compare.py` table.

Page at row 1,000,000 during export passes (10.1 ms idle, 20.8 ms loaded, cap 60 ms); the earlier run's 115.3 ms miss is superseded, since L10 changed the method.
