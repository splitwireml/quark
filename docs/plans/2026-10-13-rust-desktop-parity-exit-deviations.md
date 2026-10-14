# Quark desktop parity: exit-gate deviations

Design section 12 item 7 says every deviation from the spec (`2026-10-08-rust-desktop-parity-design.md`) needs a human sign-off. This page lists them, in SP1's format (`2026-10-04-skeleton-exit-deviations.md`; "SP1 #n" below means its item n). Sprint managers audited each sprint, and three fresh reviewers walked spec sections 1 to 14 against `rust-desktop/parity` at e8f76df (59 lines, no code blocker). Real bugs and missing tests went to Lead units L1 to L14 (see "Fixed at the exit gate"). Plan section 17 holds the amendments. The performance figures are in `2026-10-13-rust-desktop-parity-perf.md`; H2 is `docs/handoffs/PARITY_H2_CHECKLIST.md`.

Mark each item **accept** or **fix** in review.

## Sign-off

- **Date:** (blank)
- **Status:** awaiting sign-off
- **Accepted:** (blank)
- **Fix now:** (blank)
- **Moved to sub-project 3 or later:** (blank)

## Python-parity choices (made during the run)

| # | Spec or card says | Code does | Why |
|---|---|---|---|
| 1 | §6.2 values as Python's `export_value` | A HUGEINT nested in a list reads as a DECIMAL(38,0) string (Python: an int) | Same limit as SP1's `cell_json`; rare |
| 2 | §6.1 line: a TIME x column is not a date | It passes the date check, and `date_trunc` has no TIME overload: 422 `Invalid filter value: Binder Error ...` (Python: 500, `time - time` raises) | Matches Python's DuckDB-error rule; no TIME line chart is drawn in either backend (the "TIME line span", C8) |
| 3 | §6.2 sheet names unique, case-insensitive as Python's `casefold` | `to_lowercase`, so `Straße` and `STRASSE` stay distinct | Differs for a few letters only. Find and values use `lower()` on both sides in SQL, as Python does, so they have no gap |
| 4 | §6.2 Parquet and JSON through Python values | Arrow appender: INTERVAL months kept, TIMESTAMP_NS keeps nanoseconds, DATE infinity handled, UNION gives 422. HUGEINT, UHUGEINT, BIT, ENUM, VARINT and TIMETZ travel as text and are cast back (X5) | The appender changes or refuses these types without a text step. Types are listed in `features/exports/copy.rs` |
| 5 | §6.2 JSON as Python | NaN inside a LIST or STRUCT is written as NaN (Python: null). A top-level NaN is NaN in both | Edge of X5; not fixed |
| 6 | §6.3 types as `read_xlsx` (W2/F8) | calamine exposes no number format: a midnight datetime column is declared Date, and a time-styled value of 1 or more is declared Timestamp. See `2026-10-13-xlsx-read-rules.md`, "Known gaps" | Closing it needs `styles.xml` parsing with new direct dependencies |
| 7 | §6.3 corpus covers cached formula results | The dual-backend corpus has no such case (openpyxl cannot write cached values). Only a Rust unit test covers it, without a `read_xlsx` comparison | Test scope; no visible effect |
| 8 | §6 histogram bins as Python | The last bin's upper bound is `100.0` where Python prints `100` (V3); ntile bins with tied boundary values can bucket a series row differently from the main counts, for huge-integer columns (C5) | Values equal after parsing; rare |
| 9 | §6.1 labels as Python `str()` | List items of date, decimal or timestamp type use `str()` where Python uses `repr()`; non-ASCII non-printable characters are not escaped; a group literally named "Other" gives two series where Python merges them (L7 edges) | Rare; not fixed |
| 10 | §6.1 bar and pie grouped: one ranking and one grid query | Three queries (bars with a windowed total, series ranking, one grid with "Other" folded in SQL); Python ran five | Fewer queries, same output, pinned by `groups/tests.rs` |
| 11 | §6 stats: counts from the count and null cache when present | Not used; the profile scan computes counts in the same pass. Categorical runs 2 scans and date 3 (Python's), only numeric folds into one | Efficiency upgrade not taken; no visible effect |
| 12 | Approved with the spec (§7): unreadable workbook | An `.xlsx` that is not a workbook, or has no sheets, answers 400 `Could not open source: <reason>` and the file is deleted (Python: plain-text 500, W3). The three other §7 fixes (repeatable scatter sample, histogram series for every bin type, four join workspaces) are approved the same way | Pinned by `rust_only` tests. If rejected, Rust answers 500 `Internal error` |

## Error texts outside plan section 2's table

| # | Situation | Response |
|---|---|---|
| 13 | Category values with a bad `offset` or `limit` | 422 `Invalid offset or limit` (FastAPI answers its validation list); same convention as SP1 #5 |
| 14 | Export with 0 or more than 100 sheets | 422 `Export requires 1 to 100 sheets` (FastAPI: validation list); same convention |
| 15 | Request bodies that pydantic coerces (`"2"` as a page, `"yes"` as a bool) | 422; pydantic's lax coercion is not mirrored (as SP1's `QueryRequest`); the frontend never sends them |
| 16 | Wrong method on a known path | 405 `Method Not Allowed`; axum's `Allow` header lists HEAD beside GET, FastAPI's omits HEAD |
| 17 | Export that fails after the first rows were sent (L12, `2001a31`, not yet on `rust-desktop/parity`; see "Known limits") | 422 `Invalid SQL query: the result stopped before its last row`. duckdb-rs `stream_arrow` ends a failed stream as if it had finished, with no error to read, so L12 adds a closing flag row (`UNION ALL BY NAME`) and answers this text when that row never arrives. DuckDB's own message is lost where Python's export answers `Invalid SQL query: <DuckDB error>`. Errors found before the first row keep the exact text |

