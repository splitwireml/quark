# Quark desktop parity: H2 checklist

Sub-project 2's exit gate (design section 12 items 5 to 7) needs a human on macOS, Windows and Ubuntu. Modelled on `SKELETON_H2_CHECKLIST.md`. Use the installers from a bundle run on the final `rust-desktop/parity` commit, with the transport self-test green on all three OSes. Deviations for sign-off: `docs/plans/2026-10-13-rust-desktop-parity-exit-deviations.md`. Perf figures: `docs/plans/2026-10-13-rust-desktop-parity-perf.md`.

## Before installing

- [x] `frontend/index.html` has no `impeccable-live` block. Today it loads `http://localhost:8400/live.js` with a token (SP1 commit 664538f), outside SP2's scope. Remove it and rotate the token before building the installers (deviation 55). Done: removed in `76ad794`. Rotating the token is the CTO's own action.
- [x] The bundle run's self-test log has no "Error reading the log directory" line (SP1 #24, deviation 35). Checked on 2026-11-13: `gh run view 42324537207 --log` piped to `grep -i` gives no output. The log holds all three bundle jobs, each with its `QUARK_SELF_TEST` step.
- [x] The bundle run is green on all three OSes. Record its run id under Results. Run 42324537207 @ `1a0f162`: the macOS, Ubuntu and Windows bundle jobs, and every other job, ended `success` (checked 2026-11-13).
- [x] **H0:** confirm or adjust the section 9 caps, using the perf doc's summary table. On the 4-core Linux test container, three route cases miss the provisional caps idle (stats, CSV export, workbook confirm), XLSX export passes idle by 0.1 s after L12 but misses under load, and 5 of 9 reshape steps miss. Compare against the Mac figures if you measure there. Decision (CTO, 2026-11-13): the caps stay; a case within 5% of its cap passes; the misses and macOS and Windows measuring go to SPL-24 (perf doc, "H0 decision").

## SP1 carry-over (design section 12 item 6)

SP1 steps, from skeleton spec section 8.5: 1 install and launch from the icon; 2 create a project, upload the 2M-row CSV, open its View; 3 scroll to the end, sort, filter, dedupe; 4 record a Version and stop; 5 quit and relaunch (project, source, View and Versions return; deep pages are fast). Step 6 (the 501 message) is gone: nothing answers 501 now.

- [ ] Windows: steps 1 to 5, a full run (not run in SP1). Not ticked: A4 FAILs (the page-number box is clipped, SPL-18; the data was correct). A1, A2, A5 to A7 pass and A3 is an accepted PARTIAL.
- [x] macOS: step 2 in a new project (SP1 uploaded into Default). Project `SPL-12 macOS QA 2026-11-06`; the 2M-row View opened (A3). The upload progress text was not seen; see the re-check line.
- [ ] Ubuntu: step 1 from the launcher icon in a normal desktop session (SP1 launched from a terminal). Moved to SPL-26.
- [ ] Re-check SP1's H2 fixes on each OS: the footer page after jumping to page 10,000; opening a saved Version restores its sort, filter and dedupe; dedupe appears in the Version diff; the header timing follows the page; upload shows "Uploading <name>..." with a progress bar. Not ticked: the footer after page 10,000 FAILs on Windows (A4, SPL-18) and was not checked on macOS; macOS did not see the upload text or bar (A3 PARTIAL). Version restore and the dedupe diff pass on both (A6). Ubuntu moved to SPL-26.

## SP2 features (macOS, Windows and Ubuntu)

Use a 2M-row CSV with a numeric, a text and a date column, plus a small second CSV and a two-sheet `.xlsx`.

