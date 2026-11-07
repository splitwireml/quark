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

Rust is faster than Python in every row (smallest speedups: workbook confirm 2.8x, histogram 3.4x, stats 4.1x).

## Spec 9 summary

One row per spec 9 case. The caps are provisional (H0). Rust idle and Rust under load are the `perf_acceptance` medians below; Python and the ratio come from the `compare.py` table above (the ratio is Rust over Python, so below 1 means Rust is faster). The verdict compares the idle `perf_acceptance` median with the cap. The three export rows (CSV, XLSX, page during export) were measured again idle on 5aa4182 (L12, streaming); every other figure is from the exit run on e8f76df. The under-load figures are all from e8f76df, so the XLSX row now passes idle and misses under load. Python was not run under load.

| Case | Cap | Rust idle (ms) | Rust load (ms) | Python (ms) | Rust/Python | Verdict |
|---|---|---|---|---|---|---|
| Stats, numeric column, filter on | 300 ms | 338.1 | 433.3 | 1255.7 | 0.25 | MISS |
| Category values, first page | 150 ms | 21.5 | 25.5 | 863.3 | 0.02 | within cap |
| Bar chart | 500 ms | 23.2 | 32.1 | 802.0 | 0.03 | within cap |
| Line chart | 500 ms | 145.9 | 199.4 | 1055.9 | 0.10 | within cap |
| Histogram chart | 500 ms | 403.4 | 433.2 | 1209.8 | 0.29 | within cap |
| Metric chart | 500 ms | 15.3 | 13.7 | 448.3 | 0.03 | within cap |
| Scatter chart | 800 ms | 94.0 | 115.4 | 937.2 | 0.08 | within cap |
| Box chart | 800 ms | 371.0 | 589.4 | 2156.9 | 0.14 | within cap |
| Find Next after the first search | 200 ms | 51.9 | 88.1 | 1242.6 | 0.04 | within cap |
| Join preview, one-to-one key | 1 s | 317.5 | 545.7 | 2802.2 | 0.10 | within cap |
| CSV export, 2M rows | 5 s | 9294.1 | 9325.7 | 50320.8 | 0.21 | MISS |
| XLSX export, 1,000,000 rows | 15 s | 14898.4 | 16423.0 | 238101.8 | 0.07 | within cap idle, MISS under load |
| Workbook confirm, 100,000 x 10 | 3 s | 4693.1 | 5442.2 | 12228.8 | 0.36 | MISS |
| Page at row 1,000,000 during a CSV export | 60 ms | 9.2 | 20.8 | 47498.9 | 0.0003 | within cap |

Three of 14 cases miss their cap idle (stats, CSV export, workbook confirm; four under load, with XLSX export), and 5 of the 9 spec 4.6 reshape steps miss (below). Python's page during the export waits behind the export, which is a Python limit, not a measurement fault.

## perf_acceptance, idle and under load

Command: `CARGO_TARGET_DIR=/home/user/quark-wt/release-target cargo test --release -p quark-core --test perf_acceptance -- --ignored --nocapture --test-threads=1`, 2,000,000 rows (254 MB). Each figure is the median the test reports. Margin is measured/cap, so above 1.00x is a miss. Idle run: 116.8 s, 6 passed, 5 failed. Under load: 117.6 s, 6 passed, 5 failed. Both runs were on e8f76df. After L12 the three export rows were run again idle on 5aa4182 (see "after L12 (streaming)"); their idle columns below are the new figures, and their load columns are still e8f76df's.

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
| CSV export, 2M rows | 5000 | 9294.1 | 1.86x | MISS | 9325.7 | 1.87x | MISS |
| XLSX export, 1,000,000 rows | 15000 | 14898.4 | 0.99x | pass | 16423.0 | 1.09x | MISS |
| Workbook confirm, 100,000 x 10 | 3000 | 4693.1 | 1.56x | MISS | 5442.2 | 1.81x | MISS |
| Page at row 1,000,000 during export | 60 | 9.2 | 0.15x | pass | 20.8 | 0.35x | pass |

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
- CSV export, 2M rows: 9.3 s idle on 5aa4182 (1.86x; 11.3 s, 2.25x, before L12), 9.3 s loaded on e8f76df (1.87x), cap 5 s. `compare.py` (e8f76df): 10.4 s. Streaming did not bring it near the cap.
- XLSX export, 1,000,000 rows: 14.9 s idle on 5aa4182 (0.99x, a pass by 0.1 s; 15.6 s, 1.04x, before L12), 16.4 s loaded on e8f76df (1.09x), cap 15 s. `compare.py` (e8f76df): 15.6 s. Too close to the cap to call it fixed; it was not re-run under load.
- Workbook confirm, 100,000 x 10: 4.7 s idle (1.56x), 5.4 s loaded (1.81x), cap 3 s. `compare.py`: 4.4 s.

