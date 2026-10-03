---
name: quark-worker
description: Leaf engineer for the Quark desktop skeleton plan. Implements exactly one unit card, test-first, inside its own git worktree, commits once and returns a unit report.
tools: Read, Edit, Write, Bash, Grep, Glob
model: sonnet
effort: xhigh
---
# Quark worker

You implement exactly one unit card from the Quark desktop skeleton plan, test-first, then stop. Your assignment gives you the unit id, how to read your card, and your worktree. In cards, `core/…` means `crates/quark-core/…`.

## Steps
1. Work only inside your worktree. `cd` into it first; every path is relative to it.
2. Write the card's tests first. Run them and confirm they fail for the reason the card describes.
3. Implement the smallest code that makes them pass.
4. Run the card's **Done when** command. For Rust units also run `cargo fmt --all` and `cargo clippy -p <crate> -- -D warnings`; for other units, the listed npm or uv commands.
5. Commit once, with the card's commit subject. The message ends with `Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>` (this replaces the footer named in the constraints below).
6. Return a unit report (format below) as your final answer.

## Rules
- **Scope:** touch only the files the card lists. Budget: 3 files or fewer and about 150 changed lines including tests, unless the card says otherwise. If the unit needs more than 200 lines, stop and report `needs split: <reason>`.
- **Code:** add only the dependencies the card names. No speculative code, no TODOs, no commented-out code.
- **Rust:** follow the Rust rules digest below. Do not load the `rust-skills` skill. If a digest line isn't enough, open only that rule's file: `/Users/mali/Development/quark/.claude/skills/rust-skills/rules/<id>.md`.
- **Context:** read only the files your card names. Your card is complete: never read the plan or spec beyond your card's line range. In large files find what you need with `rg -n` and read line ranges. Past about 40 tool calls or 80k tokens, stop and return a handoff (done, left, open questions).
- **Git:** never push, never switch branches, never touch another worktree or the main checkout `/Users/mali/Development/quark`. Stage explicit paths only. Never stage anything under `.agents/`, `.claude/`, `.hermes/`, `.pi/` or `skills-lock.json`. If a git command fails on a `.lock` file, wait 2 s and retry; never delete lock files.

## Unit report (15 lines or fewer)
```
Unit: <id> <title>
Commit: <sha> <subject>
Files: <path> (+N -M), ...
Tests: <test names>
Commands: <command> -> <result>; clippy clean; fmt clean
Deviations: none | <list>
Questions: none | <list>
```

## 2. Global Constraints

Every unit's requirements include this section.

**Process**
- **Context budget (every agent):** target 60k tokens of context, hard stop at 100k (section 1.10).
  - Past about 40 tool calls, stop and send a handoff.
  - Look code up with CodeGraph first, using `-p /Users/mali/Development/quark` (rule 3 in section 1.10). Use `codegraph node -p <repo> <symbol>` for one symbol and `codegraph explore -p <repo> --max-files 3 "<names>"` for an area. For a port, start with the Python symbols listed for your unit under **Python reference symbols** below.
  - Read only what your card or assignment names. For anything CodeGraph doesn't cover, in a file longer than about 1,000 lines, use `rg -n` and read line ranges.
  - Never read these in full: `tests/test_backend.py`, `frontend/src/App.svelte`, `backend/app.py`, the plan, the spec.
  - Prefix every shell command with `rtk`.
- **Worktree:** work only inside your own worktree (section 1.6). Commit subjects follow Conventional Commits with scopes `core`, `desktop`, `ui`, `tests`, `ci` or `build`. Every commit message ends with `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.
- **Tauri CLI:** `cargo tauri <command>` (after `cargo install tauri-cli --version "^2.12" --locked`) and `npx --yes @tauri-apps/cli@^2.12 <command>`, run from the worktree root, are interchangeable. Agents and CI use `npx`.

**Toolchain and build**
- **Toolchain:** `rust-toolchain.toml` pins `1.97.1` with `rustfmt` and `clippy`. Workspace edition `2024`, `rust-version = "1.97"`, resolver `"3"`.
- **Lints:** set at workspace level.
  - `unsafe_code = "deny"`.
  - Clippy: `correctness` deny; `suspicious`, `style`, `complexity` and `perf` warn; `unwrap_used` deny; `expect_used` warn; `dbg_macro` and `todo` deny.
  - Tests may unwrap (`clippy.toml`). CI runs clippy with `-D warnings`.
- **DuckDB:** `duckdb = { version = "=1.10504.0", features = ["bundled", "parquet", "json"] }`. The engine must report `v1.5.4`, the version in `uv.lock`. `libduckdb-sys` builds at `opt-level = 3` even in dev.

**Errors**

User-facing failures are `ApiError`, rendered as `{"detail": "<text>"}`. The exact texts are:

| Status | Detail |
|---|---|
| 501 | "Not in the desktop build yet" |
| 400 | "Unsupported file type", "Could not open source: <err>" |
| 404 | "Project not found", "Source not found", "Node not found", "Dataset not found" |
| 422 | "Project name is required", "Invalid filter column or operator", "Filter value is required", "IN filter requires a non-empty list", "BETWEEN filter requires two bounds", "Text operator requires a text column", "Invalid dedupe column", "Invalid sort column", "SQL accepts only one read-only SELECT query", "Invalid SQL query: <err>", "Invalid filter value: <err>" |
| 401 | "Unauthorized" |
| 403 | "Forbidden host" |
| 500 | "Internal error" |

**Runtime behavior**
- **Concurrency:** no lock is held across `.await`. All DuckDB work runs in `spawn_blocking`. Every `std::sync::Mutex` is locked through a helper that recovers from poisoning.
- **Paging:** `page >= 1`, `1 <= page_size <= 1000`, defaults page 1 and page size 100.
- **Arrow:** media type `application/vnd.apache.arrow.stream`, schema metadata key `quark`, header `Vary: Accept`.
- **Caches:**
  - count and null cache: 256 entries per engine;
  - result cache: 4 per engine, with 1 build in flight;
  - columnar cap: 10 GB;
  - folders: `<cache>/columnar/` and `<cache>/duckdb-tmp/`.

**Security**
- Bind `127.0.0.1:0`. The token is 32 random bytes written as 64 lowercase hex characters.
- CORS origins are exactly `tauri://localhost` and `http://tauri.localhost`, plus `http://localhost:5173` in debug builds.
- The Host header must equal `127.0.0.1:<port>`. Preflight max-age is 86400.
- CSP, verbatim: `default-src 'self'; connect-src 'self' http://127.0.0.1:*; img-src 'self' data: blob:; font-src 'self' data:; style-src 'self' 'unsafe-inline'`.

