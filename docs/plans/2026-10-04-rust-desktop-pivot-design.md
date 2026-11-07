# Quark desktop pivot

Date: 2026-10-04, updated 2026-10-08. Status: decisions approved 2026-10-01 and 2026-10-02; sub-project 1 implemented and merged into integration; sub-project 2 implemented on `rust-desktop/parity`, exit gate awaiting H2 and deviation sign-off. This document is the single outline of the pivot and its sub-projects. Each sub-project has its own design, plan and implementation, linked below.

## 1. Context

Quark is a local-first DuckDB data viewer. Today a Svelte 5 single-page app talks to a single-file FastAPI backend (`backend/app.py`, about 2,000 lines and 30 routes) that owns DuckDB connections and query safety. It runs as a web server (uvicorn or Docker) and is used from a browser.

Quark is becoming a desktop-only application: a Tauri 2 shell around the existing Svelte UI, with the backend rewritten in Rust, on macOS, Windows and Ubuntu. Once the Rust app reaches parity, FastAPI, the Dockerfile and compose are retired.

## 2. Decisions for the whole pivot

Made during brainstorming on 2026-10-01 and 2026-10-02:

- **Scope:** Tauri 2, the existing Svelte UI, and the backend ported to Rust. No Python runtime ships with the app.
- **Desktop only:** no headless or web mode after parity.
- **Platforms:** macOS 12+ as a universal build (Apple Silicon and Intel), Windows 10 and 11 on x64, Ubuntu 22.04+ on x64 as a `.deb`.
- **Transport:** the UI reaches the backend over HTTP on `127.0.0.1` with a per-launch token. A custom `quark://` URL scheme is the fallback.
- **Code standard:** Rust code follows the repository's `rust-skills` guidelines.
- **Bundle identifier:** `ml.splitwire.quark`. It is effectively permanent; changing it later moves the data folder and breaks file associations.
- **DuckDB engine:** pinned to the version the Python side uses (1.5.4), so both backends behave the same during the port.
- **File associations and the default-app card:** `.xlsx`, `.csv`, `.json` and `.parquet` open with Quark, and an in-app card recommends making Quark the default. Both belong to sub-project 3.
- **Order:** the sub-projects run in the order of section 3. Each one starts with its own design, then a plan, then implementation.

## 3. Sub-projects

| # | Sub-project | Outcome | Status |
|---|---|---|---|
| 1 | Walking skeleton | Tauri app and Rust core on three OSes; projects, upload, paging, filter, sort and dedupe end to end; efficiency upgrades; CI | Implemented; deviations signed off; merged into integration; remaining H2 steps moved to sub-project 2's exit gate |
| 2 | Backend parity | Every remaining endpoint in Rust; offline XLSX; ICU time zones; read pool and the other efficiency upgrades; pytest runs against Rust | Implemented on `rust-desktop/parity`; exit gate awaiting H2 and deviation sign-off (`2026-10-13-rust-desktop-parity-exit-deviations.md`, `docs/handoffs/PARITY_H2_CHECKLIST.md`) |
| 3 | Native files and OS integration | Open files in place, native dialogs, drag and drop, native export save, file associations, single instance, default-app card | Not started |
| 4 | Release | Signing, notarization, installers, updates, app icon, version scheme | Not started |
| 5 | Retire Python | Remove FastAPI and Docker; port pytest to Rust or keep it as a test-only tool; update README, PRODUCT.md and SPEC.md | Not started |

### 3.1 Walking skeleton

Prove the risky parts before the bulk port, and ship one production-quality vertical slice.

- **Scope:**
  - The webview-to-backend transport on WKWebView, WebView2 and WebKitGTK.
  - DuckDB's bundled build on all three OSes.
  - Arrow pages in the existing UI.
  - Projects, upload, paging, filter, sort, dedupe and Versions end to end.
  - Four efficiency upgrades: a count and null cache, stale-query cancellation, a columnar cache and a result cache.
  - The pytest suite runnable against Rust; CI and installers on three OSes.