## Spec wording that differs from the code

| # | Spec says | Code does |
|---|---|---|
| 18 | §4.2 SQL lives in each feature's `queries.rs` | Charts, exports and find have none: per-kind chart files, `exports/copy.rs`, `exports/csv.rs` and `find/service.rs` hold their SQL beside their rules. `joins/service.rs` and `pages/service.rs` also build SQL text |
| 19 | §4.3 rule 3: a handler calls one service function | The exports handler validates, looks up each engine, writes and streams. The upload handler holds the multipart loop. `preview_join` calls `validate` then `preview`; values and charts call `validate()` first; source and node deletes call `state.delete_source` |
| 20 | §4.5 `Project` extractor gives the handler its project | No route uses it. Project routes take `Path(project_id)` and the service looks it up. The 404 text is the same |
| 21 | §4.3 rule 6: handler names match the action | `dataset_chart` and `sql_chart` (not `visualize`), `preview_join` (Python: `create_join_workspace`), `dataset_stats`, `dataset_values` |
| 22 | §4.2 layout: `runtime/` has uploads and workers; each feature has `model.rs` | Uploads live in `features/sources/uploads.rs` and `runtime/files.rs`; the columnar worker is in `runtime/state.rs`; values has no `model.rs` (`SqlBody` is in `routes.rs`). `lib.rs` also has `pub use http::router`. 13 files are over the 200-line aim (largest 244); all are under 250 |
| 23 | §4.4 the architecture test fails on a bad import | It reads `use` statements only: inline `crate::<layer>::` paths, `super::super::` hops and direction inside a feature are not checked. A grep finds no violation |
| 24 | §5.3 the find table follows the result cache | `NumberedCache` is a separate pool with its own LRU of 4, so an engine can hold 4 numbered and 4 result tables |
| 25 | §6.3 node engines attach the workbook cache read-only | `open_node` copies every sheet into an in-memory table, then detaches, as Python held its rows. Project workspaces attach read-only views |
| 26 | §6 joins: row counts from the count cache | Only same-engine dataset sides can reuse a page's count; SQL sides key differently and cross-source engines are new each time. They compute the count |
| 27 | §6 sources: `expanduser().resolve()` | `~user` is not expanded; a dangling symlink resolves to the link; the `\\?\` prefix strip is untested on Windows |
| 28 | §8.5 the pass runs against the built frontend, from `frontend/tests/*-browser.mjs` | It runs through the Vite dev server, from `frontend/tests/live/`. WebKit runs only in CI (the `live` job); locally there is only Chromium |
| 29 | §9 the comparison script sits beside `perf_acceptance` | `perf_acceptance` is a folder (`main.rs`, `routes.rs`); `compare.py` is `tests/perf/compare.py` |
| 30 | §8.2 services are tested without a server | Workbook services (`confirm_workbook`, `discard_workbook`) are tested through the router only |
| 31 | §5.2, §8.3 exports and join previews run to completion; a mount and a read never interleave | No test drops the client mid-export or mid-preview, and none races a mount against a read. The design stands in: a remount builds a new engine, and the neighbouring tests cover opens against the catalog lock |
| 32 | §5.5 join allow-lists | The exact files read, plus `.wal`, not their parent folders (J2, card-sanctioned) |
| 33 | §2 the catalog lock is never held while a query runs (L3) | Engines open outside the lock. After 8 attempts in a row lose to catalog changes, the open runs under the lock, so the call always ends |
| 34 | §5.4 ICU through `bundled-cmake` from crates.io | `duckdb` comes from the duckdb-rs git tag v1.10504.0 (DuckDB v1.5.4 submodule), because the crates.io package ships no sources. `.cargo/config.toml` plus `cmake/duckdb-version.cmake` pin the reported version. Costs: dev build +14%, release dev-server +16%. Bundle runs proved universal macOS and Windows at 6d0e5da and 7708704; none has run since, so the exit bundle run is still to come |
| 35 | §10 dev 24: self-test log message | The log folder is created before the appender starts. Nothing pins it; check the bundle run's self-test log for "Error reading the log directory" |
| 36 | §4.5 `EngineFor` gives a node's 404 first | Page routes take `ApiResult<EngineFor>`, so body and validation errors are reported before the node's 404, and the engine is opened even for a bad body (R6 review; an accepted deviation never listed until now) |
| 37 | §2 the catalog lock is never held while a query runs | `register_source` (`runtime/sources.rs`) drops the old workspace engine and calls `queue_import` while it holds the catalog lock. SP1 did the same and the code moved verbatim in R4 and R5; L3 fixed only the open and remount paths (S3 audit) |

## Known limits and watch items

| # | Item |
|---|---|
| 38 | §9 caps are unconfirmed (H0). On this 4-core Linux container, idle then under load: stats 338 and 433 ms against 300; CSV 11.3 and 9.3 s against 5; XLSX 15.6 and 16.4 s against 15; workbook confirm 4.7 and 5.4 s against 3; 5 of 9 §4.6 reshape steps (first page under import 2.9x and 4.0x). SP1 misses the same reshape steps by similar margins (A/B against 9d37bde), so they are machine-bound. Rust beats Python in every row (smallest: workbook 2.8x, histogram 3.4x, stats 4.1x). Under-load runs had no Python side |
| 39 | Perf method: page-during-export times one page per attempt in Rust and the median of 5 in `compare.py`; chart cases add an `id > n` filter that keeps almost all rows |
| 40 | §13 blob downloads: verified in Playwright WebKit on Linux for CSV and XLSX only, not Parquet or JSON, and not in the Tauri webview. There is no download handler; H2 checks each OS |
| 41 | §8.5 cross-source joins and several open join Views have no browser coverage; the backend is pinned by `test_cross_node_join_workspaces_stay_open_until_evicted` |
| 42 | Concurrency edges: a search can drop a table another reader is searching (needs 5 Views at once); an eviction mid-build can leave a View's finds at 422 until it is evicted again (V6's other leak, a table evicted while still building, was fixed in V6 round 2: the creator drops it, pinned by `creator_drops_the_table_it_built_when_its_entry_was_evicted`); a failed numbered-table prepare leaves the entry building, so finds read the live relation uncached; W2 holds one process-wide import mutex; a workspace whose `.xlsx` changes after it opens drops on the next mount (422) and recovers on retry; a formula that scans a file directly is not cache-keyed |
| 43 | Timing tests: `pages_of_one_engine_run_concurrently` and the 50 ms window in `catalog_is_free_while_an_engine_opens` can flake on a loaded runner |
| 44 | Timing tests, L14 (`7c7b224`), next to SP1 #22: CI run 38589489926 failed `shutdown_interrupts_a_running_import` on Windows, because stopping took over 2 s under a full parallel test load. The two columnar shutdown tests now use an hours-long scan and a 20 s limit, and the first waits on a channel, so a shutdown that stops interrupting fails instead of hanging (mutation-checked). A loaded runner can still be slower than 20 s, but the test no longer depends on a short wall-clock limit |
| 45 | Visibility (S3 audit of R6, open): `blocking`, `ApiJson` and `http::deps` are `pub` where `pub(crate)` would do (plan section 2, rules digest). No behaviour changes |
| 46 | Test weakness (F2, S4): the panic test hangs on a regression instead of failing; the CI timeout catches it |
| 47 | Test weakness (F4, S5): `dropped_request_frees_only_its_reader` proves "frees" only when `reader_count` is 2 (machines with 4 to 7 cores). On 8 or more cores it checks only that the interrupt reaches the right reader. `run_to_end` has no caller yet |
| 48 | V7 `source_path` (S5) holds the catalog lock while `canonicalize` makes filesystem syscalls. The syscalls are not a query, but a slow disk or network share delays catalog calls. The result is the same path or a 404 |
| 49 | §6.2 export rows stream (L12, `2001a31`): **open.** The unit is on `rust-desktop/parity-L12` and is not on `rust-desktop/parity` (head 9c14008), so until it integrates exports still materialise the result through `query_arrow`. Peak RSS of a 2M-row CSV export (debug server) was 1.10 GB before L12 and 0.74 GB with it; the rest is DuckDB's own reader and buffer working set. Move this row to "Fixed at the exit gate" once `git log` shows 2001a31 on the parity branch |
| 50 | L12 watch item: its completeness check relies on DuckDB returning `UNION ALL` children in order, which `preserve_insertion_order` (DuckDB's default, never changed by Quark) guarantees. The Lead probed DuckDB 1.5.4 with 20 runs (plain, sorted, filtered and grouped-sorted queries of 3 to 5M rows at 4 threads): the flag row was always last and the row order unchanged. Changing that setting would break exports |
| 51 | §10 dev 25 / E4: CI saves Rust caches only from main and `rust-desktop/integration`, so every parity push builds cold (core Windows 54 min). The cache-usage endpoint is blocked through the proxy, so the cache total was not read |
| 52 | The src-tauri crate is linted only on Ubuntu (lint job, no `--all-targets`), so its macOS-only and Windows-only code is not linted in CI |
| 53 | §11 size budget: several units went over the 3-file, 150-line budget with the Lead's consent: F3 (18 files), F8 (295 lines), C8, E1, E3 (each stopped at `needs_split` and resumed), F4, F5, F9, V3, W1, J1 |
| 54 | Process: mid-sprint pushes came from the session's stop hook; H1 (hands-on click-through) could not run in the container and is folded into H2; the stats panel and category list live in E2's panels pass |
| 55 | **Outside SP2:** `frontend/index.html` loads the impeccable live-edit script from `localhost:8400` with a token (SP1 commit 664538f) and ships it in the built app. Remove it and rotate the token before the H2 installers |

## Fixed at the exit gate

Each fix is a Lead unit with a commit on `rust-desktop/parity`; run `git log --oneline` to check one.

- **L1 to L3:** CI pins uv's Python to 3.13; dispatched runs get their own concurrency group; engines open outside the catalog lock (#33 is its fallback).
- **L4:** export decimals render as Python's `str(Decimal)`. **L5:** V7 path tests resolve paths first; F3's overlap timing retries.
- **L6:** CI's `live` job runs the WebKit passes. **L7:** histogram series labelled as Python prints them; scatter repeatability proved without the response cache.
- **L8:** the last 501 answers are gone (unknown paths answer 404 and 405); the live job's timeout is 50 min. **L9:** the dashboard draws every chart kind in the WebKit pass. **L10:** perf cases time uncached routes; `compare.py` joins after the import finishes.
- **L11:** XLSX durations use `[hh]:mm:ss`; a dataset-side join error answers 422 as Python; the line grain uses a fractional span; `~` reads `USERPROFILE` on Windows.
- **L13:** inline test modules moved, with an architecture check; `//!` lines in `tests/`; an HTTP drop test for a non-page route; a non-ignored pages-during-export test; a no-sheets workbook test; a no-excel-extension test; `compare.py`'s 501 wording.
- **L14:** the columnar shutdown tests use an hours-long scan and a 20 s limit (see the watch item beside SP1 #22).
- **E4:** unknown routes answer 404 and 405; the exports sweep is best-effort; fmt and clippy run on all three OSes.
