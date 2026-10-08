# Quark desktop pivot

Date: 2026-10-04. Status: decisions approved 2026-10-01 and 2026-10-02; sub-project 1 implemented. This document is the single outline of the pivot and its sub-projects. Each sub-project has its own design, plan and implementation, linked below.

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
| 1 | Walking skeleton | Tauri app and Rust core on three OSes; projects, upload, paging, filter, sort and dedupe end to end; efficiency upgrades; CI | Implemented; awaiting H2 and deviation sign-off |
| 2 | Backend parity | Every remaining endpoint in Rust; offline XLSX; the pytest suite moved to Rust | Not started |
| 3 | Native files and OS integration | Open files in place, native dialogs, drag and drop, native export save, file associations, single instance, default-app card | Not started |
| 4 | Release | Signing, notarization, installers, updates, app icon, version scheme | Not started |
| 5 | Retire Python | Remove FastAPI, Docker and pytest; update README, PRODUCT.md and SPEC.md | Not started |

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
- **Branch:** `rust-desktop/skeleton`, at de640e9. CI and installer builds are green on all three OSes, and the Rust-mode skeleton suite passes 41 of 41.
- **Remaining:**
  - H2: the manual smoke test (skeleton design, section 8.5) with the bundle-run installers. Those artifacts expire on 2026-10-18.
  - Sign-off on the 25 listed deviations.

### 3.2 Backend parity

Port every remaining endpoint so the Rust core can replace FastAPI.

- **Scope:**
  - Column stats, category values, charts, joins, exports, cell find and attach.
  - XLSX, including an offline replacement for `INSTALL excel`.
  - Time-zone handling: ICU versus the Rust side.
  - Parallel reads, deferred from the skeleton.
  - Whether the legacy routes survive.
  - Moving the remaining tests to Rust.
- **Open:** whether the legacy routes survive, and the time-zone approach. Both are settled in this sub-project's design.

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
  - Delete the Python backend, pytest and the Docker files.
  - Update README, SPEC.md, and PRODUCT.md (Tauri and Rust, plus the cache clarification).
- **Until then:** the Python backend keeps working unchanged, and the pytest suite stays the parity check for the Rust core.

## 4. Branches

- `rust-desktop/integration` is the pivot's integration branch, cut from main at 0270c8d.
- Sub-project 1 lives on `rust-desktop/skeleton`, branched from integration.
- No merge into integration or main, and no pull request, without the user's go-ahead.

## 5. Open questions for the pivot

- The branch for each later sub-project. Likely `rust-desktop/<name>` off integration; not yet decided.
- When the skeleton branch merges into integration.
