# Quark desktop skeleton: H2 checklist

Date: 2026-10-04. The first H2 pass used the CI installers from run 37194514409 at `de640e9`. New installers with every fix below come from bundle run 37304898957 at `3abceb1` (green on all three OSes, self-test included); they expire on 2026-10-19. Use them for the remaining steps. Full notes are in `/Users/mali/Development/quark-wt/runlog.md`.

Fixed on 2026-10-04 and not yet committed: the footer, Version diff, header timing and upload items below, plus the handoff text. Each fix was checked in Playwright WebKit against `quark-dev-server`. The footer and timing bugs were reproduced in WebKit before the fix, and the dedupe bug in Chromium. `npm test` (147 tests), `npm run check`, `npm run build` and `tests/scroll-browser.mjs` pass. Not yet re-checked in the installed desktop app.

## H2 smoke test (spec §8.5)

- [x] macOS: steps 1 to 6 pass with the CI `.dmg`.
- [ ] macOS: step 2 gap. Create a new project and upload into it; this run uploaded into Default.
- [x] Ubuntu: steps 2 to 6 pass with the CI `.deb`.
- [ ] Ubuntu: step 1 gap. Launch from a real launcher icon in a normal desktop session; the tester launched from a terminal.
- [ ] Windows: steps 1 to 6, not run yet.
- [x] First page of a brand-new CSV in about 0.5 s (§8.6): measured 484 ms on this Mac on 2026-10-05 (release `perf_acceptance`, fresh temp folder, so no columnar cache). All other §8.6 targets pass too.

## UI issues found

The skeleton's frontend changes touch only `api.ts`, `api-transport.ts`, `vite-env.d.ts` and one line of `AggregateMenuPopover.svelte`. All of these issues already exist on `main` and do not block H2.

- [x] **The footer is one page behind after a jump** (macOS and Ubuntu). Entering page 10,000 loads the right rows, but the footer reports page 9,999. The cause is `firstVisibleRow = scrollTop / pxPerRow` in `frontend/src/components/organisms/DataGridTable.svelte`. With 2M rows the scroll height is compressed, so each row is a fraction of a pixel. WebKit rounds `scrollTop` down, which puts the first row just before the target, and `absoluteRowToPage` then rounds down to the previous page. Fix: `scrollToAbsoluteRow` records the exact target row with the `scrollTop` it produced, and `firstVisibleRow` uses that row until `scrollTop` changes. In WebKit, jumps to pages 17, 1,000, 1,234 and 2,999 of 3,000 previously reported one page less and now report the page entered.
- [x] **Opening a saved Version resets the sort and dedupe controls** (macOS, including after a relaunch). The dedupe panel shows 0 keys, but the data is correct. This is by design: `replayVersionSnapshot` in `App.svelte` clears filters, sorts and dedupe and then replays the Version's saved SQL. Decided 2026-10-05: restore them. Stop recording now also saves the base SQL with the live filters, sorts and dedupe keys, and opening the Version replays them as controls. Older Versions keep the old behavior. While checking this, the Versions and Export menus turned out to open under the condition bar (z-index 12 under 16), so older Versions could not be picked while a filter was active; both menus now sit at 20. Checked in WebKit: v1 shows no controls; v2 brings back its sort, filter and dedupe; clearing dedupe on the restored v2 goes from 4 rows to 60,237; a reload keeps v2's controls.
- [x] **Dedupe is missing from the Version diff** (Ubuntu). The v1 to v2 diff lists the sort and the filter added and removed, but not the dedupe. Cause: `setDedupe` passes the Svelte `$state` array `dedupeDraft` as `details.columns`, and `copyChange` in `versioning.ts` called `structuredClone`, which throws `DataCloneError` on proxies. The change was never staged. The error was uncaught, so the dedupe still applied. Fix: `copyChange` copies details with a JSON round trip; details are JSON by type. Regression test added in `versioning.test.js`.
- [x] **The header timing does not update when the page changes.** It stays on the first query's value when pages come from the prefetch cache. Fix: cached pages keep their `elapsed_ms`, and `promotePage` shows it.
- [x] **Upload shows no progress.** A 254 MB file took about 10 seconds with no indicator. Fix: the control that took the file shows "Uploading <name>…" and an indeterminate bar (new atom `BusyBar.svelte`, with a static bar under reduced motion). Byte progress is not shown, because most of the time is probably server-side import, not transfer.
- [ ] **The 501 message does not show a status code.** Only the text appears, which is probably fine. Deferred: the 501 goes away as sub-project 2 ports the routes.

## Docs and environment

- [x] Handoff: replace "right-click, then Open" for macOS. Since macOS 15 that no longer works. Use Open Anyway in System Settings (Privacy & Security), or run `xattr -dr com.apple.quarantine /Applications/Quark.app`.
- [ ] Ubuntu `.deb`: installing it pulls in `fuse3`, which can stop a non-interactive install on a prompt about the config file. Watch item only.

## Exit gate (plan §12)

- [ ] H2 passes on all three OSes.
- [x] Each of the 25 deviations in `docs/plans/2026-10-04-skeleton-exit-deviations.md` is marked accept or fix (signed off 2026-10-05; see the Sign-off section there).
- [x] For each UI issue above, decide whether to fix it now or defer it to sub-project 2 (all fixed except the 501 status code, which ports away in sub-project 2; see `docs/plans/2026-10-05-sp2-backend-parity-notes.md`).
