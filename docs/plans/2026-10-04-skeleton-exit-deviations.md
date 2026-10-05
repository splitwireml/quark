# Quark desktop skeleton: exit-gate deviations

Plan section 12 says every spec deviation needs a human sign-off. This page lists them. Three exit reviewers walked the spec (`2026-10-02-rust-desktop-skeleton-design.md`) against `rust-desktop/skeleton`, and sprint managers audited each sprint. Everything that was a real bug or a missing test was fixed before this list was written (see "Fixed at the exit gate"). What remains is either a deliberate choice or a gap judged too small to fix now.

Mark each item **accept** or **fix** in review.

## Sign-off (2026-10-05)

- **Accepted:** 1 to 3, 5 to 19, and 21 to 25.
- **Fix now:** 20. Eviction now removes each evicted file's `.wal` sibling.
- **Moved to sub-project 2:** 4. Keep the TIMETZ offset by casting in the page query when queries are ported.

## Python-parity choices (made during the run)

| # | Spec or card says | Code does | Why |
|---|---|---|---|
| 1 | A4 card: `INTEGER[]`, `VARCHAR[]` are neither numeric nor text | Classified like Python's `startswith` (numeric / categorical) | Parity with the Python backend (decided 2026-10-04) |
| 2 | A10 card: map keys converted, then stringified | Keys rendered like Python `str(key)` (`True`, `2024-01-01 10:00:00`, `1.50`) | Same parity policy; verified live against DuckDB's Python client |
| 3 | §5.7 | Very small Decimal map keys print positional (`0.00000001`), Python prints `1E-8` | Rare edge; not worth the code |
| 4 | §5.7 TIMETZ follows Python (`12:00:00+02:00`) | Offset is dropped (`12:00:00`): DuckDB exports TIMETZ to Arrow without it | Fix needs a cast in the page query; TIMETZ is rare |

## Error texts outside the section 2 table

| # | Situation | Response |
|---|---|---|
| 5 | Invalid page or page size | 422 `Invalid page or page size` |
| 6 | Upload without a file field | 422 `A file field is required` (Python returns FastAPI's validation list) |
| 7 | JSON body over 2 MB | 413 with axum's default body text |
| 8 | Any mount failure, including non-DuckDB ones | 422 `Invalid filter value: <err>`, matching Python's global `duckdb.Error` handler |

## Spec wording that differs from the code

| # | Spec says | Code does |
|---|---|---|
| 9 | §5.5 guard binds the SQL with `?` | SQL is inlined as an escaped literal (`json_serialize_sql` folds only constants); 26 guard cases pass |
| 10 | §5.1 project name 1 to 200 chars after trimming | Raw length 1 to 200, non-blank after trimming (matches Python's pydantic rule) |
| 11 | §5.1 upload name is Python's `Path(filename).name` | Splits on `/` and `\` on every OS (Python on macOS/Linux keeps backslashes) |
| 12 | §4.1/§7.5 only `frontend/src/lib/api.ts` changes | Also `api-transport.ts`, the `__QUARK_API__` type, its test and the test script entry |
| 13 | §4.3 rebuild generation is part of every cache key | Caches live inside each engine and die with it; the `generation` field is unused |
| 14 | §6.4 a failed import is not retried until the file changes | Not retried within a run; retried once after a relaunch (failed keys are in memory) |
| 15 | §4.3 the catalog lock is never held during a query | Held while a node engine opens (file sniffing) and while a project remounts after a cache swap; user queries run outside it |
| 16 | §6.3 every database request is cancellable | Only the page routes (dataset query and `/sql`) are; dataset listing and mounting are not |
| 17 | §8.1 Rust mode passes `--data-dir <tmp> --port 0` | Also `--cache-dir <tmp>/.quark-cache` (keeps caches per test) |
| 18 | §8.1 no assertion changes | One expected value now derives from the local time zone (F1), so the test passes outside +04:00 |
| 19 | §8.2 cache tests drive the router through `tests/` | Most cache checks are unit tests in `src/` that call the state directly |
| 20 | §6.4 eviction removes cache files | Eviction removes `.duckdb` cache files but not `.wal` siblings (cache files are read-only, so these are rare) |

## Known limits and watch items

| # | Item |
|---|---|
| 21 | D12 deep-page target (60 ms) passes on a quiet machine (53-54 ms) but misses under heavy load (63-71 ms). `perf_acceptance` is `#[ignore]`, so CI never runs it. |
| 22 | The cancellation "frees the engine within 100 ms" test can miss under extreme oversubscription (16 parallel copies), before and after the fix. |
| 23 | `quark-dev-server` serves the router without the CORS/Host/token guard. It binds 127.0.0.1 and exists for tests and browser development only. |
| 24 | The self-test prints `Error reading the log directory/files` on all three OSes (from a dependency, not our code); it still exits 0. |
| 25 | The GitHub Actions cache sits near its 10 GB limit (the Windows entry is about 5 GB), so Ubuntu core and lint often build cold (about 20 minutes). |

## Fixed at the exit gate

X1 BIT values render as bit strings. X2 per-request logging (path, status, duration; never bodies, query strings or the token). X3 cancellation re-interrupts until the query stops (it was lost under load). X4 `.duckdb` sources get a result cache and their folder. X5 four result slots kept apart from one-off queries. X6 and X11 columnar shutdown always stops a running import, with a steady test. X7 to X10 tests for the workspace allow-list, upload cleanup and body limit, allowed origins and navigation, dev-server defaults and the first-page target. X12 Windows-safe cache-touch test. X13 CI reuses the dev-server binary that `cargo test` builds.
