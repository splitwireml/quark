# Arrow table snapshots

Implemented on `codex/arrow-table-snapshots`, after checkpointing the scroll handoff in `e0f2c3a`. Both paged query routes negotiate Arrow IPC through the Accept header; other clients retain JSON. The frontend keeps Arrow vectors in the existing bounded caches and reads cells for rendering, copy, and editing. Preview concatenation retains the original buffers.

Normal integers, floats, booleans, strings, and binary values use native Arrow. Dates, decimals, 128-bit integers, and nested values retain the previous JSON cell semantics inside individual string columns. Mixed pages currently materialize Python tuples on the backend for those conversions. No source data or project registrations were changed.

## AllSpecs measurement — September 17, 2026

Source: `AllSpecs-Ids-ARE.xlsx`, `Sheet1`, Default project, 21,376 rows and 22 columns. The 50-column case is a read-only projection repeating source columns under unique names, not a separate dataset. Each payload holds 100 rows.

| Columns | Format | Payload bytes | Gzip bytes (comparison only) | Fetch median | Decode + 30-row access median | Scroll-to-paint median |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| 22 | JSON | 57,587 | 2,673 | 612.1 ms | 0.1 ms | 1,363.7 ms |
| 22 | Arrow | 27,432 | 5,512 | 609.1 ms | 0.2 ms | 1,348.3 ms |
| 50 | JSON | 140,539 | 6,382 | 637.4 ms | 0.3 ms | 1,397.3 ms |
| 50 | Arrow | 63,560 | 7,136 | 618.5 ms | 0.7 ms | 1,397.4 ms |

Arrow reduced the uncompressed payload by 52–55%. It did not demonstrate a meaningful scrolling improvement on this source, and decoding plus cell access was slightly slower. JSON compressed better on this repetitive dataset; the backend currently sends these responses without gzip. No heap or garbage-collection benchmark was performed.

Measurements used local Chromium and the live backend through Vite. Fetch timings alternate formats, discard two warmups, and take ten samples per format. Decode timings use 90 measured iterations per format. Scroll timings use six distant uncached jumps and two animation frames after the destination row appears. Browser automation and backend scheduling are included; this is not a production network benchmark. Cell parity, bounded DOM, held preview, and release loading all passed against the real source.

The existing endpoint still recomputes counts and null fractions for every page and rereads the XLSX-backed relation. Fetch time dwarfs decode time. Removing repeated query work is the next performance investigation; it was not bundled into this transport change.

## Validation and reproduction

- 84 backend tests, including both Arrow query routes, empty pages, sorting/filtering, metadata parity, exact large integers, date/decimal/nested compatibility, and JSON errors.
- 71 frontend unit tests, including lazy cell access and concatenated preview boundaries.
- Six browser scroll scenario groups now exercise Arrow, including a 50-column grid, copy/edit cell access, stale responses, failed-request recovery, bounded caches/DOM, and JSON compatibility.
- Frontend type check: zero errors or warnings. Production build passed; the existing large-bundle advisory remains (current JS: 871 KB, 260 KB gzip).
- Svelte autofixer: no issues in either changed component; existing effect/action suggestions remain.

Run `npm run benchmark:arrow` from `frontend` with the backend running and AllSpecs registered. Use `PLAYWRIGHT_MODULE` for an existing shared Playwright installation, as described in the README. The benchmark only changes API metadata responses inside its isolated browser context for the 50-column projection; it never saves that projection as a View.
