# Quark desktop backend parity: implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking. Section 1 sets who runs which step: workers (the leaves) write code and managers review it. Section 5 sets the order: gated batches, with every worker in its own git worktree.

**Goal:** Build sub-project 2 of the Rust desktop pivot: port the 17 remaining route entries of `backend/app.py` to the Rust core, so the desktop app never needs FastAPI, and take the efficiency upgrades the Python version skipped. The work also covers a read pool, a response cache, offline XLSX, ICU time zones, three visible behaviour fixes, and the code reshaped into a CI-enforced layout.

**Architecture:**
- `crates/quark-core` is reshaped first into atomic layers:
  - `shared/`: atoms;
  - `relation/`: shared molecules;
  - `features/<name>/`: `model.rs`, `queries.rs`, services and `routes.rs`;
  - `http/`: router and dependency extractors;
  - `runtime/`: engines, read pool, caches and catalog.
- Every new route then lands in its own feature folder.
- The existing pytest suite stays the API test suite and runs against the Rust dev server. A strict pending list tracks what is not ported yet.

**Tech stack:**
- Rust 1.97.1 (edition 2024).
- `duckdb` crate `=1.10504.0` (DuckDB 1.5.4, bundled), gaining `icu` in F1 and `appender-arrow` in X5.
- axum 0.8, tokio, `tokio-util`.
- `calamine` to read XLSX and `rust_xlsxwriter` to write it.
- pytest and httpx (existing), Playwright WebKit (from the local npx cache), GitHub Actions.

**Spec:** `docs/plans/2026-10-08-rust-desktop-parity-design.md`. Managers read it. Workers get everything they need through their unit cards. References such as "spec §5.1" point into it. The parity oracle is `backend/app.py` at `0270c8d`.

In unit cards, `core/…` means `crates/quark-core/…`.

---

## 1. Team and execution

The team model is SP1's (skeleton plan §1), unchanged where this section does not say otherwise. `docs/plans/2026-10-08-rust-desktop-parity-orchestration.md` (the orchestration map) holds:
- the batches and review groups;
- the critical path;
- agent counts and the token estimate;
- the dependency graph and the team tree.

### 1.1 Tiers

| Tier | Role | Count | Owns | Reads | Reviews |
|---|---|---|---|---|---|
| L0 | Lead | 1, a fresh main session | Pre-flight, batch order and gates, the run log, escalations, human checkpoints, the exit gate | Review-group reports, batch gate output, sprint reports | Batch gates |
| L1 | Sprint manager (SM) | 8, one per sprint of 2 batches | The sprint gate: push, CI, hot-spot audit | Its sprint's group reports, CI JSON, card hot spots | Its task managers' work |
| L2 | Task manager (TM) | 21, plus the W4 loop manager | One review group of 3 units or fewer: worktrees, reviews, integration; the loop manager runs W4's fix loop | Unit reports and diffs | Every diff in its group, line by line |
| L3 | Worker | 46, plus fix and split units | Exactly one unit card, in its own worktree | Its card, section 2, the signatures of its **Needs** | Its own tests |

### 1.2 Streams

Streams own files (section 4), so units in one batch never collide, apart from the additive registration points listed in section 4.

| Stream | Owns | Units |
|---|---|---|
| R: reshape | every moved file, `lib.rs`, `http/`, `src-tauri/src/lib.rs` | R1–R7 |
| F: foundations | `Cargo.toml` (features), `runtime/{pool,engine,cancel}`, `runtime/cache/responses`, `http/deps.rs`, `relation/rows.rs` select list, `tests/conftest.py`, `tests/rust_pending.txt`, `tests/perf/`, the corpus files, CI | F1–F9 |
| V: stats, values, find, attach | `relation/{filtered,rows,profile}`, `features/{stats,values,find}`, `features/sources/attach.rs`, `runtime/cache/numbered.rs` | V1–V7 |
| C: charts | `features/charts/` | C1–C8 |
| J: joins | `features/joins/`, `runtime/joins.rs` | J1–J2 |
| X: exports | `shared/encode/export.rs`, `features/exports/` | X1–X5 |
| W: workbooks | `runtime/cache/workbook*.rs`, `features/workbooks/` | W1–W4 |
| E: exit | `core/tests/{perf_acceptance,odd_names}.rs`, `frontend/tests/live/`, `http/fallback.rs`, exit docs | E1–E5 |

### 1.3 Gated batches

- **Batches:**
  - 15 batches, equal to the critical path.
  - Each run has 6 workers or fewer, 2 task managers or fewer, and one spare slot for a change-round fixer or re-review, so 9 agents at most.
  - Only one workflow runs at a time on this Mac, with `jobs = 4`.
- **Batch contents:** a batch holds only units whose **Needs** were integrated by earlier batches. The additive registration points in section 4 may collide; the integrator keeps both sides.
- **Sprint gates:** every second batch closes a sprint. The next batch may start while CI runs, but CI must be green before the batch after that starts: a one-batch lag, as in SP1.
- **The pending list:** a unit deletes from `tests/rust_pending.txt` the line of every test that now passes, its own or not. Strict `xfail` fails the suite otherwise. Its task manager checks that each deleted test really passes.
- **The architecture test:** from R7 on, every unit's done-when also passes `rtk cargo test -p quark-core --test architecture`.

### 1.4 Workers and managers

The rules of SP1 plan §1.4 and §1.5 apply. They are embedded in the agent definitions that pre-flight step P3 updates. Changes for this plan:
- **Budgets:**
  - R units count only edits that are not moves (stream R rules in section 7).
  - F3, X3 and E4 have the file counts their cards state.
  - The W4 fix units have 100 lines each.
- **Done-when:** a card's **Done when** is also its group gate. For cards that list **pytest it makes pass**, the task manager runs the Rust-mode suite and confirms those tests pass.
- **Card lookup:** task managers pull a card with `rg -n '^#### <ID> ·' docs/plans/2026-10-08-rust-desktop-parity.md`, then `sed -n` its line range. Never the whole plan.

### 1.5 Pre-flight (Lead, once)

- [ ] **P1:** the main checkout `/Users/mali/Development/quark` is on `rust-desktop/parity`, and each integration fast-forwards it. During the run, the human neither commits on this branch nor edits files the plan touches.
- [ ] **P2:** CMake 3.20 or later is installed (`cmake --version`). If it is missing, ask the human to approve `brew install cmake`; it is a download. F1, in batch 8, cannot start without it.
- [ ] **P3:** update the four agent definitions in `.claude/agents/` (`quark-worker`, `quark-task-manager`, `quark-sprint-manager`, `quark-exit-reviewer`):
  - the plan and spec paths become this plan and its spec;
  - the branch becomes `rust-desktop/parity`, and unit branches become `rust-desktop/parity-<unit>`;
  - the embedded global constraints are replaced with section 2;
  - the exit reviewer's coverage table is section 16.

  Commit on `rust-desktop/parity` as `chore(agents): point the quark agents at the parity plan`.
- [ ] **P4:** copy `quark-wt/quark-batch.js` to `quark-wt/parity-batch.js`, and `quark-wt/quark-parity.js` to `quark-wt/parity-loop.js` (for W4). In both, set `PLAN` to this plan, and the branch and worktree prefixes to `rust-desktop/parity` and `parity-`.
- [ ] **P5:** prepare git and the gate worktree.
  1. Run `git config gc.auto 0`.
  2. Remove any stale gate worktree, then run `git worktree add --detach /Users/mali/Development/quark-wt/gate rust-desktop/parity`.
  3. Warm the gate: `cargo build -p quark-core --bins`, `npm ci` in `gate/frontend`, and `uv sync --frozen`.
- [ ] **P6:** start the run log at `/Users/mali/Development/quark-wt/runlog-parity.md`, outside the repository.
- [ ] **P7:** check the tools:
  - `rustc 1.97.1`, `uv`, Node 22, `npx`;
  - `cmake --version`;
  - `ls ~/Library/Caches/ms-playwright/webkit-2336`;
  - `codegraph status -p /Users/mali/Development/quark`;
  - `/Users/mali/Development/quark-wt/.cargo/config.toml` holds `jobs = 4`.

### 1.6 Worktrees and integration

As in SP1 plan §1.6, with this plan's names.

**Per unit (its task manager, before assigning it):**
```
wt=/Users/mali/Development/quark-wt/parity-<unit>
git worktree add -b rust-desktop/parity-<unit> "$wt" rust-desktop/parity
cp -cR /Users/mali/Development/quark-wt/gate/target "$wt/target"
cp -cR /Users/mali/Development/quark-wt/gate/frontend/node_modules "$wt/frontend/node_modules"   # units that build the frontend or run WebKit only
(cd "$wt" && uv sync --frozen)                                                                   # units that run pytest only
```

**Integration (task manager, after approving a unit):**
```
git -C "$wt" rebase rust-desktop/parity            # additive conflicts: keep both sides
(cd "$wt" && <the unit's Done when command>)
git -C /Users/mali/Development/quark merge --ff-only rust-desktop/parity-<unit>
```
Check the exit status of the merge, and never pipe it. A masked failure once deleted a branch. Afterwards, run `git worktree remove "$wt"` and `git branch -D rust-desktop/parity-<unit>`.

**Batch gate (Lead):**
1. Every unit in the batch is approved and integrated.
2. Run `codegraph sync -p /Users/mali/Development/quark`.
3. In the gate worktree, run `git checkout --detach rust-desktop/parity`, then the batch gate in section 6.
4. Add one line to the run log.
5. If the gate is red, open a fix batch: one fix unit per failure cluster, 5 at most.

**Safety rules:**
- Only sprint managers push, only `rust-desktop/parity`, and only at sprint gates. Nothing is merged into `rust-desktop/integration` or `main`, and no pull requests are opened.
- If a git command fails on a `.lock` file, wait 2 s and retry; never delete lock files.
- Never stage `.agents/`, `.claude/skills/`, `.hermes/`, `.pi/` or `skills-lock.json`. The only exception is P3's agent-definition commit.

### 1.7 Message templates

SP1 plan §1.7 applies with `rust-desktop/parity` in place of `rust-desktop/skeleton`, and the worktree path `quark-wt/parity-<unit>`. That covers the assignment, unit report, verdict, review-group report, run log line and sprint report. The assignment also says:

> Read your card with `sed -n '<range>p' docs/plans/2026-10-08-rust-desktop-parity.md`; the global constraints are in your agent definition.

### 1.8 Escalation and human checkpoints

**Escalation:** worker to task manager to Lead to human, as in SP1. These also go to the human:
- a failed ICU build on any target (spec §5.4 fallback);
- every deviation, at the exit gate;
- any change to a response shape (never allowed without the human).

**Human checkpoints:**
- **P2, before batch 8:** approve the CMake install if it is missing.
- **H0, after batch 9 (F7), not blocking:** the Lead sends the Python baseline table, and the human confirms or adjusts the performance caps in spec §9. Work goes on with the provisional caps, and E1 uses whatever is confirmed by then.
- **H1, after batch 12, recommended, not blocking:** in `npx --yes @tauri-apps/cli@^2.12 dev`, the human clicks through:
  - stats;
  - the filter values list;
  - find;
  - every chart kind;
  - a dashboard.
- **H2, at the exit gate, required:** `docs/handoffs/PARITY_H2_CHECKLIST.md` (E5) on macOS, Windows and Ubuntu with the bundle-run installers. It includes SP1's remaining H2 steps.

### 1.9 Subagents spawned

The map's figures:
- 79 agents planned, 79–88 with fix and split units;
- at most 9 per run, and about 11 alive at once;
- about 4.1M tokens in the runtime's per-agent unit (±50%).

Billed input, including cache reads, runs 5–13× that. After batch 8, the first full batch, rerun the map with `--avg-agent-k <observed>` and tell the human the new total.

### 1.10 Context budget

SP1 plan §1.10 applies unchanged:
- lean agent definitions;
- cards pasted, never the plan;
- CodeGraph before reads, with large files by line range only;
- `rtk` output;
- a handoff at about 40 tool calls or 80k tokens;
- the Lead as a fresh session running one Workflow per batch.

The never-read-whole files are in section 2.

---

## 2. Global Constraints

Every unit's requirements include this section.

**Process**
- **Context budget (every agent):** target 60k tokens of context, hard stop at 100k.
  - Past about 40 tool calls, stop and send a handoff: what is done, what is left, open questions.
  - Look code up with CodeGraph first, using `-p /Users/mali/Development/quark`:
    - `codegraph node -p <repo> <symbol>` for one symbol;
    - `codegraph explore -p <repo> --max-files 3 "<names>"` for an area.
  - For a port, start with the Python symbols listed for your unit under **Python reference symbols** below.
  - Read only what your card or assignment names. In a file longer than about 1,000 lines, use `rg -n` and read line ranges.
  - Never read these in full: `tests/test_backend.py`, `frontend/src/App.svelte`, `backend/app.py`, this plan, or the spec.
  - Prefix every shell command with `rtk`.
- **Worktree:** work only inside your own worktree (section 1.6).
- **Commits:**
  - Subjects follow Conventional Commits with scopes `core`, `desktop`, `ui`, `tests`, `ci`, `build` or `docs`.
  - Every message ends with a `Co-Authored-By: Claude <model> 5.5 <noreply@anthropic.com>` trailer naming the model that wrote it (workers: Sonnet 5.5).
- **Tauri CLI:** `npx --yes @tauri-apps/cli@^2.12 <command>`, run from the worktree root.

**Toolchain and build**
- **Toolchain:** `rust-toolchain.toml` pins `1.97.1` with `rustfmt` and `clippy`. Workspace edition `2024`, `rust-version = "1.97"`, resolver `"3"`.
- **Lints:** set at workspace level.
  - `unsafe_code = "deny"`.
  - Clippy: `correctness` deny; `suspicious`, `style`, `complexity` and `perf` warn; `unwrap_used` deny; `expect_used` warn; `dbg_macro` and `todo` deny.
  - Tests may unwrap (`clippy.toml`). CI runs clippy with `-D warnings`.
- **DuckDB:**
  - `duckdb = { version = "=1.10504.0", features = [...] }`. The features are `["bundled", "parquet", "json"]` until F1, `["bundled", "parquet", "json", "icu"]` from F1 on, and X5 adds `"appender-arrow"`.
  - The engine must report `v1.5.4`.
  - `libduckdb-sys` builds at `opt-level = 3` even in dev.
  - From F1 on, building needs CMake 3.20 or later on the machine.
- **Builds:** `/Users/mali/Development/quark-wt/.cargo/config.toml` caps cargo at `jobs = 4`. Never raise it. The 10-core, 16 GB Mac crashes apps under parallel DuckDB builds.
- **New crates:** each is added only by the unit that names it.
  - `calamine`: W1.
  - `rust_xlsxwriter`: W1 as a dev-dependency, X4 as a dependency.
  - `tokio-util` with the `io` feature: X3.

  Add each with `cargo add` at its latest release, and give the version in the unit report.

**Code structure** (spec §4; `core/tests/architecture.rs` enforces it from R7 on)
- **Layers:**
  - `shared/`: atoms, pure, with no DuckDB calls and no HTTP.
  - `relation/`: shared molecules every feature reads through.
  - `features/<name>/`: `model.rs` (serde types), `queries.rs` (one SQL statement family per function), service files (rules), `routes.rs` (thin handlers) and `tests.rs`.
  - `http/`: router, `deps.rs` extractors, `secure.rs`.
  - `runtime/`: engines, read pool, cancellation, mounting, catalog, registry, caches and workers.
- **Direction:** calls go one way only.
  - Routes call services, services call queries, and queries use `relation/` and `shared/`.
  - Routes take their dependencies from `http/deps.rs`, which draws on `runtime/`.
  - A service that changes app state gets a runtime handle as an argument.
  - `runtime/` never imports `features/` or `http/`.
  - A feature never imports another feature. Shared logic moves down a layer instead.
- **Thin routes:** a handler declares its dependencies, calls one service function and returns. When a feature adds no rules of its own, its route may call one runtime operation directly (for example `delete_source`). No SQL and no business rules in `routes.rs`.
- **Services:** never import `axum`. They take a connection and typed input, and return typed output or `ApiError`.
- **File size:**
  - Production files aim for 200 lines or fewer, with a hard cap of 250. A file over the cap is split by responsibility.
  - Unit tests live in a sibling `tests.rs`, declared as `#[cfg(test)] mod tests;` (so `foo.rs` keeps its tests in `foo/tests.rs`). Test files cap at 400 lines.
- **Names:**
  - Every file starts with a one-line `//!` purpose.
  - Files are nouns and functions are verbs.
  - Handler names match the route's action.

**Errors**

User-facing failures are `ApiError`, rendered as `{"detail": "<text>"}`. The exact texts, SP1's plus this sub-project's:

| Status | Detail |
|---|---|
| 400 | "Unsupported file type", "Could not open source: <err>", "Only .duckdb and .db paths can be attached" |
| 401 | "Unauthorized" |
| 403 | "Forbidden host" |
| 404 | "Project not found", "Source not found", "Node not found", "Dataset not found", "Column not found", "Workbook upload not found", "Database not found", "Not Found" (unknown path, from E4 on) |
| 405 | "Method Not Allowed" (wrong method on a known path, from E4 on) |
| 422, SP1 | "Project name is required", "Invalid filter column or operator", "Filter value is required", "IN filter requires a non-empty list", "BETWEEN filter requires two bounds", "Text operator requires a text column", "Invalid dedupe column", "Invalid sort column", "Invalid page or page size", "SQL accepts only one read-only SELECT query", "Invalid SQL query: <err>", "Invalid filter value: <err>" |
| 422, stats and values | "Column cannot be profiled", "Column is not text" |
| 422, find | "Invalid search column", "Cannot search this View: <err>" |
| 422, charts | "A metric formula cannot contain ? placeholders", "Metric column was not found", "This metric requires a numeric column", "This metric formula did not run: <err>", "<Label> requires a category encoding", "<Label> measure column was not found", "<Label> measure requires a numeric column", "<Label> group column was not found" (`<Label>` is `Bar` or `Pie`), "A pie chart cannot show negative values; use a bar chart instead", "Histogram requires a value encoding", "Histogram requires a numeric or date column", "Histogram group column was not found", "Box requires a value encoding", "Box requires a numeric column", "Box group column was not found", "Scatter requires x and y encodings", "Scatter <role> column was not found" (`color`, `size`, `shape`), "Scatter size requires a numeric column", "Line requires an x encoding", "Line requires a date or timestamp x column; <x> is not one", "Line measure column was not found", "Line measure requires a numeric column", "Line group column was not found", "Chart type <chart> is not available yet" |
| 422, joins | "Join key counts must match", "Join keys must be unique", "Join reference requires exactly one of dataset or sql", "SQL joins require one project workspace", "Join key not found" |
| 422, exports | "<FORMAT> export requires exactly one sheet", "<FORMAT> export requires unique column names" (`<FORMAT>` is `CSV`, `PARQUET` or `JSON`), "Excel cell text exceeds 32767 characters" |
| 422, workbooks | "Select one or more unique workbook sheets" |
| 422, request validation | Any body, path or query value outside its declared type or range. Detail text may differ from pydantic's. |
| 500 | "Internal error" |
| 501 | "Not in the desktop build yet" (until E4; replaced by FastAPI's 404/405 in E4 and L8) |

A DuckDB error that no route maps on its own becomes 422 `Invalid filter value: <err>`, as Python's global handler does. This applies to stats, values, charts, joins and mounting.

**Runtime behavior**
- **Concurrency:**
  - No lock is held across `.await`.
  - All DuckDB work runs in `spawn_blocking`, on a pooled reader (`Engine::read`) or on the engine's primary connection (catalog changes only).
  - Cache locks are held only to read or write an entry, never during a query. The catalog lock is never held while a query runs.
  - Every `std::sync::Mutex` is locked through a helper that recovers from poisoning.
- **Read pool:** `reader_count()` is half of `available_parallelism()`, clamped to 2..=4.
- **Caches:**
  - count and null cache: 256 entries per engine;
  - result cache: 4 per engine, with 1 build in flight;
  - response cache: 128 per engine;
  - find tables: 4 per engine;
  - columnar cap: 10 GB;
  - join workspaces: 4.
  - Folders: `<cache>/columnar/` (columnar and workbook caches), `<cache>/duckdb-tmp/`, `<cache>/exports/`.
- **Paging:** `page >= 1`, `1 <= page_size <= 1000`, defaults page 1 and page size 100.
- **Arrow:** media type `application/vnd.apache.arrow.stream`, schema metadata key `quark`, header `Vary: Accept`.
- **Constants copied from `backend/app.py`:**
  - Chart limits: `BAR_LIMIT = 30`, `PIE_LIMIT = 8`, `SERIES_LIMIT = 6`, `BOX_GROUP_LIMIT = 12`, `OUTLIER_LIMIT = 200`, `SCATTER_LIMIT = 5000`, `LINE_POINT_LIMIT = 400`.
  - `GRAIN_SECONDS = {hour: 3600, day: 86400, week: 604800, month: 2629746, quarter: 7889238, year: 31556952}`.
  - `METRIC_SQL = {count: count(c), distinct: count(DISTINCT c), min: min(c), max: max(c), sum: sum(c), avg: avg(c), median: median(c), stddev: stddev_samp(c)}`.
  - Export rules: `EXCEL_TEXT_LIMIT = 32767` and `CSV_FORMULA_PREFIXES = ("=", "+", "-", "@", "\t", "\r")`.
  - `ILLEGAL_XML` strips U+0000–U+0008, U+000B–U+000C, U+000E–U+001F, lone surrogates, U+FFFE and U+FFFF.
  - `EXPORT_TYPES`: csv `text/csv`, xlsx `application/vnd.openxmlformats-officedocument.spreadsheetml.sheet`, parquet `application/vnd.apache.parquet`, json `application/json`.
- **Request limits** (all 422 when broken):
  - Category values: `offset >= 0`; `1 <= limit <= 500`, default 200; `search` defaults to `""`.
  - Find:
    - `term` is 1 to 1000 characters;
    - `columns` has at least 1 entry;
    - `after_row` and `after_column` are at least -1, default -1;
    - `direction` is `next` or `previous`, default `next`.
  - Chart spec: `expression` at most 2000 characters and `label` at most 200; the enums are listed in C1.
  - Exports: 1 to 100 sheets.

**Security** (unchanged from SP1)
- Bind `127.0.0.1:0`. The token is 32 random bytes as 64 lowercase hex characters.
- CORS origins are exactly `tauri://localhost` and `http://tauri.localhost`, plus `http://localhost:5173` in debug builds.
- The Host header must equal `127.0.0.1:<port>`. Preflight max-age is 86400.
- Every database is locked down before user SQL reaches it (skeleton design §5.4). Export writers may write only into `<cache>/exports/`.

**Never**
- No `panic = "abort"`.
- No new frontend dependencies.
- No Python dependency changes, and no edits to `backend/app.py`, which is the oracle.
- Never edit a test's assertion to make Rust pass. The exceptions are the `python_only` and `rust_only` markings, and the new tests, that cards F6, C5, C7, J2 and W3 name.
- No logging of row values, file contents or the token; SQL text is logged only at debug level.

**Rust rules digest** (from `rust-skills`; rule ids in brackets)
- **Errors:**
  - Use the typed `ApiError` for anything a user can see [err-thiserror-lib].
  - Use `anyhow` with `.context(...)` only for startup, caches and internal plumbing [err-anyhow-app, err-context-chain].
  - Propagate with `?`. No `unwrap()` outside tests. Use `expect()` only for real invariants, with a message saying why [err-no-unwrap-prod, err-expect-bugs-only].
- **Borrowing:** take `&str`, `&[T]` and `&Path`; clone only when you need ownership [own-borrow-over-clone, own-slice-over-vec].
- **Async:** never hold a lock across `.await`. DuckDB and file-heavy work go in `spawn_blocking` [async-no-lock-await, async-spawn-blocking].
- **Types:** use enums, not strings, for closed sets. Parse at the boundary with serde (`deny_unknown_fields`, lowercase renames) [type-no-stringly, api-parse-dont-validate, serde-deny-unknown-fields].
- **Hot paths** (cell conversion, export writers):
  - No `format!` per cell when writing into an existing buffer works [anti-format-hot-path].
  - Use `with_capacity` when the size is known [mem-with-capacity].
- **Visibility:** `pub(crate)` for internals; `pub` only for what other modules or crates use [proj-pub-crate-internal].
- **Modules:**
  - Folders by feature [proj-mod-by-feature].
  - Folders with four or more submodules use `mod.rs`: `shared/`, `shared/encode/`, `relation/`, `runtime/`, `runtime/cache/`, `features/` and each feature. Smaller ones use an adjacent `foo.rs` beside `foo/`: `relation/profile.rs`, and every `foo/tests.rs` [proj-mod-rs-dir].
  - Tests in a `#[cfg(test)]` child module [test-cfg-test-module].
- **Tests:**
  - Names describe the behavior [test-descriptive-names].
  - Integration tests go in `core/tests/` [test-integration-dir].
- **Naming:**
  - Treat acronyms as words: `ApiError`, `SqlQueryRequest` [name-acronym-word].
  - Booleans start with `is_` or `has_` [name-is-has-bool].

**Python reference symbols** (in `backend/app.py`; look each up with `codegraph node -p /Users/mali/Development/quark -f backend/app.py <symbol>`)

| Units | Symbols |
|---|---|
| V1 | `filtered_relation`, `sql_metadata`, `metadata` |
| V2, V3 | `profile_response`, `profile_kind`, `safe`, plus the constants `INTEGER`, `NUMERIC`, `DATE` |
| V4 | `stats`, `sql_stats`, `profile`, `duckdb_error` |
| V5 | `category_response`, `category_values`, `sql_category_values` |
| V6 | `find_cell`, `CellSearch` |
| V7 | `attach`, `AttachRequest`, `get_project_source_path`, `source_for`, `add`, `public` |
| F5 | `profile`, `cached_profile` |
| C1 | `ChartEncodings`, `ChartSpec`, `VisualizeRequest`, `SqlVisualizeRequest` |
| C2 | `visualize_response` (metric branch), `visualize`, `sql_visualize`, `read_only_sql`, `METRIC_SQL` |
| C3, C4 | `visualize_response` (bar and pie branch) |
| C5 | `visualize_response` (histogram branch), `profile_response` |
| C6 | `visualize_response` (box branch) |
| C7 | `visualize_response` (scatter branch) |
| C8 | `visualize_response` (line branch), `GRAIN_SECONDS` |
| J1, J2 | `create_join_workspace`, `JoinReference`, `JoinWorkspaceRequest`, `mount_dataset`, `get_connection` |
| X1 | `export_value`, `export_nested`, `csv_value`, `safe` |
| X2 | `ExportRequest`, `ExportSheet`, `export_sheet_name`, `export_filename`, `EXPORT_TYPES` |
| X3, X4, X5 | `export`, `xlsx_row`, `export_relation` |
| W1, W2 | `workbook_sheets`, `_xlsx_table`, `connect`, `mount_dataset`, `mount_project_source` |
| W3 | `upload`, `confirm_workbook`, `discard_workbook`, `WorkbookConfirm` |

Units not listed have no Python counterpart. They look up the Rust symbols in their **Needs**.

## 3. Review Focus

These are the failure modes the spec implies but no ported test covers, most likely to hurt a real user first. Each line names the test that pins it and the unit that owns that test.

1. **A dashboard burst.** About eight chart requests and fast scrolling hit one project while filters change and drop earlier requests. Nothing may deadlock, no request may cancel another one's reader, and the grid must keep loading. Test: `burst_of_requests_with_drops_never_deadlocks` (F4).
2. **A source edited in place.** A CSV re-saved with the same size and schema must never return cached stats, values or charts from the old content. Test: `in_place_edit_of_the_source_misses` (F5).
3. **Awkward column names.** Names with `"`, spaces, non-ASCII letters, or that match SQL keywords (`a"b`, `with space`, `Zoë`, `select`) must work in stats, category values, find, a bar chart and a CSV export. Test: `odd_column_names_work_in_every_analysis_route` (E1).
4. **Machines not on UTC, across DST.** Line-chart buckets and stats year counts must follow the session time zone. CI runs on UTC and would hide a bug here. Tests: `date_trunc_and_strftime_use_the_session_zone` (F1) and `line_buckets_follow_the_session_zone_across_dst` (C8).
5. **An export the user abandons, and an export with no rows.** A client that disconnects mid-download leaves no file behind. An empty result still downloads a valid file: a CSV header, an XLSX header row, JSON `[]`, and a Parquet file with the schema. Tests: `abandoned_export_leaves_no_file` (X3) and `empty_results_export_valid_files` (X3, X4, X5 each add their format).

## 4. File structure

After the reshape (sprint 0), with the stream that owns each area:

```
crates/quark-core/src/
  lib.rs                         R   module list; pub use http::router
  error.rs                           ApiError
  shared/                        R   sql, literal, ids, naming, types, columns, encode/{cell, scalar, arrow, export(X1)}
  relation/                      R V request, builder, params, target, describe, guard, filtered(V1), rows(V1), profile/(V2, V3)
  runtime/                       R F engine, dirs, pool(F2), cancel, mount, registry, state, catalog, engines, sources, files,
                                     joins(J2), cache/{stats, results, result_build, columnar_key, columnar, responses(F5),
                                     numbered(V6), workbook(W1), workbook_import(W2)}
  features/
    projects/  sources/  pages/  R   SP1 features, reshaped; sources/attach.rs (V7)
    stats/  values/  find/       V
    charts/                      C   model, service, metric, bar, groups, histogram, box_plot, scatter, line, routes
    joins/                       J
    exports/                     X   model, names, csv, xlsx, copy, service, routes
    workbooks/                   W
  http/                          R E mod (router), deps, secure, fallback (E4)
  bin/quark-dev-server.rs        R
crates/quark-core/tests/
  architecture.rs                R
  time_zones.rs                  F
  read_pool.rs                   F
  odd_names.rs                   E
  cancellation.rs  cache_property.rs  perf_acceptance.rs  dev_server.rs  engine_version.rs   (SP1; updated by R and E)
src-tauri/src/lib.rs             R   import paths only
tests/                           F W E   conftest.py, rust_pending.txt, workbook_corpus.py, test_workbook_corpus.py, perf/compare.py
frontend/tests/live/             E   WebKit passes against quark-dev-server
docs/plans/                      F E   perf results, XLSX read rules, exit deviations
```

Streams own files, so units in the same batch never collide. Purely additive edits to shared registration points are the exception: `mod` lines, `routes()` merges in `http/mod.rs`, `[dependencies]` lines, match arms in `features/charts/service.rs`, and the lines a card deletes from `tests/rust_pending.txt`. When those collide, the integrator keeps both sides.

## 5. Schedule

The orchestration map holds the batch table, the review groups, the critical path (15 waves, equal to the 15 batches) and the dependency graph. Summary:

| Batches | Units | Why this order |
|---|---|---|
| 1–7 | R1 … R7, one per batch | The reshape moves files every later unit imports |
| 8 | C1 F1 F2 F6 V1 X1 | Risk first: ICU and the pool start as soon as the layout is final |
| 9 | F3 F7 F8 V2 V7 X2 | |
| 10 | F4 F5 F9 V3 W1 | |
| 11 | C2 J1 V4 V5 V6 W2 | First analysis routes |
| 12 | C3 C5 C6 C7 C8 X3 | |
| 13 | C4 E2 J2 W3 X4 X5 | |
| 14 | E1 E3 E4 W4 (loop) | |
| 15 | E5 | Then the exit gate (section 15) |

The critical path is R1–R7, F2, F3, F4/F5, C2, C3, C4, E3, E5.

**Sprint gates.** Each sprint manager pushes, watches CI and audits the hot spots of its batches' cards.

| Sprint | Batches | Gate |
|---|---|---|
| S1 | 1–2 | CI green |
| S2 | 3–4 | CI green |
| S3 | 5–6 | CI green, including the lint job (R6 touches `src-tauri`) |
| S4 | 7–8 | CI green on all three OSes with ICU (F1); record cold build minutes per OS and the Actions cache total. Dispatch a bundle run (`gh workflow run ci.yml --ref rust-desktop/parity`): installers and self-test must pass on all three OSes, including the universal macOS build. A failing target goes to the Lead (spec §5.4). |
| S5 | 9–10 | CI green; F7's table goes to the human (H0) |
| S6 | 11–12 | CI green; offer H1 |
| S7 | 13–14 | CI green; W4's loop report |
| S8 | 15 | CI green, then section 15 |

## 6. Batch plan

**Group gate:** the **Done when** commands of the group's cards, run again on the integrated base, plus their hot spots.

**Batch gate** (Lead, in the gate worktree, after every batch):
```
rtk cargo test -p quark-core && rtk cargo clippy --workspace --all-targets -- -D warnings && rtk cargo fmt --all --check \
  && rtk cargo build -p quark-core --bin quark-dev-server && QUARK_TEST_BACKEND=rust rtk uv run pytest -q && rtk uv run pytest -q
```
- **Batches 1–7:** the Rust-mode run uses `-m skeleton`, until F6 lands the pending list in batch 8.
- **From batch 6 on:** clippy runs with the frontend built, `npm --prefix frontend run build`, because `src-tauri` embeds it.

**Batch-specific steps:**
- **Batch 6 (R6):** also `cargo build -p quark`, the desktop shell.
- **Batch 8 (F1):**
  - after integrating, rebuild the gate's `target/` (DuckDB is rebuilt with CMake) before batch 9 starts, and record the time;
  - every later worktree clones the rebuilt `target/`.
- **Batch 9 (F7):** F7's timing run happens after the batch's other units are integrated, on an otherwise idle machine. Concurrent cargo builds would skew it.
- **Batch 13 (X5):** the `appender-arrow` feature changes `Cargo.toml`. Warm the gate again.
- **Batch 14 (E1):** E1's release-mode measurements run after W4's loop and the other units are integrated, on an idle machine.

---

## 7. Unit cards: stream R (reshape)

The reshape moves SP1's code into the layout of section 4 without changing behaviour. Its seven units run one after another, each alone in its batch.

**Rules for every R unit:**
- **Moves:** move files with `git mv`, so history follows and reviewers see renames. Change no behaviour. Add no tests; the existing tests move with their code and keep their names.
- **Budget:** counts only edits that are not moves, at 200 lines or fewer. The file list may exceed 3 files where the only change is `use` paths.
- **Review:** the task manager reads `git diff -M --stat` and `git diff -M rust-desktop/parity...HEAD`. Every non-rename hunk must be a path change or one of the card's listed edits.
- **Done when** (the R gate):
  ```
  rtk cargo test -p quark-core && rtk cargo clippy -p quark-core --all-targets -- -D warnings && rtk cargo fmt --all --check && rtk cargo build -p quark-core --bin quark-dev-server && QUARK_TEST_BACKEND=rust rtk uv run pytest -m skeleton -q
  ```
  The integration tests under `core/tests/` must also pass with only import-path edits.

#### R1 · Shared atoms · batch 1
- **Reshape rules:** move files with `git mv`. Change no behaviour. Existing tests move with their code and keep their names. The budget is 200 lines or fewer of edits that are not moves; the file list may exceed 3 where only `use` paths change. Reviewers read `git diff -M`.
- **Files:**
  - **Moves:**
    - `core/src/{sql,literal,ids,naming}.rs` → `core/src/shared/`.
    - `core/src/arrow.rs` → `core/src/shared/encode/arrow.rs`.
  - **Splits:** `core/src/values.rs` becomes:
    - `shared/types.rs`: the `NUMERIC`, `TEXT`, `DATE` and `NATIVE` lists, `has_prefix`, `is_numeric`, `is_text`, `profile_kind`, `is_native`;
    - `shared/encode/cell.rs`: `cell_json` plus its list, map and key helpers;
    - `shared/encode/scalar.rs`: the scalar renderers.
  - **New files:**
    - `shared/columns.rs`: `ColumnMeta` from `query.rs` and `ColumnSummary` from `page.rs`;
    - `shared/mod.rs` and `shared/encode/mod.rs`.
  - **Tests:** each module's inline `mod tests` moves to its sibling `<module>/tests.rs`.
  - **Import paths:** updated in `lib.rs`, `engine.rs`, `mount.rs`, `state.rs`, `page.rs`, `query.rs`, `guard.rs`, `cache/*`, `api/*` and `core/tests/cache_property.rs`.
- **Needs:** none.
- **Build:**
  - `lib.rs` declares `pub mod shared;` and drops the moved modules. The public paths become `quark_core::shared::{sql, literal, ids, naming, types, columns, encode}`.
  - No re-exports of old paths.
  - Every new file opens with a one-line `//!` purpose.
- **Tests first:** none new.
- **Hot spots:** `cell_json` keeps `pub` visibility, and helper visibility is narrowed to `pub(super)` where only `encode` uses it.
- **Done when:** `rtk cargo test -p quark-core && rtk cargo clippy -p quark-core --all-targets -- -D warnings && rtk cargo fmt --all --check && rtk cargo build -p quark-core --bin quark-dev-server && QUARK_TEST_BACKEND=rust rtk uv run pytest -m skeleton -q`.
- **Commit:** `refactor(core): move the SQL, value and id atoms into shared/`

#### R2 · Relation molecules · batch 2
- **Reshape rules:** move files with `git mv`. Change no behaviour. Existing tests move with their code and keep their names. The budget is 200 lines or fewer of edits that are not moves; the file list may exceed 3 where only `use` paths change. Reviewers read `git diff -M`.
- **Files:**
  - **Splits:** `core/src/query.rs` becomes:
    - `relation/request.rs`: `Connector`, `Direction`, `Filter`, `Sort`, `QueryRequest`, `SqlQueryRequest`, their defaults and `validate`;
    - `relation/builder.rs`: `OPERATORS`, `python_str`, `is_bound`, `clause`, `BuiltQuery`, `build_query`.
  - **Moves:** `guard.rs` → `relation/guard.rs`.
  - **From `page.rs`:**
    - `Param` and `bind` → `relation/params.rs`;
    - `PageTarget`, renamed `Target`, plus `Resolved` (fields become `pub(crate)`), `resolve`, `describe_error` and `internal` → `relation/target.rs`.
  - **From `engine.rs`:** `DatasetInfo`, `datasets` and `describe` → `relation/describe.rs`.
  - **New file:** `relation/mod.rs`.
  - **Tests:** move to siblings.
  - **Import paths:** updated in `page.rs`, `engine.rs`, `mount.rs`, `state.rs`, `cache/results.rs`, `api/query.rs`, `core/tests/cache_property.rs` and `core/tests/perf_acceptance.rs`.
- **Needs:** R1.
- **Build:**
  - `PageTarget` becomes `Target` everywhere, including the integration tests.
  - No other renames.
  - `relation/` imports only `shared/` and `error`.
- **Tests first:** none new.
- **Hot spots:** `bind` stays `pub(crate)`; nothing in `relation/` imports `runtime`.
- **Done when:** `rtk cargo test -p quark-core && rtk cargo clippy -p quark-core --all-targets -- -D warnings && rtk cargo fmt --all --check && rtk cargo build -p quark-core --bin quark-dev-server && QUARK_TEST_BACKEND=rust rtk uv run pytest -m skeleton -q`.
- **Commit:** `refactor(core): move request types, the query builder and target resolution into relation/`

#### R3 · Runtime moves · batch 3
- **Reshape rules:** move files with `git mv`. Change no behaviour. Existing tests move with their code and keep their names. The budget is 200 lines or fewer of edits that are not moves; the file list may exceed 3 where only `use` paths change. Reviewers read `git diff -M`.
- **Files:**
  - **Splits:** `engine.rs` becomes:
    - `runtime/engine.rs`: `EngineKind`, `Counters`, `EngineInner`, `Engine`, `open_node`;
    - `runtime/dirs.rs`: `Dirs`, `configure_spill`, `lock_down_paths`, `lock_down_dirs`, `lock_down`.
  - **Moves:**
    - `mount.rs`, `cancel.rs` and `registry.rs` → `runtime/`;
    - `cache/` → `runtime/cache/`.
  - **Cache splits:**
    - `results.rs` becomes `results.rs` (bookkeeping: `ResultAction`, `ResultCache`, `table_name`, `drop_statement`, `is_plain_scan`, eviction) and `result_build.rs` (`run_build`, `finish`, `is_building`, `supersede_builds`, and the body of `start_build`).
    - `columnar.rs` becomes `columnar_key.rs` (`ColumnarKey`, `ColumnarJob`), `columnar.rs` (`ColumnarWorker` and its context) and `columnar_files.rs` (`remove_partial`, `evict`, `remove_stray_partials`).
  - **New file:** `runtime/mod.rs`.
  - **Tests:** move to siblings. Columnar tests split by the file they test.
  - **Import paths:** updated, with `state.rs` still at the crate root.
- **Needs:** R2.
- **Build:** every runtime file stays at 250 lines or fewer. If one doesn't, split it further by responsibility and name the new file in the report.
- **Tests first:** none new.
- **Hot spots:** the columnar test hook (`Hold`) still reaches the worker; `ResultCache`'s `Drop` still joins its build.
- **Done when:** `rtk cargo test -p quark-core && rtk cargo clippy -p quark-core --all-targets -- -D warnings && rtk cargo fmt --all --check && rtk cargo build -p quark-core --bin quark-dev-server && QUARK_TEST_BACKEND=rust rtk uv run pytest -m skeleton -q`.
- **Commit:** `refactor(core): move engines, mounting, cancellation, the registry and caches into runtime/`

#### R4 · Catalog split · batch 4
- **Reshape rules:** move files with `git mv`. Change no behaviour. Existing tests move with their code and keep their names. The budget is 200 lines or fewer of edits that are not moves; the file list may exceed 3 where only `use` paths change. Reviewers read `git diff -M`.
- **Files:** `core/src/state.rs` becomes:
  - `runtime/state.rs`: `Shared`, `AppState`, `load`, `catalog`, `queue_import`, `cache_ready`, `shutdown`, and the error helpers `cannot_open`, `registry_not_saved`, `invalid_value`;
  - `runtime/catalog.rs`: `Catalog` and its helpers, the project and source listing methods, and their summary types;
  - `runtime/engines.rs`: `engine_for_node`, `datasets`, `workspace_in`, `mount_sources`;
  - `runtime/sources.rs`: `upload_path`, `register_upload`, `add_upload`, `delete_source`;
  - `runtime/files.rs`: `remove_cache_files`, `remove_with_retry`, `remove_orphan_uploads`.

  State tests split into the matching siblings.
- **Needs:** R3.
- **Build:**
  - Methods stay on `AppState`, spread over several `impl AppState` blocks.
  - `runtime/mod.rs` declares `pub use state::AppState; pub use dirs::Dirs;`.
  - Widen visibility only to `pub(crate)`, and only where a sibling file needs it.
- **Tests first:** none new.
- **Hot spots:** the catalog lock is still never held while mounting; Windows deletion order (engines dropped before files) is unchanged.
- **Done when:** `rtk cargo test -p quark-core && rtk cargo clippy -p quark-core --all-targets -- -D warnings && rtk cargo fmt --all --check && rtk cargo build -p quark-core --bin quark-dev-server && QUARK_TEST_BACKEND=rust rtk uv run pytest -m skeleton -q`.
- **Commit:** `refactor(core): split app state into runtime catalog, engines, sources and files`

#### R5 · Projects and sources features · batch 5
- **Reshape rules:** move files with `git mv`. Change no behaviour. Existing tests move with their code and keep their names. The budget is 200 lines or fewer of edits that are not moves; the file list may exceed 3 where only `use` paths change. Reviewers read `git diff -M`.
- **Files:**
  - **New:** `features/mod.rs`, `features/projects/{mod,model,service,routes}.rs`, `features/sources/{mod,model,service,uploads,routes}.rs`.
  - **Changed:** `runtime/catalog.rs`, `runtime/sources.rs`, `api/mod.rs` (merge the new `routes()`).
  - **Removed:** `api/read.rs` and `api/sources.rs`; their handlers move into the features.
- **Needs:** R4.
- **Build:**
  - **Response types:** `ProjectSummary` moves into `features/projects/model.rs`, along with `ProjectCreate` from `api/read.rs`. `SourceSummary`, `SourceDetail`, `LegacyNode` and `DatasetEntry` move into `features/sources/model.rs`.
  - **Rules to services.** The rules move from runtime into the feature services:
    - the project-name rule and the new project record (`uuid4` hex id, `project_<id>` node id, trimmed name);
    - the upload extension check (`supported_extension`), the uploaded name (split on `/` and `\`), and the dataset name;
    - the `SourceRecord` and raw registry record for an upload, and the public upload JSON (project-scoped or legacy shape).
  - **Runtime primitives.** Runtime keeps the state operations, exposed as `pub(crate)` methods on `AppState`:
    - `save_project(&self, project: ProjectRecord) -> ApiResult<ProjectRecord>` writes `projects.json` and pushes the project;
    - `register_source(&self, source: SourceRecord, record: RawRecord, engine: Engine) -> ApiResult<()>` does what `add_upload` did after opening the engine;
    - `open_node_engine(&self, source: &SourceRecord) -> ApiResult<Engine>` maps a failure to 400 `Could not open source: …`;
    - `catalog()`, `mount_sources()`, `engine_for_node()`, `delete_source()`, `upload_path()`.
  - **Service functions:**
    - `features::projects::service::{list_projects(&AppState) -> Vec<ProjectSummary>, create_project(&AppState, &str) -> ApiResult<ProjectSummary>}`;
    - `features::sources::service::{project_sources, source_detail, project_views, legacy_nodes, datasets}`, with SP1's signatures moved off `AppState`;
    - `features::sources::uploads::{store, register_upload}`.
  - **Routes:** each feature exposes `pub fn routes() -> Router<AppState>` with SP1's paths, methods and status codes unchanged.
- **Tests first:** none new. State tests that exercise a rule move with it into the feature's `tests.rs`.
- **Hot spots:**
  - upload clean-up on every failure path;
  - the body limit is disabled only on the two upload routes;
  - `runtime/` imports nothing from `features/`.
- **Done when:** `rtk cargo test -p quark-core && rtk cargo clippy -p quark-core --all-targets -- -D warnings && rtk cargo fmt --all --check && rtk cargo build -p quark-core --bin quark-dev-server && QUARK_TEST_BACKEND=rust rtk uv run pytest -m skeleton -q`.
- **Commit:** `refactor(core): move project and source rules into features/`

#### R6 · Pages feature and the http layer · batch 6
- **Reshape rules:** move files with `git mv`. Change no behaviour. Existing tests move with their code and keep their names. The budget is 200 lines or fewer of edits that are not moves; the file list may exceed 3 where only `use` paths change. Reviewers read `git diff -M`.
- **Files:**
  - **Pages feature:** the rest of `page.rs` becomes `features/pages/{mod,model,queries,service,routes}.rs`:
    - `model.rs`: `Format`, `PageMeta`, `PageBody`;
    - `queries.rs`: `compute_stats`, `fetch_page`;
    - `service.rs`: `run_page`, `rows_json`;
    - `routes.rs`: from `api/query.rs`.
  - **http layer:**
    - `api/mod.rs` becomes `http/mod.rs` (`router`, `log_request`, `guarded`, `unknown_route`, `panic_response`) and `http/deps.rs` (`blocking`, `ApiJson`, plus the new extractors below);
    - `secure.rs` → `http/secure.rs`.
  - **`lib.rs`:** the final module list.
  - **Callers:** `src-tauri/src/lib.rs`, `core/src/bin/quark-dev-server.rs` and `core/tests/{cancellation,cache_property,perf_acceptance,dev_server}.rs` get updated import paths.
  - **Removed:** `api/`.
- **Needs:** R5.
- **Build:**
  - **`lib.rs`:** `pub mod error; pub mod shared; pub mod relation; pub mod runtime; pub mod features; pub mod http; pub use http::router;`.
  - **New extractors in `http/deps.rs`:**
    - `Format`: Arrow when any `Accept` value contains `application/vnd.apache.arrow.stream`, otherwise JSON. This is `wants_arrow` moved.
    - `Project(ProjectRecord)`: resolves `{project_id}` from `RawPathParams`, or 404 `Project not found`.
    - `EngineFor(Arc<Engine>)`: resolves `{node_id}` from `RawPathParams` through `AppState::engine_for_node`, inside `blocking`.
  - **Pages routes:** take `EngineFor` and `Format`. The guard and ticket code stays in the pages route until F4.
- **Tests first:**
  - `format_prefers_arrow_when_accepted`;
  - `engine_for_unknown_node_is_404`;
  - `project_for_unknown_id_is_404`.

  These go in `http/deps/tests.rs` and are the only new tests in stream R.
- **Hot spots:**
  - Every route path and method is unchanged.
  - Logging still never prints query strings.
  - The tauri shell compiles.
- **Done when:** `rtk cargo test -p quark-core && rtk cargo clippy -p quark-core --all-targets -- -D warnings && rtk cargo fmt --all --check && rtk cargo build -p quark-core --bin quark-dev-server && QUARK_TEST_BACKEND=rust rtk uv run pytest -m skeleton -q`, then `npm --prefix frontend ci && npm --prefix frontend run build && rtk cargo clippy --workspace --all-targets -- -D warnings`.
- **Commit:** `refactor(core): move pages into features/ and routing into http/`

#### R7 · Architecture test and the unused generation · batch 7
- **Reshape rules:** move files with `git mv`. Change no behaviour. Existing tests move with their code and keep their names. The budget is 200 lines or fewer of edits that are not moves; the file list may exceed 3 where only `use` paths change. Reviewers read `git diff -M`.
- **Files:** `core/tests/architecture.rs` (new), plus the generation removal across `runtime/engine.rs`, `runtime/engines.rs`, `runtime/state.rs`, `runtime/mount.rs` and their `tests.rs`. Add a missing `//!` line to any file the new test flags.
- **Needs:** R6.
- **Build:**
  - **Architecture test** (standard library only). It walks `core/src` and fails when:
    - a production file passes 250 lines, or a `tests.rs` passes 400;
    - a file under `shared/` has `use crate::{relation|runtime|features|http}`;
    - a file under `relation/` has `use crate::{runtime|features|http}`;
    - a file under `runtime/` has `use crate::{features|http}`;
    - a `queries.rs` or a feature service file (any non-`routes.rs`, non-`model.rs` file under `features/`) has `use axum` or `use crate::http`;
    - a file under `features/<a>/` has `use crate::features::<b>` with `b` different from `a`;
    - a `routes.rs` has a string literal containing `SELECT `;
    - a file's first line is not `//!`.

    Each failure message names the file and the rule.
  - **Generation removal** (deviation 13): remove `Engine::generation`, `Shared::next_generation`, and the `generation` parameter of `Engine::new`, `open_node` and `open_workspace`.
- **Tests first:**
  - `production_files_stay_under_250_lines`;
  - `test_files_stay_under_400_lines`;
  - `layers_import_only_downward`;
  - `features_do_not_import_each_other`;
  - `routes_hold_no_sql`;
  - `every_file_states_its_purpose`;
  - `shell_layers_cannot_reach_features`.

  Run them against R6's tree first and confirm that whatever fails is fixable with path-only or `//!` edits.
- **Hot spots:** the import scan must also catch multi-line and grouped `use crate::{…}` statements.
- **Done when:** `rtk cargo test -p quark-core && rtk cargo clippy -p quark-core --all-targets -- -D warnings && rtk cargo fmt --all --check && rtk cargo build -p quark-core --bin quark-dev-server && QUARK_TEST_BACKEND=rust rtk uv run pytest -m skeleton -q`, plus `rtk cargo test -p quark-core --test architecture`.
- **Commit:** `test(core): enforce the module layout and drop the unused engine generation`

## 8. Unit cards: stream F (foundations)

#### F1 · ICU in DuckDB · batch 8
- **Files:** `Cargo.toml`, `Cargo.lock`, `core/tests/time_zones.rs` (new).
- **Needs:** R7. Pre-flight item P2 (CMake installed) must be done.
- **Build:**
  - The workspace `duckdb` features become `["bundled", "parquet", "json", "icu"]`. `icu` turns on `bundled-cmake`, so DuckDB now builds with CMake but is still compiled into the binary.
  - Nothing else changes. `engine_version` must still report `v1.5.4`.
  - The unit report records the build costs before and after:
    - wall time of a clean `cargo build -p quark-core` (with `jobs = 4`);
    - the size of `target/release/quark-dev-server`.
- **Tests first** (`time_zones.rs`; each test opens an in-memory connection):
  - `icu_is_loaded`: `SELECT loaded FROM duckdb_extensions() WHERE extension_name = 'icu'` is `true`.
  - `session_zone_reads_offsetless_timestamptz`: after `SET TimeZone = 'Asia/Dubai'`, `TIMESTAMPTZ '2024-01-01 10:00:00' = TIMESTAMPTZ '2024-01-01 06:00:00+00'` is `true`.
  - `date_trunc_and_strftime_use_the_session_zone` (Review Focus 4): after `SET TimeZone = 'America/New_York'`:
    - `strftime(date_trunc('day', TIMESTAMPTZ '2024-03-10 03:30:00+00'), '%Y-%m-%d')` is `2024-03-09`;
    - `strftime(TIMESTAMPTZ '2024-01-01 02:00:00+00', '%Y')` is `2023`.
  - `default_zone_is_set`: `current_setting('TimeZone')` is not empty.
- **Hot spots:** only the feature list changes; universal macOS builds both architectures through CMake (the sprint gate checks this).
- **Done when:** `rtk cargo test -p quark-core --test time_zones --test engine_version && rtk cargo test -p quark-core`.
- **Commit:** `build(core): compile DuckDB with ICU`

#### F2 · Reader pool · batch 8
- **Files:** `core/src/runtime/pool.rs` (new), `core/src/runtime/pool/tests.rs` (new), `core/src/runtime/mod.rs`.
- **Needs:** R7.
- **Build:**
  - `pub fn reader_count() -> usize` returns `reader_count_for(available_parallelism)`, where `reader_count_for(cores) = (cores / 2).clamp(2, 4)`.
  - `pub struct Reader { pub conn: Connection, interrupt: Arc<InterruptHandle> }`, with `pub fn interrupt_handle(&self) -> Arc<InterruptHandle>`.
  - `pub struct ReaderPool { idle: Mutex<Vec<Reader>>, freed: Condvar }`:
    - `ReaderPool::new(primary: &Connection, size: usize) -> duckdb::Result<Self>` clones every reader with `primary.try_clone()`;
    - `pub fn lease(&self) -> Lease<'_>` blocks until a reader is idle.
  - `Lease` derefs to `Reader` and returns it to the pool on drop, including while unwinding. A poisoned mutex is recovered.
- **Tests first:**
  - `reader_count_is_half_the_cores_within_bounds`: 1 → 2, 4 → 2, 10 → 4, 64 → 4.
  - `two_leases_query_at_the_same_time`: two threads lease and run a query of about 300 ms; both start before either finishes.
  - `lease_waits_while_every_reader_is_busy`: with a pool of 2, a third lease returns only after one drops.
  - `readers_inherit_the_lock_down`: after `lock_down_paths` on the primary, a reader cannot `read_csv_auto` a file outside the list, and `SET enable_external_access = true` fails on it.
  - `readers_see_views_created_later_on_the_primary`.
- **Hot spots:** the lease is returned on panic; no reader is created after lock-down with different settings.
- **Done when:** `rtk cargo test -p quark-core runtime::pool`.
- **Commit:** `feat(core): add a pool of DuckDB reader connections`

#### F3 · Engines read through the pool · batch 9
- **Files:** `core/src/runtime/engine.rs`, `core/src/runtime/cancel.rs`, `core/src/runtime/mount.rs`, `core/src/features/pages/service.rs`, `core/src/features/pages/routes.rs`, `core/src/features/sources/service.rs`, and their `tests.rs`. The budget is 6 files and about 250 lines, because every caller of `Engine::lock` changes.
- **Needs:** F2.
- **Build:**
  - **`Engine` fields:**
    - `primary: Mutex<Primary>`, with `Primary { conn, mounted }`;
    - `pool: ReaderPool`, sized by `reader_count()`;
    - `stats: Mutex<StatsCache>` and `results: Mutex<ResultCache>`;
    - `counters: Counters`, with an `AtomicU64` count;
    - `active: Mutex<HashMap<u64, Arc<InterruptHandle>>>`;
    - `next_ticket` and `caches`.
  - **`Engine::primary(&self) -> MutexGuard<Primary>`** replaces `lock()`. It is used only for catalog changes: mounting, attaching, dropping result tables.
  - **`Engine::read<T>(&self, ticket: Option<&Ticket>, work: impl FnOnce(&Connection) -> T) -> Result<T, Cancelled>`:**
    - It leases a reader.
    - With a ticket, it checks under `active` whether the ticket is cancelled. If so it returns `Cancelled`; otherwise it registers `ticket id → reader interrupt handle`.
    - It runs `work`, then unregisters the ticket.
  - **`CancelGuard::drop`** interrupts only the handle registered for its own ticket, and keeps re-interrupting until that ticket is unregistered (SP1's loop).
  - **Pages:**
    - `run_page(engine: &Engine, conn: &Connection, target, request, format)` reads and writes the caches through the engine's locks, one entry at a time.
    - `is_plain_scan` gets a snapshot of the mounted views.
    - Evicted result tables are dropped through `engine.primary()`.
    - The pages route calls `engine.read(Some(&ticket), …)`.
  - **Datasets listing** (`features/sources/service.rs`) reads through `engine.read(None, …)`.
- **Tests first:**
  - `pages_of_one_engine_run_concurrently`: two threads each read a query of about 300 ms on one engine, and they overlap.
  - `cancel_interrupts_only_its_own_reader`: two slow reads; dropping one guard stops that query, and the other completes.
  - `cache_locks_are_free_during_a_query`: while a slow read runs, `stats` and `results` can each be locked within 50 ms.

  SP1's page, cancellation and cache tests must pass unchanged apart from call signatures.
- **Hot spots:**
  - No cache lock is held while a query runs.
  - Mounting still happens once per source.
  - A result build still clones the primary.
- **Done when:** `rtk cargo test -p quark-core`.
- **Commit:** `feat(core): run engine queries on pooled readers with per-reader cancellation`

#### F4 · The `Reader` dependency · batch 10
- **Files:** `core/src/http/deps.rs`, `core/src/features/pages/routes.rs`, `core/tests/read_pool.rs` (new).
- **Needs:** F3.
- **Build:**
  - **`Reader` extractor.** It resolves the engine as `EngineFor` does and holds a `CancelGuard`, with two methods:
    - `run<T>(self, work: impl FnOnce(&Engine, &Connection) -> ApiResult<T>) -> ApiResult<T>` (async) runs `work` in `spawn_blocking` through `engine.read(Some(&ticket), …)`. Dropping the request future drops the guard and interrupts only that reader. `Cancelled` maps to the same response as SP1's page route.
    - `run_to_end(…)` does the same without a ticket, for exports and join previews.
  - **Pages routes** take `Reader` and `Format`. The guard and ticket code moves out of them into `Reader`.
- **Tests first** (`read_pool.rs`; drives the router over TCP like `core/tests/cancellation.rs`):
  - `page_is_served_while_a_slow_query_runs`: a `/sql` request running about 2 s, and a page request on the same node that completes in under 300 ms.
  - `dropped_request_frees_only_its_reader`.
  - `burst_of_requests_with_drops_never_deadlocks` (Review Focus 1):
    - 12 concurrent requests on one node (pages and slow `/sql`), half of them dropped mid-flight;
    - every request that isn't dropped completes;
    - a fresh page then completes in under 1 s.
- **Hot spots:** stress tests use 4 copies at most; the guard is created before `spawn_blocking`.
- **Done when:** `rtk cargo test -p quark-core --test read_pool --test cancellation`.
- **Commit:** `feat(core): add the Reader dependency and serve pages beside slow queries`

#### F5 · Response cache · batch 10
- **Files:** `core/src/runtime/cache/responses.rs` (new), `core/src/runtime/cache/responses/tests.rs` (new), `core/src/runtime/engine.rs`.
- **Needs:** F3, V1 (`Relation`).
- **Build:**
  - **`ResponseCache`:** an LRU of 128 `serde_json::Value` entries.
  - **`Engine.source_files: Vec<PathBuf>`,** set when the engine opens:
    - a node engine: its source file;
    - a workspace: every source file of the project, plus the `.wal` of each DuckDB file.
  - **`fingerprint(path) -> Option<Fingerprint>`:** size, modified time in nanoseconds, and on Unix the inode and change time. A missing source file makes the response uncacheable.
  - **`is_cacheable(engine, conn, relation_sql, params_json) -> duckdb::Result<bool>`.** It follows Python's `profile()`: take the set of lowercase identifier words of `relation_sql + " " + params_json`. The response is not cacheable when:
    - the words include one of `current_timestamp`, `current_time`, `localtimestamp`, `localtime`, `tablesample` or `sample`;
    - the words include a name from `SELECT DISTINCT function_name FROM duckdb_functions() WHERE stability != 'CONSISTENT' OR (function_type IN ('table', 'macro', 'table_macro') AND function_name != 'query')` (computed once per engine);
    - the engine has stored views in a file-backed database (`duckdb_views()` joined with `duckdb_databases()` where `path IS NOT NULL` and the view is not internal);
    - `params_json` matches `\bquery\b` or `\.(?:csv|tsv|parquet|jsonl?|ndjson|xlsx|duckdb|db)\b`, case-insensitive.
  - **`Engine::cached_response(&self, conn, kind: &str, relation: &Relation, detail: &Value, compute: impl FnOnce() -> ApiResult<Value>) -> ApiResult<Value>`:**
    - The key is the kind, the relation SQL, the parameters' compact JSON, the detail JSON and the fingerprints.
    - On a hit, an `elapsed_ms` field is replaced by the hit's own elapsed time, rounded to 3 decimals.
    - Errors are never cached.
- **Tests first** (ported from Python's `python_only` stats-cache tests; each drives `cached_response` on a real engine and counts calls to `compute`):
  - `repeat_requests_reuse_the_response`;
  - `filters_columns_and_sql_get_their_own_entries`;
  - `in_place_edit_of_the_source_misses` (Review Focus 2): rewrite the CSV with the same length;
  - `volatile_and_dynamic_sql_is_never_cached`: `random()`, `current_timestamp`, `USING SAMPLE 1 ROWS`, nested `query('…')`;
  - `stored_views_and_macros_in_attached_databases_disable_caching`;
  - `direct_file_scans_are_never_cached`;
  - `least_recently_used_entry_goes_at_128`.
- **Hot spots:** the key excludes sorts and paging; the fingerprints cover every file the engine reads.
- **Done when:** `rtk cargo test -p quark-core runtime::cache::responses`.
- **Commit:** `feat(core): cache analysis responses per engine`

#### F6 · Pending list and markers · batch 8
- **Files:** `tests/conftest.py`, `tests/rust_pending.txt` (new), `pyproject.toml`, `.github/workflows/ci.yml`, `tests/test_backend.py` (marker lines only).
- **Needs:** R7.
- **Build:**
  - **`rust_only` marker:** registered in `pyproject.toml` and `conftest.py`, and skipped unless `QUARK_TEST_BACKEND=rust`.
  - **Pending list:**
    - In Rust mode, a test whose function name appears in `tests/rust_pending.txt` (one name per line, `#` comments allowed) is marked `xfail(strict=True, reason="not ported to Rust yet")`.
    - Generate the list on the integrated base with `QUARK_TEST_BACKEND=rust uv run pytest -q --tb=no -rf -p no:cacheprovider`, recording every failing function name. The planning run on 2026-10-13 saw 66.
  - **Audit:** a test that calls a Python function directly instead of going through HTTP is marked `python_only`, and the report names the Rust test that covers the same rule. For example, `test_safe_serializes_unsafe_aggregate_integers` calls `safe()` and `page_count()`; the Rust coverage is the shared encode tests and `naming::page_count` tests.
  - **CI:** the `core` job runs `uv run pytest -q` with `QUARK_TEST_BACKEND: rust`, dropping `-m skeleton`.
- **Tests first:** with an empty list, the Rust-mode run shows the failures; with the list, it is green.
- **Hot spots:** names match whole function names, and parametrized cases share their function's line; Python mode is unaffected.
- **Done when:** `rtk cargo build -p quark-core --bin quark-dev-server && QUARK_TEST_BACKEND=rust rtk uv run pytest -q && rtk uv run pytest -q`.
- **Commit:** `test(tests): track unported tests in a strict pending list`

#### F7 · Performance comparison · batch 9
- **Files:** `tests/perf/compare.py` (new), `docs/plans/2026-10-13-rust-desktop-parity-perf.md` (new).
- **Needs:** R7.
- **Build:**
  - **The script** runs under `uv run python tests/perf/compare.py [--python] [--rust]`.
  - **Test data:** it writes the 2M-row CSV with Python DuckDB, using the same `COPY` SQL as `write_csv` in `core/tests/perf_acceptance.rs`, into a temporary folder.
  - **Servers:**
    - Python: `uv run uvicorn backend.app:app` with `QUARK_DATA_DIR` set to a temporary folder.
    - Rust: `target/release/quark-dev-server --data-dir <tmp> --cache-dir <tmp>/cache --port 0`.
    - Each server gets a free port.
  - **Setup:** upload the CSV. For Rust, keep requesting the page at row 1,000,000 until its time settles, which means the columnar import has landed.
  - **Cases:** each spec §9 case runs 5 times, and the script prints the median per backend as a markdown table. A route that answers 501 prints `n/a`.
  - **"Page during export":** start a CSV export of all rows, then time a page at row 1,000,000 while it runs.
  - **Output:** the script runs against Python now, and Rust where its routes exist. It writes the table, the date, the commit and the machine to the results doc.
- **Tests first:** none. The script's own run is the check.
- **Hot spots:** servers are always stopped and temporary folders always removed, even on failure; nothing is written outside temporary folders.
- **Done when:** `rtk uv run python tests/perf/compare.py --python --rust` runs end to end and the results doc holds the table.
- **Commit:** `test(perf): compare Python and Rust route timings`

#### F8 · Workbook reading rules and corpus · batch 9
- **Files:** `tests/workbook_corpus.py` (new), `tests/test_workbook_corpus.py` (new), `docs/plans/2026-10-13-xlsx-read-rules.md` (new, 150 lines or fewer). Also add the new test names to `tests/rust_pending.txt`.
- **Needs:** F6.
- **Build:**
  - **`workbook_corpus.py`** builds workbooks with `openpyxl` (already a dependency), one function per case:
    1. integers and floats;
    2. styled dates, datetimes and times;
    3. booleans;
    4. identifiers with leading zeros stored as text;
    5. a column that is less than 90% numeric;
    6. empty cells, and an empty row in the middle;
    7. a first row that is numeric, so there is no header;
    8. a header-only sheet;
    9. duplicate header names;
    10. sheet names with an apostrophe and with non-ASCII letters;
    11. a 50-column sheet;
    12. text with formula-like content (`=1+1` stored as text).
  - **`test_workbook_corpus.py`.** One parametrized test per case uploads the workbook, confirms its sheets, and asserts:
    - the column names;
    - the DuckDB types from the `/query` metadata;
    - every row.

    The expected values come from running the Python backend. The tests pass in Python mode and are pending in Rust mode.
  - **The rules doc** records what `read_xlsx` does, observed with a throwaway probe that is not committed:
    - header detection and generated names (a numeric first row gives `A1`-style names);
    - duplicate names;
    - empty rows and cells;
    - the text each cell type becomes under `all_varchar = true` (integers, floats, dates as serials, booleans);
    - the declared type `read_xlsx` gives without `all_varchar` (DATE, TIMESTAMP, TIME or other) and what decides it.

    W1 builds against this doc.
- **Tests first:** the corpus tests themselves, green in Python mode.
- **Hot spots:** expected values come from Python's actual output, not from hand-written guesses; formulas with cached values are out of the corpus, because `openpyxl` cannot write cached results (the doc notes this).
- **Done when:** `rtk uv run pytest tests/test_workbook_corpus.py -q` passes, and in Rust mode they show as `xfail`.
- **Commit:** `test(tests): pin Python's workbook reading with a corpus`

#### F9 · TIMETZ keeps its offset · batch 10
- **Files:** `core/src/relation/rows.rs`, `core/src/features/pages/queries.rs`, `core/src/shared/encode/scalar.rs`, and their `tests.rs`.
- **Needs:** V1, F3.
- **Build** (deviation 4, spec §5.4). DuckDB drops a TIMETZ value's offset when it exports to Arrow, so these values go out as text instead:
  - `pub fn select_list(columns: &[ColumnMeta]) -> String` in `relation/rows.rs` returns:
    - `*` when no column is `TIME WITH TIME ZONE`;
    - otherwise `* REPLACE (CAST("c" AS VARCHAR) AS "c", …)`.
  - Every query that returns cells to the client selects through it:
    - the page fetch in `features/pages/queries.rs`;
    - `json_rows`;
    - from X3 on, the export readers.
  - `shared/encode/scalar.rs` renders a TIMETZ cell that arrives as text (`HH:MM:SS[.ffffff]±HH[:MM]`, DuckDB's form) as Python's `isoformat()`: `HH:MM:SS[.ffffff]±HH:MM`. Microseconds appear only when they are not zero.
  - Column metadata keeps the type name `TIME WITH TIME ZONE`.
- **Tests first:**
  - `timetz_keeps_its_offset_in_json_pages`;
  - `timetz_keeps_its_offset_in_arrow_pages`, where TIMETZ is a JSON-fallback column;
  - `timetz_text_renders_like_python`: `+02` becomes `+02:00`, `+05:30` stays `+05:30`, and microseconds stay;
  - `select_list_is_star_without_timetz`.
- **Hot spots:** native Arrow columns stay native; only TIMETZ columns are cast.
- **Done when:** `rtk cargo test -p quark-core --lib -- relation::rows features::pages shared::encode`.
- **Commit:** `fix(core): keep TIMETZ offsets in pages and analysis rows`

## 9. Unit cards: stream V (stats, values, find, attach)

#### V1 · Filtered relations and JSON rows · batch 8
- **Files:** `core/src/relation/filtered.rs` (new), `core/src/relation/rows.rs` (new), `core/src/relation/mod.rs`, `core/src/shared/types.rs`, and the siblings `filtered/tests.rs` and `rows/tests.rs`.
- **Needs:** R7.
- **Build:**
  - `pub struct Relation { pub sql: String, pub params: Vec<Value>, pub columns: Vec<ColumnMeta> }`. `sql` is the filtered relation `(SELECT * FROM <table> WHERE … QUALIFY …)`. `params` holds the target's lead parameters (the SQL text for an SQL target), then the filter values.
  - `pub fn filtered(conn: &Connection, target: &Target, request: &QueryRequest) -> ApiResult<Relation>`:
    - runs `resolve`, then `build_query` with the request's sorts removed;
    - sorts are never validated, because Python's `filtered_relation` ignores them;
    - filters and dedupe are validated with SP1's messages.
  - `pub fn json_rows(conn: &Connection, sql: &str, params: &[Value]) -> duckdb::Result<(Vec<ColumnMeta>, Vec<Vec<Value>>)>`:
    - takes the result's types from `DESCRIBE SELECT * FROM (<sql>)`;
    - runs the query with `query_arrow`;
    - converts every cell with `cell_json(…, &chrono::Local)`, which applies Python's `safe()` rules.
  - `shared::types::is_integer(type_name) -> bool` follows Python's `INTEGER` prefixes.
- **Tests first:**
  - `dataset_target_has_no_lead_params`;
  - `sql_target_leads_with_its_sql`;
  - `unknown_sort_column_is_ignored`;
  - `duplicate_dedupe_key_is_422`;
  - `json_rows_follow_safe_rules`: a HUGEINT above 2^53 becomes a string, NaN becomes null, a DATE becomes ISO text;
  - `is_integer_matches_python`.
- **Hot spots:** parameter order (the lead comes first); type names come from DuckDB, not Arrow.
- **Done when:** `rtk cargo test -p quark-core relation::`.
- **Commit:** `feat(core): build filtered relations and JSON rows for analysis routes`

#### V2 · Categorical and date profiles · batch 9
- **Files:** `core/src/relation/profile.rs` (new: shared helpers and `pub mod` lines), `core/src/relation/profile/categorical.rs` (new), `core/src/relation/profile/date.rs` (new), `core/src/relation/profile/tests.rs` (new), `core/src/relation/mod.rs`.
- **Needs:** V1.
- **Build:**
  - **Helpers:**
    - `pub fn bin_count(non_null: u64) -> u64` is `min(20, max(1, ceil(sqrt(non_null))))`.
    - `pub fn null_fraction(nulls: u64, rows: u64) -> f64` is 0.0 when `rows` is 0.
  - **`categorical::profile(conn, relation: &Relation, column: &ColumnMeta) -> duckdb::Result<Value>`:**
    - Python's categorical branch: `count(*)`, `count(c)`, nulls, `count(DISTINCT c)`, then the top 20 values ordered by `count DESC, value`.
    - Returns `{kind, type, row_count, non_null_count, null_count, null_fraction, distinct_count, top_values: [{value, count}]}`.
  - **`date::profile(…)`:**
    - Python's date branch: the aggregates plus `min` and `max`.
    - When `non_null > 0`, an `ntile(bin_count)` histogram of `{lower, upper, count}`.
    - `year_counts` from `strftime(c, '%Y')`, except when the uppercase type starts with `TIME`. That includes `TIMESTAMP…`, which Python also leaves empty; keep that.
  - **Values:** every value goes through `json_rows`.
  - **Dispatch:** there is none; the stats service (V4) dispatches.
- **Tests first:**
  - `categorical_top_values_order_and_limit`;
  - `date_bins_follow_the_square_root_rule`;
  - `year_counts_only_for_dates`: `DATE` has year counts; `TIME` and `TIMESTAMP` get `[]`;
  - `empty_relation_profiles_to_zeros`.
- **Hot spots:** the `TIME` prefix quirk; `null_fraction` is a float even when it is 0.
- **Done when:** `rtk cargo test -p quark-core relation::profile`.
- **Commit:** `feat(core): profile categorical and date columns`

#### V3 · Numeric profile · batch 10
- **Files:** `core/src/relation/profile/numeric.rs` (new), `core/src/relation/profile/numeric/tests.rs` (new), `core/src/relation/profile.rs` (`pub mod numeric;`).
- **Needs:** V2.
- **Build:** `numeric::profile(conn, relation, column) -> duckdb::Result<Value>` ports Python's numeric branch exactly.
  - **Aggregates:** count, non-null, nulls, then `count`, `min`, `max`, `avg`, `stddev_samp` and `quantile_cont` at 0.25, 0.5 and 0.75, each `FILTER (WHERE isfinite(c::DOUBLE))`.
  - **Histogram**, when there are finite values:
    - If `is_integer(type)` and either bound's magnitude passes 2^53 − 1: `ntile` bins with string bounds through `safe()`.
    - Else if min equals max: one bin `{lower: min, upper: max, count: finite_count}`.
    - Else equal width: `width = (max − min) / bin_count` in f64. Bin `i` has lower `min + i·width` and upper `min + (i+1)·width`, except the last, whose upper is `max`. Counts come from `least(bin_count − 1, floor((c − ?) / ?)::INTEGER)`, binding `bin_count − 1`, min and width before the relation's parameters.
  - **DECIMAL** bounds become f64.
  - Returns `{kind, type, row_count, non_null_count, null_count, null_fraction, min, max, mean, stddev, p25, median, p75, histogram}`.
- **Tests first:**
  - `float_bins_match_python_edges`;
  - `single_value_column_has_one_bin`;
  - `huge_integers_use_ntile_bins_with_string_bounds`;
  - `non_finite_values_are_left_out_of_finite_stats`;
  - `decimal_bounds_become_floats`.
- **Hot spots:** f64 operation order matches Python's, so bounds compare equal after JSON parsing; parameter order in the bucket query.
- **Done when:** `rtk cargo test -p quark-core relation::profile`.
- **Commit:** `feat(core): profile numeric columns`

#### V4 · Stats routes · batch 11
- **Files:** `core/src/features/stats/{mod,service,routes}.rs` (new), `core/src/features/stats/tests.rs` (new), `core/src/http/mod.rs` (merge routes).
- **Needs:** V3, F4, F5.
- **Build:**
  - **Routes:**
    - `GET` and `POST /api/nodes/{node_id}/datasets/{dataset}/columns/{column}/stats`. GET uses `QueryRequest::default()`; POST accepts an empty body as the default.
    - `POST /api/nodes/{node_id}/sql/columns/{column}/stats` with an `SqlQueryRequest`.
    - Both take a cancellable `Reader`.
  - **`service::stats(engine, conn, target, request, column) -> ApiResult<Value>`:**
    1. `relation::filtered`, so dataset 404s and filter or dedupe 422s come first.
    2. A missing column is 404 `Column not found`; `profile_kind` of `None` is 422 `Column cannot be profiled`.
    3. Dispatch to `categorical`, `date` or `numeric`.
    4. Go through `engine.cached_response(conn, "stats", &relation, &json!(column), …)`.
  - **Errors:** a DuckDB error while profiling is 422 `Invalid filter value: <err>` on both routes, as Python's global handler does.
- **Tests first:**
  - `error_precedence_matches_python`: an invalid filter on a missing column gives 422, not 404;
  - `get_equals_post_with_empty_body`;
  - `sorts_and_paging_share_one_cache_entry`.
- **pytest it makes pass** (delete these lines from `tests/rust_pending.txt`):
  - `test_numeric_stats_histogram_and_json_safe_values`;
  - `test_profile_kinds_and_categorical_and_date_stats`;
  - `test_time_profile_has_no_year_breakdown`;
  - `test_filtered_deduped_query_metadata_and_stats_share_rows`;
  - `test_stats_invalid_filter_value_returns_422`.
- **Hot spots:** the dataset POST accepts both no body and `{}`; the cache detail is the column name.
- **Done when:** `rtk cargo test -p quark-core features::stats && rtk cargo build -p quark-core --bin quark-dev-server && QUARK_TEST_BACKEND=rust rtk uv run pytest -q`.
- **Commit:** `feat(core): serve column stats`

#### V5 · Category values · batch 11
- **Files:** `core/src/features/values/{mod,service,queries,routes}.rs` (new), `core/src/features/values/tests.rs` (new), `core/src/http/mod.rs`.
- **Needs:** V1, F4, F5.
- **Build:**
  - **Routes:**
    - `GET /api/nodes/{node_id}/datasets/{dataset}/columns/{column}/values`, with no body and so no filters;
    - `POST /api/nodes/{node_id}/sql/columns/{column}/values`, with an `{sql}` body and no filters;
    - both take query parameters `search` (default `""`), `offset` (`>= 0`, default 0) and `limit` (1 to 500, default 200); anything else is 422.
  - **Checks:** a missing column is 404 `Column not found`; a column whose uppercase type does not start with `VARCHAR`, `CHAR` or `TEXT` is 422 `Column is not text`.
  - **Query:** the condition is `c IS NOT NULL AND contains(lower(c), lower(?))`.
    - The page orders by `count DESC, c`, with `LIMIT ? OFFSET ?`.
    - The total, `count(DISTINCT c)`, comes from the same scan through a window over the grouped rows. When the page is empty, a separate count supplies it.
  - **Response:** `{values: [{value, count}], total, offset, limit, has_more: offset + rows < total}`.
  - **Cache and cancellation:** responses go through the response cache with kind `values`, and the reader is cancellable.
- **Tests first:**
  - `values_are_counted_and_ordered`;
  - `search_is_case_insensitive`;
  - `paging_reports_has_more`;
  - `empty_page_past_the_end_still_reports_the_total`;
  - `non_text_column_is_422`.
- **pytest it makes pass:** `test_category_values_are_distinct_counted_safe_and_text_only`, `test_category_values_are_paged_and_searchable`.
- **Done when:** `rtk cargo test -p quark-core features::values && rtk cargo build -p quark-core --bin quark-dev-server && QUARK_TEST_BACKEND=rust rtk uv run pytest -q`.
- **Commit:** `feat(core): serve category values`

#### V6 · Find · batch 11
- **Files:** `core/src/features/find/{mod,model,service,routes}.rs` (new), `core/src/features/find/tests.rs` (new), `core/src/runtime/cache/numbered.rs` (new), `core/src/http/mod.rs`.
- **Needs:** V1, F4.
- **Build:**
  - **Request:** `CellSearch { sql, term, columns, after_row, after_column, direction }`, with the limits in section 2.
  - **Checks:** guard and describe through `resolve(Target::Sql)`. Duplicate or unknown columns are 422 `Invalid search column`.
  - **Numbered relation:**
    - `runtime/cache/numbered.rs` keeps up to 4 numbered tables per engine, least recently used first out.
    - Each table, `quark_mem.quark_results.f_<hash of sql>`, is `SELECT row_number() OVER () - 1 AS "__quark_row_<hash>", * FROM query(?)`.
    - A table is created on its first search, and evicted tables are dropped through `engine.primary()`.
  - **Search:** `UNION ALL` over the requested columns of `contains(lower(CAST(c AS VARCHAR)), lower(?))`.
    - Forward searches use `>` and ascending order; backward searches use `<` and descending order.
    - Rows past the cursor sort first, so the search wraps. `LIMIT 1`.
  - **Response:** `{match: {row, column, column_index, value}}`, or `{match: null}`.
  - **Errors:** DuckDB errors are 422 `Cannot search this View: <err>`.
  - **Cancellation:** cancellable.
- **Tests first:**
  - `finds_next_and_previous_and_wraps`;
  - `second_search_reuses_the_numbered_table`;
  - `fifth_view_evicts_the_oldest_table`;
  - `invalid_column_is_422`.
- **pytest it makes pass:** `test_find_cell_searches_whole_view_literal_values_and_wraps`.
- **Hot spots:** row numbers follow the View's scan order (insertion order preserved); the row-key name cannot collide with a user column.
- **Done when:** `rtk cargo test -p quark-core --lib -- features::find runtime::cache::numbered && rtk cargo build -p quark-core --bin quark-dev-server && QUARK_TEST_BACKEND=rust rtk uv run pytest -q`.
- **Commit:** `feat(core): find cells in a View`

#### V7 · Attach and source path · batch 9
- **Files:** `core/src/features/sources/attach.rs` (new), `core/src/features/sources/routes.rs`, `core/src/features/sources/tests.rs`.
- **Needs:** R7.
- **Build:**
  - **Attach routes:** `POST /api/projects/{project_id}/sources/attach` and `POST /api/nodes/attach`, with body `{path}` (unknown fields 422). The checks run in Python's order:
    1. the project (404 `Project not found`);
    2. expand a leading `~` to the home folder and make the path absolute without requiring it to exist;
    3. the suffix must be `.duckdb` or `.db`, case-insensitive (400 `Only .duckdb and .db paths can be attached`);
    4. the path must be a file (404 `Database not found`).
  - **Registering:** the node is `{id: uuid4 hex, name: file name, kind: "attached", source: path, project_id?}`. It goes through `open_node_engine` (400 `Could not open source: …`) and `register_source`.
  - **Attach response:** 201, with `{id, name, kind, source}` on the legacy route and `{id, name, kind, project_id}` on the project route.
  - **Deleting** an attached source never deletes its file (`delete_source` already removes only uploads; keep it so).
  - **`GET /api/projects/{project_id}/sources/{source_id}/path`:** returns `{path: <expanded, absolute source path>}`, with 404 `Project not found` or `Source not found`.
- **Tests first:**
  - `attach_rejects_other_suffixes_before_missing_files`;
  - `attached_database_is_read_only`;
  - `deleting_an_attachment_keeps_the_file`;
  - `source_path_is_absolute`.
- **pytest it makes pass:**
  - `test_attach_is_read_only_and_missing_paths_fail`;
  - `test_attached_sql_blocks_external_files_but_queries_database_table`;
  - `test_dataset_ids_distinguish_schemas_and_slashes`;
  - `test_dataset_list_includes_columns_for_every_table`;
  - `test_project_workspace_exposes_every_attached_database_relation`;
  - `test_project_workspace_blocks_other_project_files_without_breaking_registered_views`;
  - `test_query_preserves_large_integers_and_filters_exactly`, which also needs V4.
- **Hot spots:** the order of the suffix and existence checks; on Windows, the path is not lower-cased.
- **Done when:** `rtk cargo test -p quark-core features::sources && rtk cargo build -p quark-core --bin quark-dev-server && QUARK_TEST_BACKEND=rust rtk uv run pytest -q`.
- **Commit:** `feat(core): attach databases by path and report source paths`

## 10. Unit cards: stream C (charts)

Every chart card follows the Python branch it names, line for line: the same checks in the same order, the same messages, the same limits, and values through `json_rows` (`safe()` rules). The upgrades named in a card change only how many queries run. Each kind lives in its own file under `features/charts/`. C2 adds the dispatcher; C3 to C8 each replace their kind's arm in `features/charts/service.rs`, which is an additive edit.

#### C1 · Chart requests · batch 8
- **Files:** `core/src/features/charts/mod.rs` (new), `core/src/features/charts/model.rs` (new), `core/src/features/charts/model/tests.rs` (new).
- **Needs:** R7.
- **Build:** serde types with `deny_unknown_fields` and lowercase enums.
  - **`ChartEncodings`:** `x`, `y`, `category`, `value`, `group`, `size`, `color` and `pattern`, all optional strings; defaults to empty.
  - **`ChartKind`:** `bar`, `histogram`, `box`, `scatter`, `line`, `pie`, `metric`.
  - **Other enums:**
    - `Metric`: `count`, `distinct`, `min`, `max`, `sum`, `avg`, `median`, `stddev`;
    - `Layout`: `grouped`, `stacked`, `stacked100`;
    - `Grain`: `hour`, `day`, `week`, `month`, `quarter`, `year`;
    - `Align`: `start`, `center`, `end`;
    - `Font`: `sans`, `mono`;
    - `Size`: `fit`, `sm`, `md`, `lg`.
  - **`ChartSpec`:** `chart`, then `encodings` (default empty), `metric`, `density` (default `false`), `layout`, `grain`, `expression`, `label`, `align`, `font` and `size`, all optional.
  - **Requests:**
    - `VisualizeRequest`: the `QueryRequest` fields plus `spec`;
    - `SqlVisualizeRequest`: `sql` plus the same fields. The fields are repeated rather than flattened, as in `SqlQueryRequest`;
    - both have `query()` conversions.
  - **`validate()`:**
    - paging gives 422 `Invalid page or page size`;
    - an `expression` over 2000 characters or a `label` over 200 (counted in `char`s) gives 422 with a short detail.
  - **`METRIC_SQL`** is a `fn metric_sql(metric: Metric, column: &str) -> String` over a quoted column, with the templates in section 2.
- **Tests first:**
  - `unknown_chart_kind_is_rejected`;
  - `unknown_encoding_field_is_rejected`;
  - `long_expression_is_422`;
  - `defaults_match_python`;
  - `metric_sql_templates`.
- **Done when:** `rtk cargo test -p quark-core features::charts::model`.
- **Commit:** `feat(core): add chart request types`

#### C2 · Chart dispatch, routes and the metric card · batch 11
- **Files:** `core/src/features/charts/service.rs` (new), `core/src/features/charts/metric.rs` (new), `core/src/features/charts/routes.rs` (new), `core/src/features/charts/tests.rs` (new), `core/src/http/mod.rs`.
- **Needs:** C1, V1, F4, F5.
- **Build:**
  - **Routes:** `POST /api/nodes/{node_id}/datasets/{dataset}/visualize` and `POST /api/nodes/{node_id}/sql/visualize`, with a cancellable `Reader`.
  - **`service::visualize(engine, conn, target, request) -> ApiResult<Value>`:**
    - `relation::filtered` runs once;
    - then a `match` on `spec.chart` calls the kind's function with `(conn, &relation, &spec)`;
    - every response carries `elapsed_ms`, rounded to 3 decimals;
    - the whole response goes through `engine.cached_response(conn, "chart", &relation, &serde_json::to_value(&spec)?, …)`.
  - **Kinds not built yet** answer 422 `Chart type <kind> is not available yet` (Python's fallback text) until their card replaces the arm.
  - **Errors:** DuckDB errors not mapped by a kind are 422 `Invalid filter value: <err>`.
  - **`metric::chart`:**
    - **Expression given:** trim it.
      - A `?` inside is 422 `A metric formula cannot contain ? placeholders`.
      - Guard `SELECT <expression> AS value FROM <relation.sql>` with `check_select`. If the SP1 guard cannot serialize SQL that holds `?` placeholders, guard `SELECT <expression> AS value FROM quark_relation` instead, and say so in the report.
    - **No expression, no value column:** `count(*)`.
    - **No expression, value column given:**
      - a missing column is 422 `Metric column was not found`;
      - `sum`, `avg`, `median` and `stddev` on a non-numeric column are 422 `This metric requires a numeric column`.
    - **Query:** `SELECT <value_sql>, count(*) FROM <relation>`.
      - A DuckDB error with an expression is 422 `This metric formula did not run: <err>`.
      - Without an expression, it is the default mapping.
    - **Returns** `{chart: "metric", value, rows, elapsed_ms}`.
- **Tests first:**
  - `metric_counts_rows_by_default`;
  - `metric_formula_binds_the_view_filters`;
  - `metric_formula_rejects_placeholders_and_multiple_statements`;
  - `cached_chart_reports_its_own_elapsed_time`.
- **pytest it makes pass:** `test_visualize_metric_card_reduces_a_column_to_one_value`, `test_visualize_metric_formula_runs_read_only_aggregates`.
- **Done when:** `rtk cargo test -p quark-core features::charts && rtk cargo build -p quark-core --bin quark-dev-server && QUARK_TEST_BACKEND=rust rtk uv run pytest -q`.
- **Commit:** `feat(core): serve charts, starting with the metric card`

#### C3 · Bar and pie · batch 12
- **Files:** `core/src/features/charts/bar.rs` (new), `core/src/features/charts/bar/tests.rs` (new), `core/src/features/charts/service.rs` (arms for `bar` and `pie`).
- **Needs:** C2.
- **Build:** Python's bar and pie branch, without the group encoding (C4 adds it).
  - **Label and limit:** `Pie` with `PIE_LIMIT`, otherwise `Bar` with `BAR_LIMIT`.
  - **Category:** missing is 422 `<Label> requires a category encoding`.
  - **Measure** (`encodings.value`). The metric is `spec.metric`, or `avg` when a measure is given and `count` otherwise.
    - A missing measure is 422 `<Label> measure column was not found`.
    - A non-count metric on a non-numeric measure is 422 `<Label> measure requires a numeric column`.
    - The extra filter adds `m IS NOT NULL`, plus `isfinite(m::DOUBLE)` unless the metric is `count` or `distinct`.
  - **One query** replaces Python's two. The rows and the non-null total come from `SELECT label, value, n, sum(n) OVER () AS non_null FROM (SELECT c AS label, <value_sql> AS value, count(*) AS n FROM <relation> WHERE c IS NOT NULL<extra> GROUP BY 1) ORDER BY value DESC NULLS LAST, label LIMIT <limit>`. When there are no rows, `non_null` is 0.
  - **Pie:** any row with a negative value is 422 `A pie chart cannot show negative values; use a bar chart instead`. A pie ignores `encodings.group`.
  - **Returns** `{chart, rows: [{label, value, n}], series: null, other_count: non_null − shown, elapsed_ms}`.
- **Tests first:**
  - `bar_counts_top_values_and_other_bucket`;
  - `bar_measure_filters_non_finite_values`;
  - `pie_uses_eight_slices`;
  - `pie_rejects_negative_values`;
  - `one_query_reports_the_non_null_total`.
- **pytest it makes pass:**
  - `test_visualize_bar_counts_top_values_and_other_bucket`;
  - `test_visualize_bar_aggregates_numeric_measure`;
  - `test_visualize_bar_layout_does_not_change_the_rows`;
  - `test_visualize_bar_without_group_keeps_the_old_shape`;
  - `test_visualize_pie_uses_the_tighter_slice_limit`;
  - `test_visualize_pie_rejects_negative_values`;
  - `test_visualize_pie_ignores_a_group_encoding`;
  - `test_visualize_sql_bar_and_rejects_unknown_columns_and_charts`.
- **Done when:** `rtk cargo test -p quark-core features::charts && rtk cargo build -p quark-core --bin quark-dev-server && QUARK_TEST_BACKEND=rust rtk uv run pytest -q`.
- **Commit:** `feat(core): draw bar and pie charts`

#### C4 · Grouped bar series · batch 13
- **Files:** `core/src/features/charts/groups.rs` (new), `core/src/features/charts/groups/tests.rs` (new), `core/src/features/charts/bar.rs`.
- **Needs:** C3, X1.
- **Build:** the group encoding for bar charts.
  - **Check:** a missing group column is 422 `Bar group column was not found`.
  - **Two queries** replace Python's three:
    1. Rank the series: `SELECT g AS series, count(*) AS n FROM <relation> WHERE c IS NOT NULL AND g IS NOT NULL<extra> GROUP BY 1 ORDER BY n DESC, series LIMIT 6`.
    2. Build the grid: labels limited to the bar's labels, grouped by label, `g IN (<top>) AS is_top`, and `CASE WHEN g IN (<top>) THEN g END`, with `<value_sql>` per group. Bind the relation parameters first, then the labels, then the series.
  - **"Other"** is the non-top group. `series` lists the top series as Python `str()` text (the renderer X1 exposes), then `"Other"` when that group exists.
  - **Values:** each row gets `values` aligned to `series`, with 0 for a missing cell.
  - **No labels or no series** leave `series: null` and add no `values`.
- **Tests first:**
  - `groups_align_values_to_series`;
  - `series_beyond_six_fold_into_other`;
  - `series_labels_use_python_str` (numbers, booleans `True`/`False`, timestamps with a space separator).
- **pytest it makes pass:**
  - `test_visualize_bar_groups_into_aligned_series`;
  - `test_visualize_bar_folds_series_beyond_the_cap_into_other`;
  - `test_visualize_bar_rejects_an_unknown_group_column`.
- **Done when:** `rtk cargo test -p quark-core features::charts && rtk cargo build -p quark-core --bin quark-dev-server && QUARK_TEST_BACKEND=rust rtk uv run pytest -q`.
- **Commit:** `feat(core): split bar charts into series`

#### C5 · Histogram · batch 12
- **Files:** `core/src/features/charts/histogram.rs` (new), `core/src/features/charts/histogram/tests.rs` (new), `core/src/features/charts/service.rs` (arm), `tests/test_backend.py` (one marker plus one new `rust_only` test).
- **Needs:** C2, V2, V3.
- **Build:** Python's histogram branch.
  - **Checks:**
    - missing value is 422 `Histogram requires a value encoding`;
    - a kind other than numeric or date is 422 `Histogram requires a numeric or date column`;
    - a missing group is 422 `Histogram group column was not found`.
  - **Bins** are the `histogram` of the column's profile from `relation::profile::numeric` or `date`, fetched through `engine.cached_response(conn, "stats", &relation, &json!(column), …)`. A histogram then reuses a profile the stats panel already computed, and the other way round.
  - **Uniform numeric bins** (at least 2, equal width within 1e-6 relative, as Python checks): grouped counts use Python's formula. Rank the top 6 series over finite rows, then compute `least(?, floor((c − ?) / ?)::INTEGER)` per series, and fold everything else into `"Other"`.
  - **Behaviour fix 2:** for every other bin layout (date bins, huge-integer `ntile` bins, a single bin), grouped counts bucket each row into the first bin whose `upper` is at least the value. Use a `CASE` chain over the bins' upper bounds, each bound as `CAST(? AS <column type>)`.
  - **Returns** `{chart: "histogram", bins, elapsed_ms}`, plus `series: [{label, bins: [{lower, upper, count}]}]` when a group is given and there is at least one series.
  - **Test markers:** `test_visualize_histogram_skips_series_for_a_date_column` becomes `python_only`. Add `test_visualize_histogram_groups_date_columns_into_series` (`rust_only`): a date column grouped by region returns series whose bins share the main bins' edges and whose counts add up to the totals.
- **Tests first:**
  - `uniform_bins_use_the_width_formula`;
  - `date_bins_group_by_edges`;
  - `single_bin_groups_by_series`;
  - `no_group_omits_series`.
- **pytest it makes pass:**
  - `test_visualize_histogram_matches_filtered_profile_bins`;
  - `test_visualize_histogram_groups_share_bin_edges`;
  - `test_visualize_histogram_without_group_omits_series`;
  - `test_visualize_histogram_rejects_an_unknown_group`;
  - the new `rust_only` test.
- **Hot spots:** the edge bucketing never changes ungrouped bins; the uniform path's results equal Python's.
- **Done when:** `rtk cargo test -p quark-core features::charts && rtk cargo build -p quark-core --bin quark-dev-server && QUARK_TEST_BACKEND=rust rtk uv run pytest -q`.
- **Commit:** `feat(core): draw histograms with series for every bin type`

#### C6 · Box · batch 12
- **Files:** `core/src/features/charts/box_plot.rs` (new), `core/src/features/charts/box_plot/tests.rs` (new), `core/src/features/charts/service.rs` (arm).
- **Needs:** C2.
- **Build:** Python's box branch.
  - **Checks:**
    - missing value is 422 `Box requires a value encoding`;
    - non-numeric is 422 `Box requires a numeric column`;
    - a missing group is 422 `Box group column was not found`.
  - **Groups:** the top 12 group labels by count of finite rows. When a group is given and there are none, return `{chart: "box", groups: [], elapsed_ms}`.
  - **One pass** replaces Python's two queries:
    - `quantile_cont` at 0.25, 0.5 and 0.75 per label;
    - Tukey whiskers: the minimum value at least `p25 − 1.5·IQR`, and the maximum at most `p75 + 1.5·IQR`;
    - up to 200 outliers per label, ascending, collected with `list(val ORDER BY val) FILTER (WHERE outside)` and sliced to 200.
  - **Order:** groups by `n DESC, label`.
  - **Returns** `{label, p25, median, p75, whisker_low, whisker_high, outliers, count}` per group. The label is `"all"` without a group.
- **Tests first:**
  - `single_box_has_tukey_whiskers_and_outliers`;
  - `groups_keep_the_twelve_largest`;
  - `outliers_are_capped_at_200_per_group`.
- **pytest it makes pass:** `test_visualize_box_tukey_whiskers_outliers_and_groups`.
- **Done when:** `rtk cargo test -p quark-core features::charts && rtk cargo build -p quark-core --bin quark-dev-server && QUARK_TEST_BACKEND=rust rtk uv run pytest -q`.
- **Commit:** `feat(core): draw box plots in one pass`

#### C7 · Scatter · batch 12
- **Files:** `core/src/features/charts/scatter.rs` (new), `core/src/features/charts/scatter/tests.rs` (new), `core/src/features/charts/service.rs` (arm), `tests/test_backend.py` (one new `rust_only` test).
- **Needs:** C2, X1 (Python `str()` renderer).
- **Build:** Python's scatter branch.
  - **Checks, in order:**
    1. x and y must be given and exist: 422 `Scatter requires x and y encodings`.
    2. Each of color, size and shape (`encodings.pattern`) must exist: 422 `Scatter <role> column was not found`.
    3. A non-numeric size is 422 `Scatter size requires a numeric column`.
  - **Rows:** both coordinates are non-null and, when numeric, finite.
  - **Totals and domains:** `total_points` is the count. Scale domains (`[min, max]` of finite values) come from the whole relation.
  - **Labels:** categorical color and shape labels are the top 6 by count, as Python `str()`. Values outside the labels fold to `"Other"`. `"Other"` is appended to a label list that reached 6.
  - **Behaviour fix 1:** the 5000-point sample is repeatable, so the same View and data give the same points.
    - Use `USING SAMPLE reservoir(5000 ROWS) REPEATABLE (42)` if the test below passes with the engine's default threads on a 200,000-row relation.
    - Otherwise use `ORDER BY hash(<x>, <y>, <extras>) LIMIT 5000`.
    - Name the choice in the report.
  - **Response keys:** `color_kind`, `color_domain` or `color_labels`, `size_domain` and `shape_labels` appear exactly when Python adds them.
  - **New test:** `test_visualize_scatter_sample_is_repeatable` (`rust_only`), where two calls on a 20,000-row file return identical `points`.
- **Tests first:**
  - `sample_is_repeatable` (5 runs, 200,000 rows);
  - `domains_cover_the_whole_relation`;
  - `labels_fold_into_other`;
  - `non_numeric_size_is_422`.
- **pytest it makes pass:**
  - `test_visualize_scatter_carries_color_categories_and_rejects_unknown_color`;
  - `test_visualize_scatter_labels_categorical_color_and_shape`;
  - `test_visualize_scatter_rejects_a_non_numeric_size`;
  - the new `rust_only` test.
- **Done when:** `rtk cargo test -p quark-core features::charts && rtk cargo build -p quark-core --bin quark-dev-server && QUARK_TEST_BACKEND=rust rtk uv run pytest -q`.
- **Commit:** `feat(core): draw scatter charts from a repeatable sample`

#### C8 · Line · batch 12
- **Files:** `core/src/features/charts/line.rs` (new), `core/src/features/charts/line/tests.rs` (new), `core/src/features/charts/service.rs` (arm).
- **Needs:** C2, X1, F1.
- **Build:** Python's line branch.
  - **x checks:** missing x is 422 `Line requires an x encoding`; an x that is not a date kind is 422 `Line requires a date or timestamp x column; <x> is not one`.
  - **Measure** (`encodings.y`). The metric defaults to `avg` with a measure and `count` without.
    - A missing measure is 422 `Line measure column was not found`.
    - A metric other than `count` or `distinct` on a non-numeric measure is 422 `Line measure requires a numeric column`.
    - The extra filter adds `m IS NOT NULL`, plus `isfinite` for metrics other than `count` and `distinct`.
  - **Grain:**
    - the automatic grain is the first of hour, day, week, month, quarter, year whose `span / GRAIN_SECONDS ≤ 400`, and `year` otherwise;
    - the span is `date_diff('second', min(x), max(x))`, which also works for TIME columns (Python raises a 500 there; record that as a deviation);
    - the bucket is `date_trunc('<grain>', x)`.
  - **One query** replaces Python's one per series:
    - Rank the top 6 groups in a CTE, bucket every row with `is_top` and the top group's value, and keep each series' latest 400 buckets with `QUALIFY row_number() OVER (PARTITION BY is_top, series ORDER BY bucket DESC) <= 400`.
    - Bind the relation parameters once per use of the relation.
    - The service orders the series by rank, then `"Other"`, and each series' points by ascending bucket.
    - An empty series is skipped. "Other" appears only when there are top series and other rows.
    - Without a group there is one series `{label: "", points}`.
    - A missing group column is 422 `Line group column was not found`.
  - **Returns** `{chart: "line", grain, series: [{label, points: [{x, y}]}], elapsed_ms}`.
- **Tests first:**
  - `automatic_grain_picks_the_first_fit`;
  - `explicit_grain_is_honoured`;
  - `series_are_ranked_and_other_comes_last`;
  - `counts_rows_without_a_y_encoding`;
  - `line_buckets_follow_the_session_zone_across_dst` (Review Focus 4): with `SET TimeZone = 'America/New_York'`, TIMESTAMPTZ rows around 2024-03-10 bucket into local days.
- **pytest it makes pass:**
  - `test_visualize_line_buckets_by_an_automatic_grain`;
  - `test_visualize_line_honours_an_explicit_grain`;
  - `test_visualize_line_splits_into_series_by_group`;
  - `test_visualize_line_counts_rows_without_a_y_encoding`;
  - `test_visualize_line_rejects_a_non_date_x`.
- **Done when:** `rtk cargo test -p quark-core features::charts && rtk cargo build -p quark-core --bin quark-dev-server && QUARK_TEST_BACKEND=rust rtk uv run pytest -q`.
- **Commit:** `feat(core): draw line charts in one query`

## 11. Unit cards: stream J (joins)

#### J1 · Join preview in one engine · batch 11
- **Files:** `core/src/features/joins/{mod,model,service,routes}.rs` (new), `core/src/features/joins/tests.rs` (new), `core/src/http/mod.rs`.
- **Needs:** V1, F4.
- **Build:** Python's `create_join_workspace` for every case except two dataset references on two different nodes, which J2 adds.
  - **Request:** `JoinReference { node_id, dataset?, sql?, name? }` and `JoinWorkspaceRequest { left, right, left_keys, right_keys }`, with unknown fields 422.
  - **Checks, in Python's order:**
    1. Key counts differ: 422 `Join key counts must match`.
    2. Duplicate keys on either side: 422 `Join keys must be unique`.
    3. A reference without exactly one of `dataset` and `sql`: 422 `Join reference requires exactly one of dataset or sql`.
  - **SQL path** (either side uses `sql`):
    - Both node ids must resolve to the same engine (`Arc::ptr_eq`), or 422 `SQL joins require one project workspace`.
    - An SQL side is guarded and described.
      - Its table is `query(<sql as an escaped literal>)`, inline with no parameter, as Python writes it.
      - Its identity is `{name: name or "View", sql}`.
    - A dataset side's table is `"schema"."name"` and its identity is `{schema, name}`.
  - **Dataset path, same node:**
    - Both node ids must be active default-project sources (`legacy_nodes`), or 404 `Node not found`. Project node ids are refused here, as in Python.
    - Both sides run in that node's engine.
  - **Keys** missing on their side are 422 `Join key not found`.
  - **Counts:**
    - **Row counts:** `left_rows` and `right_rows` come from the count cache when a page of that dataset was served before (key `stats_key("(SELECT * FROM <table>)", [])`), and from `count(*)` otherwise.
    - **No keys:** relationship `cartesian`, and `output_rows` is the product.
    - **With keys:** each side is unique when its non-null key count equals `count(DISTINCT (keys))`, one scan per side. The relationship is `one_to_one`, `one_to_many`, `many_to_one` or `many_to_many`. `output_rows` is the inner-join count.
  - **Returns,** through `safe()`: `{node_id, left, right, left_rows, right_rows, output_rows, relationship, cartesian_risk: relationship is cartesian or many_to_many}`. `node_id` is the engine's node id.
  - **Run** to completion on a reader (`run_to_end`).
- **Tests first:**
  - `key_validation_order_matches_python`;
  - `sql_join_needs_one_workspace`;
  - `relationship_classification`;
  - `cartesian_preview_multiplies_counts`;
  - `counts_come_from_the_count_cache_when_present`.
- **pytest it makes pass:**
  - `test_join_workspace_same_node_composite_cardinality`;
  - `test_join_workspace_empty_keys_previews_cartesian_product`;
  - `test_join_workspace_marks_many_to_many_risk`;
  - `test_join_workspace_rejects_invalid_references_and_keys`;
  - `test_join_preview_accepts_arbitrary_sql_views_only_in_one_project_workspace`;
  - `test_project_owned_sources_cannot_join_through_dataset_references`.
- **Done when:** `rtk cargo test -p quark-core features::joins && rtk cargo build -p quark-core --bin quark-dev-server && QUARK_TEST_BACKEND=rust rtk uv run pytest -q`.
- **Commit:** `feat(core): preview joins inside one engine`

#### J2 · Cross-source join workspaces · batch 13
- **Files:** `core/src/runtime/joins.rs` (new), `core/src/runtime/joins/tests.rs` (new), `core/src/runtime/engines.rs` (resolve `join_*` ids), `core/src/features/joins/service.rs` (cross branch), `tests/test_backend.py` (one marker plus one new `rust_only` test).
- **Needs:** J1, F3.
- **Build:**
  - **`JoinSlots`** (runtime) keeps up to 4 cross-source join engines, keyed by `join_<uuid4 hex>`, least recently used first out.
  - **`AppState::open_join(left: (&SourceRecord, &DatasetInfo), right: (&SourceRecord, &DatasetInfo)) -> ApiResult<(String, Arc<Engine>)>`** builds a fresh in-memory engine with a read pool. Each side mounts under schema `left_source` or `right_source` as Python's `mount_dataset` does:
    - a DuckDB file attaches read-only as `<schema>_database`, with a view over it;
    - a flat file with a ready columnar cache gets a view over the attached cache file;
    - any other flat file gets a view over its scan expression.
  - **Lock-down:** the engine allows exactly the files it reads (`allowed_paths`). Python allowed whole parent folders, which is wider; record this as a deviation.
  - **Lookup:** `engine_for_node` resolves join ids from the slots (404 `Node not found` once evicted). Join ids are never listed.
  - **Identities** are `{schema: "left_source" | "right_source", name}`.
  - **Test markers:** `test_cross_node_join_workspace_replaces_previous_without_listing_it` becomes `python_only`. Add `test_cross_node_join_workspaces_stay_open_until_evicted` (`rust_only`):
    - two cross-source joins are both queryable through `/sql`;
    - creating three more evicts the first (404);
    - `/api/nodes` lists none of them.
- **Tests first:**
  - `join_engine_reads_columnar_copies_when_ready`;
  - `fifth_join_evicts_the_least_recent`;
  - `join_engine_cannot_read_other_files`.
- **pytest it makes pass:** `test_cross_node_join_workspace_is_queryable_by_sql_and_profile_routes` and the new `rust_only` test.
- **Hot spots:** an evicted join engine still finishes its running queries (`Arc`); the allow-list includes `.wal` siblings and cache files.
- **Done when:** `rtk cargo test -p quark-core --lib -- features::joins runtime::joins && rtk cargo build -p quark-core --bin quark-dev-server && QUARK_TEST_BACKEND=rust rtk uv run pytest -q`.
- **Commit:** `feat(core): keep several cross-source join workspaces open`

## 12. Unit cards: stream X (exports)

#### X1 · Export cells · batch 8
- **Files:** `core/src/shared/encode/export.rs` (new), `core/src/shared/encode/export/tests.rs` (new), `core/src/shared/encode/cell.rs` (visibility only), `core/src/shared/encode/mod.rs` (mod line).
- **Needs:** R7.
- **Build:**
  - **Python `str()` renderer:** SP1's renderer for Python `str()` of a cell (`python_key_str`, used for map keys) becomes `pub(crate) fn python_str(array, row, zone) -> String`. Charts (C4, C7, C8) use it too.
  - **`pub enum ExportCell`:** `Null`, `Bool(bool)`, `Int(i64)`, `Float(f64)`, `Decimal(String)`, `Text(String)`, `Date(NaiveDate)`, `DateTime(NaiveDateTime)`, `Time(NaiveTime)`, `Duration(chrono::TimeDelta)`.
  - **`export_cell(array, row, type_name, zone) -> ExportCell`** follows Python's `export_value`:
    - BLOB is lowercase hex text.
    - LIST, ARRAY, STRUCT and MAP become `json.dumps(export_nested(v), ensure_ascii=False)`. That means Python's `", "` and `": "` separators, Python float `repr` (`python_float_repr`), and `str(key)` for map keys. Nested values follow `export_nested`:
      - bytes become hex;
      - decimals become `str`;
      - an integer beyond ±(10^15 − 1) becomes `str`;
      - NaN and ±infinity become `"NaN"` and `"Infinity"` or `"-Infinity"`;
      - temporal values become `isoformat()`;
      - an interval becomes Python `str(timedelta)`.
    - An integer beyond ±(10^15 − 1) becomes text.
    - A DECIMAL that is not finite or has more than 15 digits becomes text; otherwise it stays `Decimal`, holding Python's `str`.
    - Float NaN becomes `"NaN"`, and ±infinity becomes `"Infinity"` or `"-Infinity"`.
    - TIMESTAMPTZ and TIMETZ become text in `isoformat()` in the machine's zone.
    - DATE, TIMESTAMP and TIME keep their types.
    - INTERVAL becomes a duration, with months counted as 30 days, as DuckDB's Python client does.
    - UUID, ENUM, BIT and JSON become text.
    - Every text value is stripped of `ILLEGAL_XML` characters.
  - **`csv_text(cell) -> Option<String>`** follows Python's `csv` module on `csv_value`. `None` means an empty field. Formats:
    - `True` and `False`;
    - integers as digits, floats as `repr`, decimals as given;
    - dates as `YYYY-MM-DD`;
    - naive datetimes as `YYYY-MM-DD HH:MM:SS[.ffffff]`, with a space;
    - times as `HH:MM:SS[.ffffff]`;
    - durations as Python `str(timedelta)`, such as `1 day, 2:00:00` or `-1 day, 23:59:59`.

    Text starting with one of `CSV_FORMULA_PREFIXES` gets a leading `'`.
  - **`csv_line(fields: &[Option<String>]) -> String`:**
    - the excel dialect: comma, `"` quoting only when a field holds `,`, `"`, `\r` or `\n`, with inner quotes doubled;
    - a row of one empty field is written as `""`;
    - each line ends with `\r\n`.
- **Tests first:** one test per rule above, named for the rule, for example `big_integers_become_text`, `nested_values_use_python_json_separators`, `formula_prefixes_are_neutralized` and `naive_datetimes_use_a_space`.
- **Done when:** `rtk cargo test -p quark-core shared::encode::export`.
- **Commit:** `feat(core): convert cells for export the way Python does`

#### X2 · Export requests and names · batch 9
- **Files:** `core/src/features/exports/{mod,model,names}.rs` (new), `core/src/features/exports/names/tests.rs` (new).
- **Needs:** R7.
- **Build:**
  - **Request types:**
    - `ExportFormat`: `csv`, `xlsx`, `parquet`, `json`;
    - `JsonLayout`: `rows` (default) or `columns`;
    - `ExportSheet { node_id, name, sql }`;
    - `ExportRequest { format, json_layout, filename?, sheets }`, with 1 to 100 sheets, else 422. Unknown fields are 422.
  - **`media_type(format)` and `extension(format)`** follow `EXPORT_TYPES`.
  - **`export_sheet_name(name, used: &mut HashSet<String>) -> String`** ports Python's function:
    1. strip `ILLEGAL_XML`, replace any of `\ / * ? : [ ]` with `_`, trim whitespace, then trim `'`; empty becomes `Sheet`;
    2. keep 31 characters, trim a trailing `'`, `Sheet` when empty;
    3. `History` (any case) gets `_` appended;
    4. while the lower-cased candidate is taken, use `<base cut to 31 − len(suffix), right-trimmed of '>` plus ` (n)`, with n from 2;
    5. remember the lower-cased name.

    Python uses `casefold`; `to_lowercase` differs only for a few letters such as `ß`. Mark that with a `ponytail:` comment.
  - **`export_filename(name: Option<&str>, extension) -> String`:**
    1. take the base name after turning `\` into `/`, then its stem as Python's `Path.stem` gives it;
    2. drop control characters `\x00`–`\x1f` and `\x7f`, trim spaces and dots; empty becomes `export`;
    3. add the extension.
  - **`content_disposition(filename) -> String`** follows Starlette's `FileResponse`:
    - `attachment; filename="<name>"` when Python's `urllib.parse.quote(name)` (safe `/`, unreserved `A–Z a–z 0–9 _ . - ~`) leaves it unchanged;
    - `attachment; filename*=utf-8''<quoted>` otherwise.
- **Tests first:**
  - `sheet_names_are_sanitized_truncated_and_unique`;
  - `history_is_renamed`;
  - `filename_keeps_only_a_safe_stem`;
  - `content_disposition_quotes_non_ascii_names`;
  - `sheet_count_bounds`.
- **Done when:** `rtk cargo test -p quark-core features::exports`.
- **Commit:** `feat(core): add export request types and file naming`

#### X3 · CSV export and the export route · batch 12
- **Files:** `core/src/features/exports/{service,csv,routes}.rs` (new), `core/src/features/exports/tests.rs` (new), `core/src/runtime/state.rs` (sweep the exports folder at startup), `core/src/http/mod.rs`, `Cargo.toml` (`tokio-util` with `io`). Six files in total: the route, the writer and the response streaming have to land together.
- **Needs:** X1, X2, F4, F9.
- **Build:**
  - **Route:** `POST /api/exports`.
  - **Checks, in Python's order:**
    1. A format other than `xlsx` needs exactly one sheet: 422 `<FORMAT> export requires exactly one sheet`.
    2. Each sheet's node resolves through `engine_for_node` (404 `Node not found`).
    3. Each sheet's SQL passes `check_select`.
  - **Writing:**
    - The file is `<cache>/exports/.export-<uuid4 hex><ext>`, written on a reader with `run_to_end`.
    - The sheet's SQL is read through `relation::rows::select_list` (F9), so TIMETZ columns keep their offsets.
    - The CSV writer puts the header row first (column names through `csv_text` of a `Text` cell), then rows from Arrow batches through `export_cell`, `csv_text` and `csv_line`.
    - A DuckDB error is 422 `Invalid SQL query: <err>`. Any failure deletes the file.
  - **Response:**
    - Status 200, `Content-Type` from `media_type`, and `Content-Disposition` from `content_disposition(export_filename(...))`.
    - The body streams the file through `tokio_util::io::ReaderStream`.
    - The file is deleted when the body ends or is dropped. The path guard drops after the file handle, so Windows can delete it.
  - **Startup:** removes leftover files in `<cache>/exports/`.
- **Tests first:**
  - `csv_export_writes_headers_and_rows`;
  - `abandoned_export_leaves_no_file` (Review Focus 5): drop the response body after the first chunk, then the folder is empty;
  - `empty_results_export_valid_files` (Review Focus 5, CSV): a header line only;
  - `export_needs_exactly_one_sheet_for_csv`;
  - `startup_sweeps_old_exports`.
- **pytest it makes pass:**
  - `test_export_csv_returns_every_row_with_headers_and_safe_filename`;
  - `test_export_csv_neutralizes_spreadsheet_formulas`;
  - `test_export_csv_requires_exactly_one_sheet_and_payload_bounds`;
  - `test_export_rejects_non_read_only_unknown_and_external_sql`;
  - `test_project_owned_sources_cannot_use_legacy_node_routes`.
- **Hot spots:** the file is deleted on every path, including a panic in the writer; the export never holds a cancellable guard.
- **Done when:** `rtk cargo test -p quark-core features::exports && rtk cargo build -p quark-core --bin quark-dev-server && QUARK_TEST_BACKEND=rust rtk uv run pytest -q`.
- **Commit:** `feat(core): export Views as CSV downloads`

#### X4 · XLSX export · batch 13
- **Files:** `core/src/features/exports/xlsx.rs` (new), `core/src/features/exports/xlsx/tests.rs` (new), `core/src/features/exports/service.rs` (the `xlsx` arm), `Cargo.toml` (`rust_xlsxwriter`).
- **Needs:** X3.
- **Build:**
  - **Workbook:** `rust_xlsxwriter` in constant-memory mode, with one worksheet per sheet, in request order, named by `export_sheet_name`. Each sheet is read from its own node's engine.
  - **Rows:** a header row of text cells, then the rows.
  - **Cells:**
    - `Text` is written as a string, never as a formula. A text longer than 32,767 characters is 422 `Excel cell text exceeds 32767 characters`, and the partial file is deleted.
    - `Int`, `Float` and `Decimal` are numbers; `Bool` is a boolean.
    - `Date`, `DateTime` and `Time` use the formats `yyyy-mm-dd`, `yyyy-mm-dd h:mm:ss` and `h:mm:ss`, so `openpyxl` reads them back as `date`, `datetime` and `time`.
    - `Duration` is a number of days with the format `[h]:mm:ss`.
    - `Null` leaves the cell empty.
- **Tests first:**
  - `sheets_follow_request_order_and_names`;
  - `text_is_never_a_formula`;
  - `oversized_text_is_422_and_leaves_no_file`;
  - `empty_results_export_valid_files` (XLSX case: a header row only).
- **pytest it makes pass:**
  - `test_export_xlsx_writes_multiple_nodes_and_stream_safe_values`;
  - `test_export_xlsx_sanitizes_and_uniquifies_sheet_names_case_insensitively`;
  - `test_export_xlsx_preserves_excel_unsafe_scalars_headers_and_zoned_dates`;
  - `test_xlsx_rejects_oversized_cells_without_truncating_csv`.
- **Done when:** `rtk cargo test -p quark-core features::exports && rtk cargo build -p quark-core --bin quark-dev-server && QUARK_TEST_BACKEND=rust rtk uv run pytest -q`.
- **Commit:** `feat(core): export XLSX workbooks`

#### X5 · Parquet and JSON export · batch 13
- **Files:** `core/src/features/exports/copy.rs` (new), `core/src/features/exports/copy/tests.rs` (new), `core/src/features/exports/service.rs` (the `parquet` and `json` arms), `Cargo.toml` (the `appender-arrow` feature on `duckdb`).
- **Needs:** X3.
- **Build:** Python's `export_relation`, with Arrow batches in place of `executemany`.
  1. Duplicate column names are 422 `<FORMAT> export requires unique column names`.
  2. Open a writer: a fresh in-memory DuckDB that may write only into `<cache>/exports/`. It never runs user SQL.
  3. Create `export_data(<quoted name> <DuckDB type>, …)` from `DESCRIBE` of the sheet's SQL.
  4. Append the reader's Arrow batches with `Appender::append_record_batch`.
  5. Copy out:
     - Parquet: `COPY export_data TO '<file>' (FORMAT PARQUET)`;
     - JSON rows: `(FORMAT JSON, ARRAY true)`;
     - JSON columns: `COPY (SELECT COALESCE(list("c"), CAST([] AS <type>[])) AS "c", … FROM export_data) TO '<file>' (FORMAT JSON)`.

  A type the appender cannot take is 422 with DuckDB's message; record each one met as a deviation.
- **Tests first:**
  - `parquet_keeps_column_types`;
  - `json_rows_are_an_array_of_objects`;
  - `json_columns_hold_lists_even_when_empty`;
  - `duplicate_names_are_422`;
  - `empty_results_export_valid_files` (Parquet and JSON cases).
- **pytest it makes pass:** `test_export_parquet_writes_typed_columns`, `test_export_json_row_objects_and_column_lists`.
- **Hot spots:** the writer's allow-list holds only the exports folder; `TIMESTAMPTZ` keeps the machine's zone (ICU), as in Python.
- **Done when:** `rtk cargo test -p quark-core features::exports && rtk cargo build -p quark-core --bin quark-dev-server && QUARK_TEST_BACKEND=rust rtk uv run pytest -q`.
- **Commit:** `feat(core): export Parquet and JSON through a sandboxed writer`

## 13. Unit cards: stream W (workbooks)

#### W1 · Workbook cells into a staging table · batch 10
- **Files:** `core/src/runtime/cache/workbook.rs` (new), `core/src/runtime/cache/workbook/tests.rs` (new), `core/Cargo.toml` (`calamine`; `rust_xlsxwriter` as a dev-dependency).
- **Needs:** F8, R7.
- **Build:** follow `docs/plans/2026-10-13-xlsx-read-rules.md`, read in full; it is under 150 lines.
  - **`pub fn sheet_names(path: &Path) -> anyhow::Result<Vec<String>>`:** workbook order. No sheets is an error, `Workbook has no sheets`.
  - **`pub enum Declared { Date, Timestamp, Time, Other }`.**
  - **`pub fn stage_sheet(conn: &Connection, path: &Path, sheet: &str, table: &str) -> anyhow::Result<Vec<(String, Declared)>>`:**
    - streams the sheet's cells with calamine;
    - applies the doc's header detection, column naming and empty-row rules;
    - creates `<table>` with one `VARCHAR` column per sheet column;
    - appends each cell's all-text form (the doc's rendering rules) with the DuckDB `Appender`;
    - returns each column's name and the type `read_xlsx` would declare.
- **Tests first:** use workbooks written with `rust_xlsxwriter` that mirror the F8 corpus cases. For each case, assert:
  - the staged column names;
  - the staged text per cell;
  - the declared types.

  Name each test for its rule, for example `numeric_first_row_means_no_header` and `dates_stage_as_excel_serials`.
- **Hot spots:** memory stays flat on a 100,000-row sheet (streamed, not loaded whole); sheet names with `'` and non-ASCII letters.
- **Done when:** `rtk cargo test -p quark-core runtime::cache::workbook`.
- **Commit:** `feat(core): stage workbook sheets as text with calamine`

#### W2 · Typed workbook cache and mounting · batch 11
- **Files:** `core/src/runtime/cache/workbook_import.rs` (new), `core/src/runtime/cache/workbook_import/tests.rs` (new), `core/src/runtime/engine.rs` (the `.xlsx` branch of `open_node`), `core/src/runtime/mount.rs` (`.xlsx` sources).
- **Needs:** W1.
- **Build:**
  - **`pub fn workbook_key(path, sheets: &[String]) -> io::Result<ColumnarKey>`:** `ColumnarKey::new(path, "xlsx:" + compact JSON of sheets, engine_version)`. `engine_version()` in `runtime/mount.rs` becomes `pub(crate)`.
  - **`pub fn import_workbook(path, sheets, cache_dir) -> anyhow::Result<PathBuf>`:**
    1. Into the key's `.partial` DuckDB file, for each sheet: `stage_sheet` into a temporary table.
    2. Compute Python's `_xlsx_table` ratios for every column in one scan: `numeric_ratio`, `date_ratio`, `timestamp_ratio`, `has_time` (`regexp_matches(TRIM(c), '[T ]\d{1,2}:')`) and `has_fraction`, with the same SQL expressions.
    3. Pick each column's expression with Python's rules, in Python's order:
       - declared DATE with a fraction becomes a TIMESTAMP from the serial;
       - declared DATE becomes a DATE;
       - declared TIMESTAMP becomes a TIMESTAMP;
       - declared TIME becomes a TIME;
       - a numeric ratio of at least 0.9 becomes DOUBLE;
       - a timestamp ratio of at least 0.9 with a time part becomes TIMESTAMP;
       - a date ratio of at least 0.9 becomes DATE;
       - anything else stays text.
    4. `CREATE TABLE main."<sheet>" AS SELECT …`, then drop the staging table.
    5. Checkpoint, close, and rename into place, like the columnar cache.
  - **Node engine:** a `.xlsx` source attaches its cache read-only, importing first when the cache isn't ready. Each sheet is copied in with `CREATE TABLE main."<sheet>" AS SELECT * FROM <cache>.main."<sheet>"`, so `/datasets` reports type `TABLE`, as Python's per-connection import did.
  - **Workspace mount:** attaches the cache read-only, with the cache file and its `.wal` in the allow-list. Each sheet gets schema `source_<…>` and a view over the cached table. Its `ViewInfo` has `type: "TABLE"`, as Python's.
  - **Eviction:** an evicted cache is imported again from the `.xlsx` on next use.
- **Tests first:**
  - `typed_tables_follow_python_rules`;
  - `serial_dates_reinterpret_only_when_declared`;
  - `node_engine_lists_sheets_as_tables`;
  - `workspace_views_read_the_cache`;
  - `evicted_cache_is_imported_again`;
  - `import_leaves_no_partial_file_on_failure`.
- **Done when:** `rtk cargo test -p quark-core --lib -- runtime::cache::workbook_import runtime::mount`.
- **Commit:** `feat(core): import workbooks once into a typed cache`

#### W3 · Workbook staging routes · batch 13
- **Files:** `core/src/features/workbooks/{mod,service,routes}.rs` (new), `core/src/features/workbooks/tests.rs` (new), `core/src/runtime/catalog.rs` (pending workbooks), `core/src/features/sources/uploads.rs` (`.xlsx` branch), `core/src/http/mod.rs`.
- **Needs:** W2, R7.
- **Build:**
  - **Runtime:** `pending_workbooks: HashMap<String, PendingWorkbook { id, name, source, sheets, project_id: Option<String> }>`, held in memory only. Runtime primitives:
    - `stage_workbook(...)`: lists sheet names; on failure deletes the file and returns 400 `Could not open source: <err>` (the error-path difference, spec §7);
    - `take_pending_workbook(stage_id, project_id)`;
    - `discard_pending_workbook(...)`.
  - **Upload:** `.xlsx` is now accepted. After the file is written it is staged, and the upload answers 201 `{id, name, kind: "workbook", sheets}`, plus `project_id` on the project route.
  - **Confirm:** `POST /api/projects/{project_id}/sources/upload/{stage_id}/confirm` and `POST /api/nodes/upload/{stage_id}/confirm`, with body `{sheets}`.
    1. The project must exist.
    2. A missing stage, or one from another project (`default` when unset), is 404 `Workbook upload not found`.
    3. Empty, duplicate or unknown sheets are 422 `Select one or more unique workbook sheets`.
    4. Register the source: kind `upload`, `sheets` set, no `dataset_name`. The registry record also holds `sheets`.
    5. Open the node engine, which imports the cache. A failure is 400 `Could not open source: …`, and the stage is kept, as in Python.
    6. Remove the stage and answer 200 with the public node (legacy or project shape).
  - **Discard:** `DELETE /api/projects/{project_id}/sources/upload/{stage_id}` and `DELETE /api/nodes/upload/{stage_id}`. It removes the stage and deletes its file, answering 204, or 404 as for confirm.
  - **Restart:** a restart forgets pending stages, and SP1's orphan sweep deletes their files.
  - **New test:** `test_unreadable_workbook_upload_is_400` (`rust_only`): a `.xlsx` that is not a zip answers 400, and nothing stays in `uploads/`.
- **Tests first:**
  - `staging_lists_sheets_in_workbook_order`;
  - `confirm_validates_the_selection`;
  - `confirm_from_another_project_is_404`;
  - `discard_deletes_the_file`;
  - `unreadable_workbook_is_400`.
- **Done when:** `rtk cargo test -p quark-core features::workbooks && rtk cargo build -p quark-core --bin quark-dev-server && QUARK_TEST_BACKEND=rust rtk uv run pytest -q`.
- **Commit:** `feat(core): stage, confirm and discard workbook uploads`

#### W4 · Workbook parity · batch 14
- **Who does what:** the task manager runs the loop, and fix workers fix.
- **Needs:** W3.
- **Loop:**
  1. Run `rtk cargo build -p quark-core --bin quark-dev-server && QUARK_TEST_BACKEND=rust rtk uv run pytest tests/test_workbook_corpus.py tests/test_backend.py -k "workbook or xlsx" -q --tb=line` with the workbook lines temporarily removed from `tests/rust_pending.txt`.
  2. For each failure cluster, open a fix unit with:
     - the failing test;
     - the expected-versus-actual difference;
     - the owning file: `runtime/cache/workbook.rs` for staging, `workbook_import.rs` for typing, `features/workbooks/` for routes;
     - a budget of 100 lines;
     - a Rust regression test.
  3. Repeat until nothing fails, then delete the workbook lines from `tests/rust_pending.txt` for good.
- **pytest it makes pass:**
  - `test_upload_workbook_requires_sheet_confirmation`;
  - `test_workbook_dates_are_typed_summarized_by_year_and_identifiers_stay_text`;
  - `test_workbook_scroll_pages_do_not_reopen_upload`;
  - `test_workbook_confirmation_validates_selection_and_cancel`;
  - `test_restart_discards_unconfirmed_workbook_upload`;
  - every case in `tests/test_workbook_corpus.py`;
  - `test_unreadable_workbook_upload_is_400`.
- **Rules:**
  - Never change a Python assertion.
  - A difference that cannot be closed goes to the Lead as a deviation.
- **Done when:** the Rust-mode run of the workbook and corpus tests passes on macOS.
- **Commits:** `fix(core): <behavior> to match Python's workbook reading`, one per fix unit.

## 14. Unit cards: stream E (exit)

#### E1 · Performance gate and awkward names · batch 14
- **Files:** `core/tests/perf_acceptance.rs`, `core/tests/odd_names.rs` (new), `docs/plans/2026-10-13-rust-desktop-parity-perf.md`.
- **Needs:** V4, V5, V6, C3, C5, C6, C7, C8, J2, X4, X5, W3, F7.
- **Build:**
  - **Performance cases.** `perf_acceptance.rs` gains one `#[ignore]` case per spec §9 row, each asserting its confirmed cap on the 2M-row CSV:
    - stats;
    - category values;
    - bar, line, histogram and metric;
    - scatter and box;
    - Find Next;
    - join preview;
    - CSV export;
    - XLSX export of 1,000,000 rows;
    - workbook confirm of 100,000 by 10, using a workbook written with `rust_xlsxwriter` from the dev-dependencies;
    - a page during an export.
  - **Comparison:** run `tests/perf/compare.py --python --rust` again and update the results doc. Every Rust median must be under Python's.
  - **`odd_names.rs`** (Review Focus 3) drives the router over a CSV whose headers are `a"b`, `with space`, `Zoë` and `select`. It runs stats, category values, find, a bar chart grouped by another awkward column, and a CSV export, and checks every answer is 200 with the right names.
- **Tests first:**
  - `odd_column_names_work_in_every_analysis_route` (Review Focus 3);
  - one `#[ignore]` perf case per cap above, each failing before its route exists.
- **Done when:** `rtk cargo test -p quark-core --test odd_names`, then, on a quiet machine, `rtk cargo test -p quark-core --release --test perf_acceptance -- --ignored --nocapture` passes and the results doc is updated.
- **Commit:** `test(core): hold the new routes to their performance caps`

#### E2 · WebKit pass: data panels · batch 13
- **Files:** `frontend/tests/live/harness.mjs` (new), `frontend/tests/live/panels-browser.mjs` (new), `frontend/package.json` (script `test:live`).
- **Needs:** V4, V5, V6.
- **Build:**
  - **`harness.mjs`:**
    - starts `target/debug/quark-dev-server --data-dir <tmp> --cache-dir <tmp>/cache --port 0` and reads `QUARK_LISTENING`;
    - starts Vite programmatically with `server.proxy['/api']` pointing at that port;
    - launches Playwright `webkit` from `PLAYWRIGHT_MODULE`, or `playwright`;
    - creates a project and uploads a fixture CSV through the API;
    - tears everything down on exit or failure.
  - **`panels-browser.mjs`:** open the View, then:
    - open the stats panel for a numeric and a text column and check the shown counts against the API;
    - open the category filter list, search it, apply one value, and check the grid's total;
    - use find next, previous and wrap-around.
  - **Fixes:** any frontend fix the pass needs is a separate fix unit, reported to the Lead.
- **Tests first:**
  - `stats_panel_matches_the_api`;
  - `filter_values_search_and_apply`;
  - `find_next_previous_and_wrap`.

  Each is a named step in `panels-browser.mjs` that asserts and fails loudly.
- **Done when:** `npm --prefix frontend run test:live -- panels` passes.
- **Commit:** `test(ui): run the data panels against the Rust core in WebKit`

#### E3 · WebKit pass: charts, joins, exports, workbooks · batch 14
- **Files:** `frontend/tests/live/charts-browser.mjs` (new), `frontend/tests/live/flows-browser.mjs` (new).
- **Needs:** E2, C3, C4, C5, C6, C7, C8, J2, X4, X5, W3.
- **Build:**
  - **`charts-browser.mjs`:** every chart kind on the Chart canvas, then a dashboard of 4 tiles. It changes a filter while tiles load and checks that every tile renders, with no page errors and no failed requests other than aborted ones.
  - **`flows-browser.mjs`:**
    - joins two sources and opens the join View;
    - exports CSV and XLSX, catching Playwright's `download` event and checking the file's first row;
    - uploads an `.xlsx`, confirms two sheets and reads rows from each.
  - **Report:** whether the blob download works in WebKit. If it doesn't, the Lead opens a fix unit: an `on_download` handler in `src-tauri/src/lib.rs` that saves into the Downloads folder (spec §2). That unit is checked in H2 on each OS.
- **Tests first:**
  - `every_chart_kind_renders`;
  - `dashboard_survives_a_filter_change`;
  - `join_view_opens`;
  - `csv_and_xlsx_download_with_the_first_row`;
  - `workbook_upload_confirms_two_sheets`.

  Each is a named step that asserts.
- **Done when:** `npm --prefix frontend run test:live -- charts flows` passes.
- **Commit:** `test(ui): run charts, joins, exports and workbooks against the Rust core in WebKit`

#### E4 · Fallbacks and SP1 leftovers · batch 14
- **Files:** `core/src/http/fallback.rs` (new: `unknown_route`, moved out of `http/mod.rs`), `core/src/runtime/state.rs`, `core/src/runtime/cache/columnar_files.rs`, `.github/workflows/ci.yml`, and `src-tauri/src/lib.rs` only for deviation 24.
- **Needs:** V4, V5, V6, V7, C8, J2, X5, W3.
- **Build:**
  - **Unknown routes** now answer as FastAPI does: 404 `{"detail": "Not Found"}` for an unknown path and 405 `{"detail": "Method Not Allowed"}` for a wrong method on a known path. The 501 text is gone.
  - **Startup** removes `.wal` files in the columnar folder whose cache file is missing.
  - **Deviation 24:** find which dependency prints `Error reading the log directory/files` during the self-test. If creating that folder before start-up silences it, do that; otherwise record it as accepted.
  - **Deviation 25:** in CI, save caches only from `rust-desktop/integration` and `main`, and give each job one key. Report the cache total before and after.
- **Tests first:**
  - `unknown_api_path_is_404_not_found`;
  - `wrong_method_is_405`;
  - `stray_wal_files_are_swept`.
- **Done when:** `rtk cargo test -p quark-core --lib -- http:: runtime::cache::columnar_files && QUARK_TEST_BACKEND=rust rtk uv run pytest -q`.
- **Commit:** `fix(core): answer unknown routes like FastAPI and sweep stray cache logs`

#### E5 · Exit documents · batch 15
- **Files:** `docs/plans/2026-10-13-rust-desktop-parity-exit-deviations.md` (new), `docs/plans/2026-10-04-rust-desktop-pivot-design.md`, `docs/plans/2026-10-08-rust-desktop-parity-design.md` (status line), `docs/handoffs/PARITY_H2_CHECKLIST.md` (new).
- **Needs:** E1, E3, E4, W4.
- **Build:**
  - **Deviations doc,** in SP1's format: every deviation the run log and the exit reviewers recorded, each with spec section, code behaviour and reason, ready for sign-off. It includes at least:
    - the TIME line span (C8);
    - join allow-lists (J2);
    - appender types (X5), if any;
    - the bad-workbook 400 (W3);
    - casefold (X2).
  - **H2 checklist:** SP1's remaining steps plus every SP2 feature on macOS, Windows and Ubuntu, including an export saved to disk on each.
  - **Pivot spec:** sub-project 2's status.
- **Tests first:** none. These are documents; the check is that every run-log deviation is listed.
- **Done when:** the documents exist and every deviation in the run log appears in the deviations doc.
- **Commit:** `docs(parity): record exit deviations and the H2 checklist`

## 15. Exit gate (Lead, after the last batch)

- [ ] The latest commit on `rust-desktop/parity` has every CI job green, including the full Rust-mode pytest suite on all three OSes. `tests/rust_pending.txt` is empty.
- [ ] A dispatched bundle run is green on all three OSes, transport self-test included.
- [ ] E1's performance report meets every confirmed cap, and every Rust median beats Python's.
- [ ] E2 and E3's WebKit passes are green.
- [ ] Three fresh exit reviewers each walk one part of the spec and list every deviation, using the section 16 table:
  - spec §1–§4;
  - spec §5–§7;
  - spec §8–§14.
- [ ] H2: a human runs `docs/handoffs/PARITY_H2_CHECKLIST.md` with the bundle-run installers on macOS, Windows and Ubuntu, and the results are recorded.
- [ ] The human signs off every deviation in the exit deviations doc.
- [ ] Clean up and push:
  - [ ] Remove the remaining worktrees, including the gate worktree.
  - [ ] Run `git config --unset gc.auto`.
  - [ ] Push `rust-desktop/parity`. Do not merge, and do not open a pull request.

## 16. Spec coverage

| Spec section | Units |
|---|---|
| §2 Decisions: tests | F6, every **pytest it makes pass** list |
| §2 Decisions: branches | Pre-flight (section 1), section 15 |
| §4.1–§4.3 Layers, layout, rules | R1–R6 |
| §4.4 Enforcement | R7 |
| §4.5 Dependencies | R6 (`EngineFor`, `Project`, `Format`), F4 (`Reader`) |
| §4.6 Reshaping SP1's code | R1–R7 |
| §5.1 Read pool | F2, F3, F4 |
| §5.2 Cancellation | F3, F4, V4, V5, V6, C2 |
| §5.3 Caches | F5 (responses), V6 (find tables), R7 (generation) |
| §5.4 ICU and time zones | F1, F9, C8 |
| §5.5 Join workspaces | J1, J2 |
| §6 Routes | V4, V5, V6, V7, C2–C8, J1, J2, X3–X5, W3 |
| §6.1 Charts | C1–C8 |
| §6.2 Exports | X1–X5 |
| §6.3 Workbooks | F8, W1–W4 |
| §7 Behaviour fixes | C7 (1), C5 (2), J2 (3), W3 (error path) |
| §8.1 pytest against Rust | F6, W4 |
| §8.2 Rust unit tests | every **Tests first** list |
| §8.3 Concurrency | F2, F3, F4 |
| §8.4 Architecture | R7 |
| §8.5 WebKit pass | E2, E3 |
| §9 Performance | F7, E1 |
| §10 Carried over | F9 (dev 4), R7 (dev 13), F3/F4 (dev 16), E1 (dev 21, 22), E4 (dev 24, 25, `.wal`, 501), section 15 (H2) |
| §11 Sprints | Section 5 |
| §12 Exit gate | Section 15 |
| §13 Risks | F1 and the sprint 1 gate (CMake), F8 and W4 (calamine), F3 and F4 (pool), E3 (blob download), section 1.10 (machine limits) |

## 17. Amendments during the run

The Lead records here every change to the cards, the schedule and the environment made while the plan runs. Deviations from the spec listed here go to the exit deviations doc (E5) for sign-off.

**Schedule**
- spec §4.6 wants `perf_acceptance` to give the same results after the reshape. That release-mode run needs an idle machine, so it moves from the R7 gate to the batch 9 idle slot, beside F7's timing run.
- F1 ran in batches 1–3 instead of batch 8, before the reshape finished. Spec §5.4 makes ICU the first card of sprint 1. F1 touches no source file, so it does not depend on R1–R7. Sprint 1 closed after batch 3 instead of batch 2.

**Lead units** (not in the original plan)
- L1 · batch 1 · `ci: pin the Python that uv uses to 3.13`. CI was red on 9d37bde. On `ubuntu-22.04`, `uv sync --frozen` downloaded CPython 3.15.0, which has no `pyarrow 25.0.1` wheel. Every `setup-uv` step now pins `python-version: "3.13"`.
- L2 · batch 4 · dispatched CI runs get their own concurrency group, so a push to the branch no longer cancels a bundle run, and the reverse.
- L3 · batch 6 · `fix(core): open engines and remount outside the catalog lock`. The sprint 2 audit found that, since SP1, `engine_for_node` and `workspace_in` held the global catalog lock while DuckDB opened engines and mounted sources. That breaks the section 2 rule. The fix is in `runtime/engines.rs` and has three steps:
  1. Snapshot the catalog state under the lock.
  2. Open the engine and remount with no lock held.
  3. Publish under the lock. An engine already published for the node wins. If the project's sources changed during the open, retry; a node that has gone answers its usual 404.
  - **Deviation for sign-off:** after 8 attempts in a row lose to catalog changes, the open falls back to running under the catalog lock (SP1's behaviour), so the call always ends. This is a pathological case only.
  - **Follow-up, folded into R7:** an engine that loses the race must be dropped after the catalog guard is released.

- L4 · batch 9 · `fix(core): render export decimals the way Python's str(Decimal) does`. Sprint 4 audit (spec §6.2): X1 printed decimals with Arrow's text, but Python switches to scientific notation below an adjusted exponent of -6 (`0.00000012` → `1.2E-7`).
- L5 · batch 11 · `test(core): resolve paths before comparing and retry the read-overlap timing`. Sprint 5 CI was red on macOS and Windows: two V7 tests compared unresolved temp-dir paths, and F3's overlap timing test flaked on Ubuntu. The tests changed; the production code did not.
- L6 · batch 13 · `ci: run the live WebKit passes against the Rust core`. This container has only Chromium and may not run `playwright install`, so CI's new `live` job runs E2's and E3's passes in WebKit, and local runs use Chromium as a smoke check.
- L7 · batch 14 · `fix(core): label histogram series as Python prints them and prove scatter samples repeat`. Sprint 6 audit (spec §6.1, §7 fix 1): histogram labels used SQL `CAST` text instead of Python `str()`, and the scatter repeatability tests were served from the response cache.
- L8 · batch 15 · `fix(core): replace the last 501 answers and give the live CI job room`. Sprint 7 audit (spec §10): two internal arms still answered 501 (a join side with no scan expression, an unreachable export arm). The `live` job's timeout goes from 30 to 50 minutes, because parity pushes build cold.
- L9 · batch 15 · `test(ui): draw every chart kind on the dashboard in the WebKit pass`. Spec §8.5 asks for every kind on both canvases; E3's dashboard had four tiles.
- L10 · batch 15 · `test(perf): time the routes, not the response cache`. Sprint 7 audit (spec §9): the values, chart and Find Next cases repeated one request, so 4 of 5 runs were response-cache hits. compare.py also joined before the second source's import finished, and rewrote the whole results doc.
- L11 · batch 16 · `fix(core): match Python on xlsx durations, join side errors, line spans and home expansion`. Exit review (spec §6.1, §6.2, §6): XLSX durations print as `[hh]:mm:ss`, a dataset-side join error answers 422 as Python, the line grain uses a fractional span, and `~` reads `USERPROFILE` on Windows.
- L12 · batch 16 · `perf(core): stream export rows instead of materialising them`. Exit review (spec §6.2): export rows were collected before the writer ran, so a large export held every row in memory. The rows now stream through the writer; a closing flag row detects a stream that stopped early (deviations doc, items on L12).
- L13 · batch 16 · `test(core): pin cancellation, export overlap, workbook and extension rules; move inline tests`. Exit review (spec §4.4, §8): inline test modules move to `tests.rs` with an architecture check; `tests/` files carry a `//!` line, also checked; new tests for an HTTP drop on a non-page route, pages during an export (not ignored), a workbook with no sheets and a file with no Excel extension; `compare.py` uses the 404/405 wording.
- L14 · batch 16 · `test(core): give the columnar shutdown tests room on slow runners`. A Lead direct unit (7c7b224). CI run 38589489926 failed `shutdown_interrupts_a_running_import` on Windows: stopping took over 2 s under a full parallel test load. Both columnar shutdown tests now use an hours-long scan and a 20 s limit, and the first waits on a channel, so a shutdown that stops interrupting fails instead of hanging.

**Schedule, exit stream**
- E1 and E3 split their work across batches 14 and 14b after reaching the 200-line split rule; the Lead accepted their sizes and resumed them.
- Batch 15 runs L8–L10, then an idle-slot release perf run, idle and under load. Spec §9 asks for margins under load, as SP1's deviations 21 and 22 recorded.
- Batch 16 runs the three exit reviewers of section 15 first, then E5, so the deviations doc holds what they find.

**F1, revised.** The card below replaces the card in section 8.
- **Why:** the crates.io package of `libduckdb-sys 1.10504.0` ships no DuckDB sources, so the `icu` feature (`bundled-cmake`) cannot build from it.
- **Deviation for sign-off:**
  - `duckdb` comes from the duckdb-rs git tag `v1.10504.0`, whose submodule is DuckDB `v1.5.4`.
  - A CMake toolchain file pins the reported engine version, because the cargo checkout of the submodule has no tags.
- **Measured costs:**
  - first ICU dev build: 22m07s against 19m27s (+14%);
  - release `quark-dev-server`: 52.6 MB against 45.3 MB (+16%).

#### F1 (rev. 2, as integrated in d3b9e87) · ICU in DuckDB · batch 3 (card revised by the Lead after the batch 1 block and amended after the batch 2 escalation)
- **Why revised:** the crates.io package of `libduckdb-sys 1.10504.0` excludes `duckdb-sources`, so the `icu` feature (which turns on `bundled-cmake`) panics in its build script: "`bundled-cmake` requires a duckdb-rs checkout with DuckDB sources". The duckdb-rs git tag `v1.10504.0` (commit 452fc0b6) carries `crates/libduckdb-sys/duckdb-sources` as a submodule pinned to DuckDB `v1.5.4` (commit 08e34c4), the same engine the spec requires. The Lead chose this over the spec §5.4 UTC fallback; it is recorded as a deviation for the human's sign-off.
- **Files:** `Cargo.toml`, `Cargo.lock`, `core/tests/time_zones.rs` (new), `.cargo/config.toml` (new), `cmake/duckdb-version.cmake` (new).
- **Lead decision (batch 2 escalation):** the cargo checkout of the `duckdb-sources` submodule has no git tags, so DuckDB's CMake build reports `v0.0.1`. DuckDB reads `OVERRIDE_GIT_DESCRIBE` only as a CMake variable, so the version is pinned through a CMake toolchain file: `.cargo/config.toml` sets `[env] CMAKE_TOOLCHAIN_FILE = { value = "cmake/duckdb-version.cmake", relative = true }`, and that file sets `OVERRIDE_GIT_DESCRIBE` to `v1.5.4`. The Lead accepts these two files and records them with the ICU deviation. Known limits, documented in the cmake file's comment: a bump of the pin needs `cargo clean -p libduckdb-sys` (the build script does not watch the file); an externally set `CMAKE_TOOLCHAIN_FILE` overrides the pin (`engine_version` catches it); the pin applies to cargo runs inside the repository.
- **Needs:** none.
- **Build:**
  - The workspace dependency becomes `duckdb = { version = "=1.10504.0", git = "https://github.com/duckdb/duckdb-rs", tag = "v1.10504.0", features = ["bundled", "parquet", "json", "icu"] }`. Cargo fetches the `duckdb-sources` submodule itself. Regenerate `Cargo.lock` with cargo (`cargo update -p duckdb -p libduckdb-sys` or a build), never by hand; only the `duckdb` and `libduckdb-sys` entries (and their source lines) should change. If cargo pulls in any other changed package version, report it.
  - Nothing else changes beyond the version pin above. `engine_version` must still report `v1.5.4`. `[profile.dev.package.libduckdb-sys] opt-level = 3` stays.
  - The unit report records the build costs:
    - wall time of your first `cargo build -p quark-core` with `icu` on (DuckDB builds from scratch through CMake there), against the Lead's before-numbers in `/home/user/quark-wt/f1-before.txt`;
    - the size of a release `quark-dev-server`: run `nice -n 19 env CARGO_TARGET_DIR=/home/user/quark-wt/release-f1 cargo build -j 2 --release -p quark-core --bin quark-dev-server` (in the background, Bash timeout 90 minutes, poll), record its size, then `rm -rf /home/user/quark-wt/release-f1`. If it has not finished when everything else is done and committed, report "release size: pending" rather than waiting past 90 minutes.
- **Tests first** (`time_zones.rs`, using only the `duckdb` crate, never `quark_core` paths; each test opens an in-memory connection):
  - `icu_is_loaded`: `SELECT loaded FROM duckdb_extensions() WHERE extension_name = 'icu'` is `true`.
  - `session_zone_reads_offsetless_timestamptz`: after `SET TimeZone = 'Asia/Dubai'`, `TIMESTAMPTZ '2024-01-01 10:00:00' = TIMESTAMPTZ '2024-01-01 06:00:00+00'` is `true`.
  - `date_trunc_and_strftime_use_the_session_zone` (Review Focus 4): after `SET TimeZone = 'America/New_York'`:
    - `strftime(date_trunc('day', TIMESTAMPTZ '2024-03-10 03:30:00+00'), '%Y-%m-%d')` is `2024-03-09`;
    - `strftime(TIMESTAMPTZ '2024-01-01 02:00:00+00', '%Y')` is `2023`.
  - `default_zone_is_set`: `current_setting('TimeZone')` is not empty.
- **Hot spots:** only the dependency source and feature list change; `cargo test --locked` must work from a clean checkout (CI fetches the submodule through cargo); universal macOS builds both architectures through CMake (the sprint gate's bundle run checks this).
- **Done when:** `cargo test --locked -p quark-core --test time_zones --test engine_version && cargo test -p quark-core && cargo clippy -p quark-core --all-targets -- -D warnings && cargo fmt --all --check`, and `git diff rust-desktop/parity -- Cargo.lock` changes only the `duckdb` and `libduckdb-sys` entries and adds their build dependencies `cmake` and `which`.
- **Commit:** `build(core): compile DuckDB with ICU`