- [x] Column stats for a numeric, a text and a date column, with a filter on. Windows B1 PASS; macOS B1 PASS (id profile on the deduped View only).
- [ ] Category values: first page, search, scroll for more. Not ticked: macOS B2 is PARTIAL (not all 400 loaded values checked for duplicates), not in the CTO's accepted list. Windows B2 PASS.
- [x] Find: Next and Previous across columns, then after an edit to the filter. Windows B3 PASS; macOS B3 and R6 PASS.
- [ ] Charts on the Chart canvas and on the Dashboard canvas: bar, pie, histogram, box, scatter, line and metric. Group a bar, histogram or line by a column. The scatter keeps its points after a refetch. Not ticked: macOS "Deduped bar chart" FAILs (30 bars and 10 in Other, not 40 bars). Windows B4 and B5 PASS; macOS B4 and B5 PASS.
- [ ] A same-project join (two Views of one project) and a cross-source join (two sources); keep two join Views open and use both (the browser pass never covered cross-source joins). Not ticked: macOS ran one join (B6 PASS) and did not keep two join Views open. Windows B6 and B7 PASS.
- [ ] **Exports saved to disk.** For each of CSV, XLSX, Parquet and JSON: export a View, confirm the file lands on disk with the offered name, and open it (the first row matches the View). This is the only check that the Tauri webview saves blob downloads (design section 13 risk; the browser pass covered CSV and XLSX in Playwright WebKit only). If a webview cannot save, add the minimal Tauri download handler from design section 2. macOS B8 and R4 PASS. Windows B8 is an accepted PARTIAL: Parquet contents not read (no Python in the VM), XLSX not opened in Excel (not installed). PR #9 added the download handler (deviation 40).
- [ ] Export a View with 100,000 or more rows while scrolling another View; scrolling stays smooth. Not ticked: macOS B9 is PARTIAL (smoothness not judged from snapshots), not in the CTO's accepted list. Windows B9 is an accepted PARTIAL.
- [ ] Workbook: upload a two-sheet `.xlsx`, pick one sheet, confirm; the Views open. Upload one more and discard it. Not ticked: not run on macOS. Windows B10 PASS.
- [ ] Bad workbook: rename a text file to `.xlsx` and upload it. The app shows "Could not open source: ..." (400), and no file is left in the uploads folder. Not ticked: not run on macOS. Windows B11 PASS.
- [ ] Unknown routes: the app shows no 501 text anywhere ("Not in the desktop build yet"). The 404 and 405 answers need no hands-on step: `unknown_api_path_is_404_not_found` and `wrong_method_is_405` pin them, and a person cannot call the API from a terminal because the token is never logged (the app log shows only `listening on 127.0.0.1:<port>`) and a request without it is refused. Not ticked: not run on macOS. Windows B12 PASS.
- [ ] Quit and relaunch: projects, sources, Views, Versions and workbook sources return. Not ticked: macOS relaunch passed (B5, R9) but had no workbook source to check. Windows B13 PASS.

## Results

| OS | Installer run id | Date | SP1 carry-over | SP2 features | Exports saved | Notes |
|---|---|---|---|---|---|---|
| macOS | 42275220553 @ `85173de`; re-check 42324537207 @ `1a0f162` | 2026-11-06; re-check 2026-11-07 | Step 2 and A6 pass; A3 PARTIAL (no upload text or bar seen) | B1, B3, B4, B5, B6 PASS; deduped bar chart FAIL; B2, B9 PARTIAL. Re-check R1 to R9: 9/9 PASS | Yes: CSV, XLSX (Numbers), Parquet (DuckDB) and JSON, B8 and R4 | macOS 27.0.1, Mac16,10. The first CSV waited on the Downloads privacy prompt, which the CTO answered. CTO sign-off 2026-11-07 (SPL-12) |
| Windows | 42275220553 @ `85173de` | 2026-11-06 | A1, A2, A5 to A7 PASS; A3 PARTIAL; A4 FAIL (page box clipped, SPL-18; data correct) | B1 to B7, B10 to B13 PASS; B8, B9 PARTIAL | Yes, all four with the offered names; Parquet contents not read, XLSX not opened in Excel | 16 PASS, 1 FAIL, 3 PARTIAL. Windows 11 Arm64 VM; the x64 build ran under emulation. Re-check on `1a0f162` skipped by the CTO: Defender quarantined the unsigned installer (`Trojan:Win32/Bearfoos.A!ml`, an ML guess); signing is SPL-23. CTO sign-off 2026-11-07 (SPL-14), accepted as is |
| Ubuntu | not run | | | | | Removed from the gate by the CTO on 2026-11-13; moved to SPL-26 |

## Exit gate (plan section 15)

- [x] H0 caps confirmed or adjusted. CTO, 2026-11-13: the caps stay; within 5% of a cap passes; the misses go to SPL-24.
- [x] H2 passes on macOS and Windows (CTO sign-off 2026-11-07); Ubuntu waived by the CTO on 2026-11-13 and moved to SPL-26.
- [x] Each deviation in the exit deviations doc is marked accept or fix. Signed off 2026-11-13.
- [x] The Lead cleans up and pushes `rust-desktop/parity` (no merge, no pull request). Superseded: on 2026-11-07 the CTO had PR splitwireml/quark#9 opened and merged into `rust-desktop/integration` as `08162d3`.
