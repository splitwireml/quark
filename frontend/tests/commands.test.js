import assert from 'node:assert/strict';
import { test } from 'node:test';
import { commandFor, operations, operationsFor } from '../src/lib/commands.ts';
const key = (value, options = {}) => ({ key: value, metaKey: false, ctrlKey: false, altKey: false, shiftKey: false, ...options });
test('commands own sequences without stealing text editing or modified keys', () => {
  assert.equal(commandFor(key('a', { metaKey: true }), null, false), 'wheel');
  assert.equal(commandFor(key('a', { ctrlKey: true }), null, false), 'wheel');
  assert.equal(commandFor(key('a', { metaKey: true }), null, true), null);
  assert.equal(commandFor(key('f', { metaKey: true }), null, false), 'prefix-find');
  assert.equal(commandFor(key('c'), 'find', false), 'find-column');
  assert.equal(commandFor(key('f'), 'find', false), 'find-values');
  assert.equal(commandFor(key('f'), 'column', false), 'filter');
  assert.equal(commandFor(key('s'), 'column', false), 'sort');
  assert.equal(commandFor(key('h'), 'column', false), 'hide');
  assert.equal(commandFor(key('p'), 'column', false), 'pin');
  assert.equal(commandFor(key('t'), 'sidebar', false), 'sources');
  assert.equal(commandFor(key('b', { metaKey: true }), null, false), 'prefix-sidebar');
  assert.equal(commandFor(key('b'), 'sidebar', false), 'sidebar');
  assert.equal(commandFor(key('b', { metaKey: true }), 'sidebar', false), 'sidebar');
  assert.equal(commandFor(key('t', { metaKey: true }), 'sidebar', false), 'sources');
  assert.equal(commandFor(key('c', { ctrlKey: true }), 'find', false), 'find-column');
  assert.equal(commandFor(key('Meta', { metaKey: true }), 'find', false), null);
  assert.equal(commandFor(key('x'), 'column', false), 'cancel');
  assert.equal(commandFor(key('Escape'), 'find', false), 'cancel');
  assert.equal(commandFor(key('Meta'), 'find', false), null);
  assert.equal(commandFor(key('z', { metaKey: true, shiftKey: true }), null, false), 'redo');
  assert.equal(commandFor(key('v', { metaKey: true, shiftKey: true }), null, false), 'versions');
  assert.equal(commandFor(key('a', { metaKey: true, shiftKey: true }), null, false), null);
  assert.equal(commandFor(key('f', { isComposing: true }), 'find', false), null);
  assert.equal(new Set(operations.map(item => item.key)).size, operations.length);
});
test('simple actions retain four operations and two sizing shortcuts', () => {
  assert.deepEqual(operationsFor('simple').map(({ id, key }) => [id, key]), [
    ['aggregate', 'a'], ['joins', 'j'], ['columns', 'c'], ['dedupe', 'd'], ['density', 'r'], ['fit', 'w'],
  ]);
  assert.equal(operationsFor('comprehensive'), operations);
  assert.deepEqual(operationsFor('simple').filter(item => item.direction).map(({ id, direction }) => [direction, id]), [
    ['↑', 'aggregate'], ['←', 'joins'], ['↓', 'columns'], ['→', 'dedupe'],
  ]);
});
