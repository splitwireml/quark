# Sub-project 2 (backend parity): planning notes

Date: 2026-10-05. These notes feed the sub-project 2 brainstorm, spec and plan. They are not a spec. They collect what sub-project 1 (the walking skeleton) left open, what is still unported, and what the skeleton run taught about planning and running the work.

Read with: the pivot spec `docs/plans/2026-10-04-rust-desktop-pivot-design.md` (section 3.2 is this sub-project's scope), the skeleton spec `docs/plans/2026-10-02-rust-desktop-skeleton-design.md` (sections 5.1 to 5.3 set the parity rules), and the signed-off deviations `docs/plans/2026-10-04-skeleton-exit-deviations.md`.

## 1. Scope from the pivot spec

Port every remaining endpoint so the Rust core can replace FastAPI:

- Column stats, category values, charts, joins, exports, cell find and attach.
- XLSX, including an offline replacement for `INSTALL excel`.
- Time-zone handling: ICU versus the Rust side.
- Parallel reads, deferred from the skeleton.
- Whether the legacy routes survive.
- Moving the remaining tests to Rust.

Parity policy (user decision, 2026-10-04): when the Rust and Python backends differ, match Python. Parity covers response shapes, field names, status codes, handled-error `detail` text, identifiers (byte-identical, because Versions store view ids), values after JSON parsing, and the data folder format.

## 2. What is still unported

`backend/app.py` declares 30 route entries. The Rust router serves 13 of them; every other `/api/*` request answers 501 `Not in the desktop build yet`. The 17 unported entries:

| Feature | Route(s) | Notes |
|---|---|---|
| Column stats (profile) | `GET` and `POST /api/nodes/{node_id}/datasets/{dataset}/columns/{column}/stats`, `POST /api/nodes/{node_id}/sql/columns/{column}/stats` | The H2 smoke test used this as its 501 example. |
| Category values (filter panel) | `GET /api/nodes/{node_id}/datasets/{dataset}/columns/{column}/values`, `POST /api/nodes/{node_id}/sql/columns/{column}/values` | Used by the category filter list. |
| Charts and dashboards | `POST /api/nodes/{node_id}/datasets/{dataset}/visualize`, `POST /api/nodes/{node_id}/sql/visualize` | Feeds the Chart and Dashboard canvases. |
| Joins | `POST /api/join-workspaces` | Version replay also calls it for Versions that carry a join. |
| Cell find | `POST /api/nodes/{node_id}/sql/find` | |
| Exports | `POST /api/exports` | CSV, XLSX, Parquet, JSON; Python returns a `FileResponse` download. |
| Attach a database path | `POST /api/projects/{project_id}/sources/attach`, `POST /api/nodes/attach` | Takes a typed absolute path today. |
| XLSX workbook staging | `POST .../sources/upload/{stage_id}/confirm`, `DELETE .../sources/upload/{stage_id}`, and their `/api/nodes/upload/...` twins | The `.xlsx` upload itself also returns 501 today. Python uses `INSTALL excel`, which needs network access. |
| Source path | `GET /api/projects/{project_id}/sources/{source_id}/path` | Returns the resolved file path. |

Already ported: project list and create, project sources, source detail, project views, legacy node list and datasets, `/sql` and dataset `/query` pages (JSON and Arrow), uploads for the formats other than XLSX, and source or node delete.

## 3. Tests

- `tests/test_backend.py` has 102 tests. The 22 marked `skeleton` (41 cases with parameters) pass in Rust mode (`QUARK_TEST_BACKEND=rust`). The other 80 or so are this sub-project's parity oracle, and each one moved over is evidence that a route is done.
- `cargo test -p quark-core` runs 190 tests plus one ignored release-mode perf test (`tests/perf_acceptance.rs`).
- The frontend has 149 node tests and browser tests under `frontend/tests/*-browser.mjs`. Playwright 1.62.1 and WebKit 2336 are in the local npx cache, and the desktop app's WebKit engines (WKWebView, WebKitGTK) are closer to Playwright WebKit than to Chromium. The H2 footer bug reproduced only in WebKit.

## 4. Carried over from sub-project 1

Signed-off deviations that land here:

- **Deviation 4, TIMETZ offset dropped:** `12:00:00+02:00` shows as `12:00:00` because DuckDB exports TIMETZ to Arrow without the offset. The fix is a cast in the page query; settle it together with the time-zone decision.
- **Parallel reads:** each engine's connection sits behind one mutex (a `ponytail:` comment names the ceiling). New heavy routes (stats, charts, exports, joins) make that ceiling more visible.
- **Deviation 16, cancellation:** only the page routes cancel. Stats, charts, exports and joins can be slow, so decide whether they cancel like the page routes.
- **Legacy routes:** `/api/nodes`, `/api/nodes/upload`, `DELETE /api/nodes/{node_id}`, `/api/nodes/{node_id}/datasets` and the dataset `/query` route. The UI uses none of them; they exist because the pytest suite creates data through them. Decide whether they survive.
- **Deviation 13:** the cache `generation` field is unused; use it or remove it.
- **Deviation 14:** failed columnar imports are remembered in memory only, so one retry happens after a relaunch.
- **Deviations 21 and 22:** the 60 ms deep-page target and the 100 ms cancel test have little margin under load. New routes need their own targets.
- **Deviation 24:** the self-test prints a dependency's log-directory error and still exits 0.
- **Deviation 25:** the Actions cache sits near 10 GB (Windows about 5 GB), so Ubuntu jobs often build cold (about 20 minutes).

Smaller items:

- `.wal` files left by evictions before `2a80026` are not swept (`remove_stray_partials` only removes `.partial.wal`). Tester machines only.
- Upload progress is indeterminate. On Ubuntu a 254 MB upload took about 10 s; measure how that splits between transfer and import before adding byte progress.
- The 501 message shows no status code. That stops mattering once these routes are ported.
- Ubuntu `.deb`: installing pulls in `fuse3`, whose conffile prompt can stop a non-interactive install. This is a packaging watch item for sub-project 4.

Frontend fixes made on `rust-desktop/skeleton` after H2 (they also apply to `main`, which still has the bugs): footer page after a jump, dedupe missing from the Version diff, header timing on prefetched pages, upload busy state, the Versions and Export menus drawing under the condition bar, and Versions restoring their filters, sorts and dedupe keys.

## 5. Open questions for the design

1. **Branch.** The pivot spec suggests `rust-desktop/<name>` off `rust-desktop/integration`, but the skeleton is not merged into integration. Either merge the skeleton first (user go-ahead needed) or branch from `rust-desktop/skeleton`.
2. **Legacy routes:** keep or drop (see section 4).
3. **Time zones:** ICU in DuckDB versus conversion on the Rust side. This also settles deviation 4.
4. **XLSX offline:** bundle DuckDB's `excel` extension with the app, or read with a Rust crate and write with another. This covers both import (workbook staging) and XLSX export.
5. **Tests:** keep pytest as the oracle until sub-project 5 retires Python, or port the tests to Rust as each route lands.
6. **Parallel reads:** design, for example several read connections per engine, and how it interacts with the result and columnar caches.
7. **Performance targets** for stats, values, charts, joins, find and exports, measured the way `perf_acceptance` measures pages.
8. **Boundary with sub-project 3:** exports, attach and the source-path route touch native files. Python streams an export as a download and takes a typed path for attach; native save and open dialogs and reveal-in-folder belong to sub-project 3. Decide what sub-project 2 ships for these (HTTP parity only, or a minimal desktop behavior).

## 6. Measured baselines (release build, 2026-10-05, this Mac)

2M-row, 254 MB CSV (`perf_acceptance`):

| Step | Measured | Target |
|---|---|---|
| First page of a brand-new CSV, filter on, import running | 484 ms | about 500 ms (750 ms test bound) |
| Columnar import (background) | 534 ms | none set |
| Page at row 1,000,000 after the import | 53.9 ms | 60 ms or less |
| Sorted page at offsets 0, 500,000, 1,000,000 | 1.4 to 1.9 ms | 5 ms or less |

## 7. Lessons from running sub-project 1

- **Size of the run:** 11 batches plus fix batches F1 to F10 and exit fixes X1 to X13, from 52 unit cards. Batches used 6 to 10 agents, 170k to 414k reported tokens and 6 to 37 minutes each. Reported per-agent tokens approximate final context size; billed input including cache reads is 5 to 13 times higher.
- **Machine limits:** the 10-core, 16 GB Mac crashed apps when three workflows built cargo and DuckDB at once (load average about 92). Run one workflow at a time, cap builds at `jobs = 4` (`/Users/mali/Development/quark-wt/.cargo/config.toml`), and keep stress tests to four copies or fewer.
- **CI:** a push run takes 23 to 39 minutes and a bundle run about 35 (the universal macOS build is the slowest). The `ci-<ref>` concurrency group cancels in-progress runs, so dispatch a bundle only after the push run finishes, and expect a new push to cancel the running one.
- **Windows-only failures appear only in CI:** no `<>:"|?*` in fixture names, and files DuckDB holds open cannot be opened for writing.
- **macOS 27 release builds** need `[profile.release.build-override] strip = false` (already in `Cargo.toml`).
- **Git:** never pipe `git merge --ff-only`; check its exit status, because a masked failure once deleted a branch.
- **Unit size that reviewed well:** 3 files or fewer and about 150 lines, with distinct files across units that run in parallel.
- **Frontend state bugs** (proxies, caches) slipped past unit tests and showed up only in the hands-on smoke test. Plan a scripted WebKit pass over each newly ported feature, not just pytest parity.
