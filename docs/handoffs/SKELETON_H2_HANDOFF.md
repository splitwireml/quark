# Handoff: Quark desktop skeleton, H2 smoke test and closing batch

Next session focus: run the **H2 human smoke test** with the user, collect results per OS, then fix any failures and the deviations the user wants changed, as a closing batch. You are the **Lead** of an orchestrated run. Workers are Sonnet 5.5 (effort xhigh), and managers and reviewers are Opus 5.5 (effort high).

## Where things stand (2026-10-04)

- Sub-project 1 (walking skeleton) of the Rust desktop pivot is fully implemented on branch `rust-desktop/skeleton` @ `de640e9`, pushed to `origin` (github.com/splitwireml/quark). The user said: no merge, no PR.
- Final CI run 37192828724 and installer run 37194514409 are green on macOS, Windows and Ubuntu, with the self-test passing.
- Every worktree is removed, `gc.auto` is unset again, and the main checkout `/Users/mali/Development/quark` is clean on `rust-desktop/skeleton`.
- Exit-gate items still open (plan §12):
  1. **H2**: the user runs spec §8.5 with the CI installers on 3 OSes.
  2. **Deviation sign-off**: 25 items in `docs/plans/2026-10-04-skeleton-exit-deviations.md`. The user marks each one accept or fix.

## Read these instead of re-deriving

| What | Where |
|---|---|
| Plan (team, batches, unit cards, §12 exit gate) | `docs/plans/2026-10-02-rust-desktop-skeleton.md` (~1.9k lines; read by section with `rg -n '^## '`) |
| Spec (§8.5 smoke steps, §11 fallbacks) | `docs/plans/2026-10-02-rust-desktop-skeleton-design.md` |
| Deviations for sign-off | `docs/plans/2026-10-04-skeleton-exit-deviations.md` |
| Full run log (every batch, gate, CI run, decision) | `/Users/mali/Development/quark-wt/runlog.md` (outside repo) |
| Lead-written fix cards (format to copy) | `/Users/mali/Development/quark-wt/cards/*.md` |
| Batch workflow script (workers, then TM review, fix rounds, ff-only integrate) | `/Users/mali/Development/quark-wt/quark-batch.js` |
| Parity loop script (C12) | `/Users/mali/Development/quark-wt/quark-parity.js` |
| Agent role definitions | `.claude/agents/quark-{worker,task-manager,sprint-manager,exit-reviewer}.md` (committed) |
| Commit history of the run | `git log 0ba6375..de640e9` |

## H2: what to give the user

Installer downloads (bundle run 37304898957 at `3abceb1`, which includes the H2 fixes; the artifacts expire **2026-10-19**; after that, re-dispatch with `gh workflow run ci.yml --ref rust-desktop/skeleton`):
- macOS universal .dmg: https://github.com/splitwireml/quark/actions/runs/37304898957/artifacts/11344612316
- Windows .exe and .msi: https://github.com/splitwireml/quark/actions/runs/37304898957/artifacts/11344477609
- Ubuntu .deb: https://github.com/splitwireml/quark/actions/runs/37304898957/artifacts/11343503293

The steps are spec §8.5 (6 steps). The builds are unsigned. On macOS 15 and later, right-click and Open no longer bypasses Gatekeeper: after the first blocked launch, use Open Anyway in System Settings > Privacy & Security, or run `xattr -dr com.apple.quarantine /Applications/Quark.app`. On Windows, SmartScreen needs "Run anyway". The 2M-row CSV for step 2 is not in the repo; D12's `crates/quark-core/tests/perf_acceptance.rs` shows how one is generated, or the user brings their own. Record the results per OS (step, pass or fail, notes) in the run log.

## Closing batch: how to run fixes

