import assert from 'node:assert/strict';
import test from 'node:test';
import {
  MIN_THUMB_PX,
  absoluteRowToPage,
  clampAbsoluteRow,
  safeTotalRows,
  thumbGeometry
} from '../src/lib/row-scrollbar.ts';

test('safeTotalRows passes numbers through and caps unsafe strings', () => {
  assert.equal(safeTotalRows(0), 0);
  assert.equal(safeTotalRows(1234), 1234);
  assert.equal(safeTotalRows('1234'), 1234);
  assert.equal(safeTotalRows('-5'), 0);
  assert.equal(safeTotalRows('not-a-count'), 0);
  assert.equal(safeTotalRows('9'.repeat(30)), Number.MAX_SAFE_INTEGER);
  assert.equal(safeTotalRows(Number.MAX_SAFE_INTEGER), Number.MAX_SAFE_INTEGER);
});

test('thumbGeometry is not scrollable when everything fits', () => {
  assert.equal(thumbGeometry(400, 100, 0, 100).scrollable, false);
  assert.equal(thumbGeometry(400, 80, 0, 100).scrollable, false);
  assert.equal(thumbGeometry(0, 1000, 0, 20).scrollable, false);
  assert.equal(thumbGeometry(400, 0, 0, 20).scrollable, false);
});

test('thumbGeometry scales the thumb to the visible fraction', () => {
  const geometry = thumbGeometry(400, 1000, 0, 100);
  assert.equal(geometry.scrollable, true);
  assert.equal(geometry.thumbHeight, 40);
  assert.equal(geometry.thumbTop, 0);
});

test('thumbGeometry positions the thumb proportionally', () => {
  const total = 1000;
  const visible = 100;
  const track = 400;
  const travel = track - (track * visible) / total;
  const middle = thumbGeometry(track, total, 450, visible);
  assert.ok(Math.abs(middle.thumbTop - travel / 2) < 1e-9);
  const end = thumbGeometry(track, total, 900, visible);
  assert.ok(Math.abs(end.thumbTop - travel) < 1e-9);
});

test('thumbGeometry clamps out-of-range positions and enforces a minimum thumb', () => {
  const track = 400;
  const huge = thumbGeometry(track, 10_000_000, 0, 20);
  assert.equal(huge.thumbHeight, MIN_THUMB_PX);
  const before = thumbGeometry(track, 1000, -50, 100);
  assert.equal(before.thumbTop, 0);
  const after = thumbGeometry(track, 1000, 5000, 100);
  const height = (track * 100) / 1000;
  assert.ok(Math.abs(after.thumbTop - (track - height)) < 1e-9);
});

test('clampAbsoluteRow keeps seeks inside the dataset', () => {
  assert.equal(clampAbsoluteRow(-3, 100), 0);
  assert.equal(clampAbsoluteRow(4.9, 100), 4);
  assert.equal(clampAbsoluteRow(99, 100), 99);
  assert.equal(clampAbsoluteRow(100, 100), 99);
  assert.equal(clampAbsoluteRow(10, 0), 0);
});

test('absoluteRowToPage maps absolute rows onto pages', () => {
  assert.deepEqual(absoluteRowToPage(0, 100, 10), { page: 1, intraRow: 0 });
  assert.deepEqual(absoluteRowToPage(99, 100, 10), { page: 1, intraRow: 99 });
  assert.deepEqual(absoluteRowToPage(100, 100, 10), { page: 2, intraRow: 0 });
  assert.deepEqual(absoluteRowToPage(999, 100, 10), { page: 10, intraRow: 99 });
  assert.deepEqual(absoluteRowToPage(5000, 100, 10), { page: 10, intraRow: 99 });
  assert.deepEqual(absoluteRowToPage(0, 0, 5), { page: 1, intraRow: 0 });
});