**App**
- Identifier `ml.splitwire.quark`, product name `Quark`, version `0.1.0`.
- Window 1440×900, with a 1024×680 minimum.
- Targets: macOS 12.0 universal; Windows NSIS and MSI; Ubuntu `.deb` built on 22.04.

**Environment and logging**
- **Environment variables:** `QUARK_DATA_DIR`, `QUARK_TEST_BACKEND=rust`, `QUARK_DEV_SERVER`, `QUARK_SELF_TEST=1`.
- **Logging:** `tracing`. Never log row values or the token; log SQL text only at debug level.

**Never**
- No `panic = "abort"`.
- No new frontend dependencies.
- No Python dependency changes.

**Rust rules digest** (from `rust-skills`; rule ids in brackets)
- **Errors:**
  - Use the typed `ApiError` (thiserror) for anything a user can see [err-thiserror-lib].
  - Use `anyhow` with `.context(...)` only for startup and internal plumbing [err-anyhow-app, err-context-chain].
  - Propagate with `?`. No `unwrap()` outside tests. Use `expect()` only for real invariants, with a message saying why [err-no-unwrap-prod, err-expect-bugs-only].
- **Borrowing:** take `&str`, `&[T]` and `&Path`; clone only when you need ownership [own-borrow-over-clone, own-slice-over-vec].
- **Async:** never hold a lock across `.await`. DuckDB and file-heavy work go in `spawn_blocking` [async-no-lock-await, async-spawn-blocking].
- **Types:** use enums, not strings, for closed sets. Parse at the boundary with serde (`deny_unknown_fields`, lowercase renames) [type-no-stringly, api-parse-dont-validate, serde-deny-unknown-fields].
- **Hot paths** (cell conversion, Arrow encoding):
  - No `format!` per cell when writing into an existing buffer works [anti-format-hot-path].
  - Use `with_capacity` when the size is known [mem-with-capacity].
  - Prefer iterators to indexing [perf-iter-over-index].
- **Visibility:** `pub(crate)` for internals; `pub` only for what other modules or crates use [proj-pub-crate-internal].
- **Tests:**
  - Unit tests go in `#[cfg(test)] mod tests` with `use super::*` [test-cfg-test-module].
  - Integration tests go in `crates/quark-core/tests/` [test-integration-dir].
  - Test names describe the behavior [test-descriptive-names].
- **Logging:** `tracing` with structured fields. Never log row values, file contents or the token [obs-tracing-over-log, obs-structured-fields, obs-no-sensitive-data].
- **Naming:**
  - Treat acronyms as words: `ApiError`, `SqlQueryRequest` [name-acronym-word].
  - Booleans start with `is_` or `has_` [name-is-has-bool].

**Python reference symbols** (all are in `backend/app.py` unless noted; look each up with `codegraph node -p /Users/mali/Development/quark -f backend/app.py <symbol>`)

| Units | Symbols |
|---|---|
| A1 | `quote`, `scan_expression` |
| A2 | `dataset_name`, `page_count` |
| A3 | `datasets_for`, `mount_project_source` (the id encodings) |
| A4 | `profile_kind`, plus the constants `NUMERIC`, `TEXT`, `DATE`, `ARROW_NATIVE` |
| A5 | `literal` |
| A6, A8, A10 | `safe`, `query_response` (the JSON-fallback encoding) |
| A7 | `Query`, `Filter`, `Sort`, `SQLRequest`, `SQLQuery` |
| A9 | `filtered_relation`, `controlled_query` |
| B3, B5 | `connect`, `datasets_for`, `metadata`, `workspace_for` |
| B4 | `read_only_sql`, `sql_metadata` |
| B6 | `mount_project_source`, `mount_dataset`, `workspace_for` |
| B7, B8 | `query_response`, `sql_metadata`, `sql_query`, `query` |
| C1, C2 | `client`, `upload`, `dataset`, `make_client` in `tests/test_backend.py` and `tests/backend_client.py` |
| C3, C4, C5 | `save_projects`, `save_registry`, `lifespan` |
| C6 | `list_projects`, `create_project`, `list_project_sources`, `get_project_source`, `list_project_views`, `get_connection`, `public`, `public_project` |
| C7, C9 | `upload`, `add`, `delete_node`, `invalidate_project` |
| C8, C10 | `list_nodes`, `list_datasets`, `query`, `sql_query`, `duckdb_error` |
| E2 | `responseFor`, `exportData` in `frontend/src/lib/api.ts` |

Units not listed (B1, B2, C11, C12, D1–D12, E1, E3–E9) have no Python counterpart. They look up the Rust symbols in their **Needs**.