1. Write a fix card per failure or "fix" deviation in `/Users/mali/Development/quark-wt/cards/<name>.md`, in the same format as the existing cards: Files, Why, Build, Tests first, Done when, Commit, plus a "Group gate" block. Keep units atomic: 3 files or fewer and about 150 lines, with distinct files across units that run in parallel.
2. Recreate the gate worktree (removed at cleanup): `git config gc.auto 0`, then `git worktree add --detach /Users/mali/Development/quark-wt/gate rust-desktop/skeleton`, and warm it (`cargo build -p quark-core`, `npm ci` in `frontend/`, `uv sync --frozen`). Expect one cold DuckDB compile of about 10 to 15 minutes.
3. Per unit: `git worktree add -b rust-desktop/skeleton-<ID> /Users/mali/Development/quark-wt/<ID> rust-desktop/skeleton`, then `cp -cR` the gate's `target/` (and `frontend/node_modules` for UI or Tauri units, plus `uv sync --frozen` for pytest units).
4. Launch `Workflow({scriptPath: '/Users/mali/Development/quark-wt/quark-batch.js', args: {batch, warm: true, workerNotes, tmNotes, groups: [{id, hotFile, hotLines, units: [{id, file, lines}]}]}})`. See earlier args in the run log or session for wording.
5. After it reports: run the batch gate in the gate worktree (`cargo test -p quark-core --locked`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, `cargo fmt --all --check`, Rust-mode `QUARK_TEST_BACKEND=rust uv run pytest -m skeleton -q`, `uv run pytest -q`), then push `rust-desktop/skeleton` and watch CI with `gh run watch <id> --exit-status`. Dispatch a bundle run only after the push run finishes (the concurrency group cancels in-progress runs).
6. Finish with cleanup: remove the worktrees including gate, run `git config --unset gc.auto`, and update the run log and memory.

## Hard constraints learned this run

- **The user's Mac (10 cores, 16 GB) crashed apps** when 3 workflows ran cargo and DuckDB builds in parallel (load average about 92). Run **one workflow at a time** and keep stress tests to 4 parallel copies or fewer. `/Users/mali/Development/quark-wt/.cargo/config.toml` caps builds there at `jobs = 4`, and `.metadata_never_index` stops Spotlight indexing that folder. Recreate both if `quark-wt/` gets wiped.
- **User wants autonomous fixing**: fix blockers and continue, don't stop to ask. The exceptions are the H2 results and the deviation decisions themselves, which are the user's call.
- Python-parity policy: when the Rust and Python backends differ, match Python (user decision).
- Never pipe `git merge --ff-only`; check its exit status, because a masked failure once deleted a branch. Cargo multi-filter tests need `--` before the filters (`cargo test -p quark-core -- a:: b::`).
- Windows-only issues show up only in CI: no `<>:"|?*` in fixture file names, and files DuckDB holds open can't be opened for writing (use `FILE_WRITE_ATTRIBUTES` the way `ColumnarKey::touch` does).
- macOS 27 local release builds need `[profile.release.build-override] strip = false` (already in `Cargo.toml`).
- Commit trailer: workers use `Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>`, and Lead commits use the Opus 5.5 trailer. Prefix shell commands with `rtk` (see `CLAUDE.md`).
- Session restarts kill background workflows and agents. Resume with `Workflow({scriptPath, resumeFromRunId, args: <identical args>})`; completed agents replay from cache. Reset any half-edited worktrees first.

## After H2 and sign-off

Sub-project 1 closes. Next is sub-project 2 (backend parity: port the rest of `backend/app.py`), which needs its own spec and plan first. See memory `rust-desktop-pivot.md` for the 5 sub-projects and their parked open items.

## Suggested skills

- `superpowers:subagent-driven-development`: the plan header requires it for executing units.
- `workflow-authoring`: before editing or launching `quark-batch.js` and similar workflow scripts.
- `superpowers:systematic-debugging`: for any H2 failure before writing a fix card.
- `rust-skills`: only for a specific rule file when a fix needs it (workers use the digest in plan §2).
- `superpowers:verification-before-completion`: before declaring the exit gate closed.
- `superpowers:finishing-a-development-branch`: only if the user later asks to merge; the default is no merge and no PR.