- **Rejected:** ART indexes, which were measured and dropped.
- **Documents:**
  - Design: `2026-10-02-rust-desktop-skeleton-design.md`.
  - Plan: `2026-10-02-rust-desktop-skeleton.md`.
  - Exit deviations: `2026-10-04-skeleton-exit-deviations.md`.
- **Branch:** `rust-desktop/skeleton`, fast-forwarded into `rust-desktop/integration` at af7c0bd on 2026-10-08. CI and installer builds are green on all three OSes, and the Rust-mode skeleton suite passes 41 of 41.
- **Remaining:** the manual H2 steps (a full Windows run, creating a project and uploading into it on macOS, launching from the icon on Ubuntu, re-checking the H2 fixes). They run in sub-project 2's exit gate with its installers. The 25 deviations are signed off.

### 3.2 Backend parity

Port every remaining endpoint so the Rust core can replace FastAPI, and take the efficiency upgrades the Python version skipped.

- **Scope:** column stats, category values, charts, joins, exports, cell find, attach and the source path; offline XLSX with calamine and rust_xlsxwriter; ICU compiled into DuckDB; a read pool per engine; a response cache; three visible behaviour fixes; the code reshaped into feature folders with a CI-enforced layout.
- **Settled in its design:** the legacy routes stay until sub-project 5; time zones use ICU; the pytest suite stays the API test suite and runs against Rust, and sub-project 5 decides whether to port it.
- **Documents:** design `2026-10-08-rust-desktop-parity-design.md`; plan `2026-10-08-rust-desktop-parity.md`; orchestration map `2026-10-08-rust-desktop-parity-orchestration.md`; planning notes `2026-10-05-sp2-backend-parity-notes.md`.
- **Branch:** `rust-desktop/parity`, from integration.

### 3.3 Native files and OS integration

Make Quark behave like a native app for the files it opens.

- **Scope:**
  - Opening files in place.
  - Native open and save dialogs.
  - Drag and drop from the OS.
  - Exports saved by the backend.
  - File associations for `.xlsx`, `.csv`, `.json` and `.parquet`.
  - Single instance, with open-file routing.
  - The "make Quark the default" card. macOS and Ubuntu can set the default directly; Windows can only open its Settings page.
- **Open questions:**
  - The "recommendation widget" is read as an in-app card suggesting Quark as the default app for its file types, the way browsers do. To be confirmed.
  - Which project a double-clicked file lands in.
  - Whether `.tsv`, `.jsonl`, `.ndjson` and `.duckdb` appear under "Open with" without being offered as a default.
  - When the card appears, and how dismissing it persists.

### 3.4 Release

Ship installers people can run without security warnings, and keep them updated.

- **Scope:** Apple Developer ID signing and notarization; Windows code signing; the update channel; the real app icon (the skeleton ships a neutral placeholder); the version scheme.
- **Resolves:** the Gatekeeper and SmartScreen warnings that the skeleton's unsigned installers trigger.

### 3.5 Retire Python

Remove the web-era stack once the Rust app is at parity.

- **Scope:**
  - Delete the Python backend and the Docker files.
  - Decide whether the pytest API suite is ported to Rust or kept as a test-only tool. Either way its Python mode and `python_only` tests go.
  - Update README, SPEC.md, and PRODUCT.md (Tauri and Rust, plus the cache clarification).
- **Until then:** the Python backend keeps working unchanged, and the pytest suite stays the parity check for the Rust core.

## 4. Branches

- `rust-desktop/integration` is the pivot's integration branch, cut from main at 0270c8d. It holds sub-project 1 since 2026-10-08 (af7c0bd).
- Each sub-project lives on its own `rust-desktop/<name>` branch from integration: sub-project 1 on `rust-desktop/skeleton`, sub-project 2 on `rust-desktop/parity`.
- No merge into integration or main, and no pull request, without the user's go-ahead.

## 5. Open questions for the pivot

- When integration merges into main.
