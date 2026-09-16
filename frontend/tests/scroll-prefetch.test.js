import assert from 'node:assert/strict';
import test from 'node:test';
import {
  MAX_PREFETCH_PAGES,
  criticalPages,
  latencyEma,
  pagesForRange,
  prefetchHorizonRows,
  scrollRowStep,
  smoothVelocity,
  snapshotWindow
} from '../src/lib/scroll-prefetch.ts';

test('smoothVelocity blends toward the instant reading', () => {
  assert.equal(smoothVelocity(0, 100), 35);
  assert.equal(smoothVelocity(35, 100), 35 + 0.35 * 65);
  assert.equal(smoothVelocity(10, Number.NaN), 10);
});

test('prefetchHorizonRows floors at ten percent of a page and grows with speed', () => {
  assert.equal(prefetchHorizonRows(0, 300, 100), 10);
  assert.equal(prefetchHorizonRows(5, 300, 100), 10);
  assert.equal(prefetchHorizonRows(1000, 300, 100), 300);
  // Latency clamps bound the lookahead both ways.
  assert.equal(prefetchHorizonRows(10_000, 10, 100), Math.ceil(10_000 * 0.15));
  assert.equal(prefetchHorizonRows(10_000, 60_000, 100), Math.ceil(10_000 * 1.2));
});

test('pagesForRange covers the range and clamps to valid pages', () => {
  assert.deepEqual(pagesForRange(0, 99, 100, 10), [1]);
  assert.deepEqual(pagesForRange(50, 250, 100, 10), [1, 2, 3]);
  assert.deepEqual(pagesForRange(-500, 50, 100, 10), [1]);
  assert.deepEqual(pagesForRange(950, 5000, 100, 10), [10]);
  assert.deepEqual(pagesForRange(0, 99, 0, 5), [1, 2, 3, 4, 5]);
});

test('criticalPages does not warm a neighbor before the critical point', () => {
  assert.deepEqual(
    criticalPages({ firstRow: 0, lastRow: 30, velocity: 0, pageSize: 100, totalPages: 10, currentPage: 1, coveredPages: [1], latencyMs: 300, lastDir: 1 }),
    []
  );
  // Already warm: nothing to do.
  assert.deepEqual(
    criticalPages({ firstRow: 0, lastRow: 30, velocity: 0, pageSize: 100, totalPages: 10, currentPage: 1, coveredPages: [1, 2], latencyMs: 300, lastDir: 1 }),
    []
  );
  // Idle at the last page has no forward neighbor.
  assert.deepEqual(
    criticalPages({ firstRow: 900, lastRow: 930, velocity: 0, pageSize: 100, totalPages: 10, currentPage: 10, coveredPages: [10], latencyMs: 300, lastDir: 1 }),
    []
  );
});

test('criticalPages loads past the critical point when scanning slowly', () => {
  // Slow downward scan near the bottom of page 1: only page 2 is past the edge.
  assert.deepEqual(
    criticalPages({ firstRow: 60, lastRow: 95, velocity: 8, pageSize: 100, totalPages: 10, currentPage: 1, coveredPages: [1], latencyMs: 300, lastDir: 1 }),
    [2]
  );
  // Slow upward scan mirrors toward the previous page.
  assert.deepEqual(
    criticalPages({ firstRow: 105, lastRow: 140, velocity: -8, pageSize: 100, totalPages: 10, currentPage: 2, coveredPages: [2], latencyMs: 300, lastDir: -1 }),
    [1]
  );
});

test('criticalPages looks further ahead for fast swipes and caps the burst', () => {
  const pages = criticalPages({ firstRow: 0, lastRow: 30, velocity: 4000, pageSize: 100, totalPages: 50, currentPage: 1, coveredPages: [1], latencyMs: 300, lastDir: 1 });
  assert.ok(pages.length >= 2);
  assert.ok(pages.length <= MAX_PREFETCH_PAGES);
  assert.equal(pages[0], 2);
  assert.deepEqual(
    criticalPages({ firstRow: 0, lastRow: 30, velocity: 4000, pageSize: 100, totalPages: 50, currentPage: 1, coveredPages: [1, 2, 3], latencyMs: 300, lastDir: 1 }),
    []
  );
});

test('snapshotWindow sizes to the viewport and aligns to its own grid', () => {
  const window = snapshotWindow(450, 30);
  assert.ok(window.size >= 20 && window.size <= 500);
  assert.equal(window.size, 38);
  assert.equal(window.start, Math.floor(450 / 38) * 38);
  assert.equal(window.page, Math.floor(450 / 38) + 1);
  assert.equal(window.start, (window.page - 1) * window.size);
});

test('latencyEma tracks samples within bounds', () => {
  assert.equal(latencyEma(300, 500), 350);
  assert.ok(latencyEma(100, 1_000_000) <= 2000);
  assert.ok(latencyEma(1900, 0) >= 80);
});

test("slow scrolling before the critical point does not load another page", () => {
  for (const velocity of [0, 8]) {
    assert.deepEqual(criticalPages({ firstRow: 0, lastRow: 30, velocity, pageSize: 100, totalPages: 10, currentPage: 1, coveredPages: [1], latencyMs: 300, lastDir: 1 }), []);
  }
});


test('compressed canvas preserves natural viewport row count and reaches the last row', () => {
  const total = 10_000_000, height = 34, viewport = 680;
  const step = scrollRowStep(total, height, viewport);
  assert.equal(scrollRowStep(1000, height, viewport), height);
  assert.ok(Math.abs((10_000_000 - viewport) / step + viewport / height - total) < 0.001);
});

test('missing visible pages take priority over lookahead', () => {
  assert.deepEqual(criticalPages({ firstRow: 499, lastRow: 525, velocity: 4000, pageSize: 100, totalPages: 100, currentPage: 1, coveredPages: [1], latencyMs: 300, lastDir: 1 }), [5, 6]);
});