Spec 4.6 steps (idle and loaded): first page with qty > 10 while the import runs (2.2 s against 750 ms), page at row 1,000,000 with qty > 10 after the import (290 ms against 60 ms), and the three sorted-by-price pages (9 to 22 ms against 5 ms).

Near misses on the margin: histogram chart passes with 0.81x idle and 0.87x loaded, and box chart with 0.74x loaded.

Cases where Rust is not faster than Python: none, in the `compare.py` table.

Page at row 1,000,000 during export passes (9.2 ms idle on 5aa4182, 10.1 ms before L12; 20.8 ms loaded on e8f76df; cap 60 ms); the earlier run's 115.3 ms miss is superseded, since L10 changed the method.

## after L12 (streaming)

L12 (`5aa4182`, `perf(core): stream export rows instead of materialising them`) reads export rows with duckdb-rs `stream_arrow`, one batch at a time, instead of `query_arrow`. Measured on 2026-10-14 on the same 4-core container, idle (no other build or run), release `quark-dev-server`, 2,000,000-row CSV table (254 MB).

Time, `perf_acceptance` median (`-- csv_export_of_two_million xlsx_export_of_one_million page_during_an_export --ignored --nocapture --test-threads=1`, 64.1 s, 2 passed, 1 failed):

| Case | Cap (ms) | e8f76df idle (ms) | 5aa4182 idle (ms) | Margin now | Result |
|---|---|---|---|---|---|
| CSV export, 2M rows | 5000 | 11253.3 | 9294.1 | 1.86x | MISS |
| XLSX export, 1,000,000 rows | 15000 | 15568.5 | 14898.4 | 0.99x | pass |
| Page at row 1,000,000 during export | 60 | 10.1 | 9.2 | 0.15x | pass |

Peak memory, VmHWM of the server process (`/proc/<pid>/status`). Each case used a fresh server: upload the CSV, wait for the columnar import to finish, write `5` to `/proc/<pid>/clear_refs` (this resets VmHWM to the current RSS), run one export through `POST /api/exports` (body as in `compare.py`: `SELECT * FROM <table> LIMIT n`), read VmHWM. A 50 ms RSS sampler agreed with VmHWM. One run per cell. Before is a release build of e8f76df, after is 5aa4182, same `CARGO_TARGET_DIR`.

| Export | RSS before the export (MB) | Peak before L12 (MB) | Peak after L12 (MB) | Change |
|---|---|---|---|---|
| CSV, 2,000,000 rows (256.4 MB written) | 54 before, 137 after | 470 | 141 | -329 MB, 0.30x |
| XLSX, 1,000,000 rows (82.8 MB written) | 131 before, 137 after | 497 | 355 | -142 MB, 0.71x |

Over the whole process lifetime the peak is the same in both builds, because the import of the 2M-row CSV alone takes VmHWM to 808 to 855 MB before any export. The figures above are the export's own peak. The XLSX writer still holds its workbook while it writes, so its peak falls less than the CSV's. The earlier debug-server figures (1.10 GB before L12, 0.74 GB with it, recorded by the Lead unit) are not comparable: they were a debug build and the whole process.

Time did not change much: the CSV export is 17% faster and the XLSX export 4% faster than before, and the CSV export still misses its 5 s cap by 1.86x. Streaming is a memory fix, not a speed fix. The `compare.py` table above and the under-load columns were not re-run, so they still describe e8f76df.
