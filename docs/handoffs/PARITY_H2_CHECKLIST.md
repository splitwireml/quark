# Quark desktop parity: H2 checklist

Sub-project 2's exit gate (design section 12 items 5 to 7) needs a human on macOS, Windows and Ubuntu. Modelled on `SKELETON_H2_CHECKLIST.md`. Use the installers from a bundle run on the final `rust-desktop/parity` commit, with the transport self-test green on all three OSes. Deviations for sign-off: `docs/plans/2026-10-13-rust-desktop-parity-exit-deviations.md`. Perf figures: `docs/plans/2026-10-13-rust-desktop-parity-perf.md`.

## Before installing

- [ ] `frontend/index.html` has no `impeccable-live` block. Today it loads `http://localhost:8400/live.js` with a token (SP1 commit 664538f), outside SP2's scope. Remove it and rotate the token before building the installers (deviation 55).
- [ ] The bundle run's self-test log has no "Error reading the log directory" line (SP1 #24, deviation 35).
- [ ] The bundle run is green on all three OSes. Record its run id under Results.
- [ ] **H0:** confirm or adjust the section 9 caps, using the perf doc's summary table. On the 4-core Linux test container, three route cases miss the provisional caps idle (stats, CSV export, workbook confirm), XLSX export passes idle by 0.1 s after L12 but misses under load, and 5 of 9 reshape steps miss. Compare against the Mac figures if you measure there.

## SP1 carry-over (design section 12 item 6)

SP1 steps, from skeleton spec section 8.5: 1 install and launch from the icon; 2 create a project, upload the 2M-row CSV, open its View; 3 scroll to the end, sort, filter, dedupe; 4 record a Version and stop; 5 quit and relaunch (project, source, View and Versions return; deep pages are fast). Step 6 (the 501 message) is gone: nothing answers 501 now.

- [ ] Windows: steps 1 to 5, a full run (not run in SP1).
- [ ] macOS: step 2 in a new project (SP1 uploaded into Default).
- [ ] Ubuntu: step 1 from the launcher icon in a normal desktop session (SP1 launched from a terminal).
- [ ] Re-check SP1's H2 fixes on each OS: the footer page after jumping to page 10,000; opening a saved Version restores its sort, filter and dedupe; dedupe appears in the Version diff; the header timing follows the page; upload shows "Uploading <name>..." with a progress bar.

## SP2 features (macOS, Windows and Ubuntu)

Use a 2M-row CSV with a numeric, a text and a date column, plus a small second CSV and a two-sheet `.xlsx`.

- [ ] Column stats for a numeric, a text and a date column, with a filter on.
- [ ] Category values: first page, search, scroll for more.
- [ ] Find: Next and Previous across columns, then after an edit to the filter.
- [ ] Charts on the Chart canvas and on the Dashboard canvas: bar, pie, histogram, box, scatter, line and metric. Group a bar, histogram or line by a column. The scatter keeps its points after a refetch.
- [ ] A same-project join (two Views of one project) and a cross-source join (two sources); keep two join Views open and use both (the browser pass never covered cross-source joins).
- [ ] **Exports saved to disk.** For each of CSV, XLSX, Parquet and JSON: export a View, confirm the file lands on disk with the offered name, and open it (the first row matches the View). This is the only check that the Tauri webview saves blob downloads (design section 13 risk; the browser pass covered CSV and XLSX in Playwright WebKit only). If a webview cannot save, add the minimal Tauri download handler from design section 2.
- [ ] Export a View with 100,000 or more rows while scrolling another View; scrolling stays smooth.
- [ ] Workbook: upload a two-sheet `.xlsx`, pick one sheet, confirm; the Views open. Upload one more and discard it.
- [ ] Bad workbook: rename a text file to `.xlsx` and upload it. The app shows "Could not open source: ..." (400), and no file is left in the uploads folder.
- [ ] Unknown routes: the app shows no 501 text anywhere ("Not in the desktop build yet"). The 404 and 405 answers need no hands-on step: `unknown_api_path_is_404_not_found` and `wrong_method_is_405` pin them, and a person cannot call the API from a terminal because the token is never logged (the app log shows only `listening on 127.0.0.1:<port>`) and a request without it is refused.
- [ ] Quit and relaunch: projects, sources, Views, Versions and workbook sources return.

## Results

| OS | Installer run id | Date | SP1 carry-over | SP2 features | Exports saved | Notes |
|---|---|---|---|---|---|---|
| macOS | | | | | | |
| Windows | | | | | | |
| Ubuntu | | | | | | |

## Exit gate (plan section 15)

- [ ] H0 caps confirmed or adjusted.
- [ ] H2 passes on all three OSes.
- [ ] Each deviation in the exit deviations doc is marked accept or fix.
- [ ] The Lead cleans up and pushes `rust-desktop/parity` (no merge, no pull request).
