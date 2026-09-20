import assert from 'node:assert/strict';
import { test } from 'node:test';
import {
  chartThemeCssVariables,
  defaultChartThemePreferences,
  readChartThemePreferences,
  serializeChartThemePreferences
} from '../src/lib/chartThemes.ts';

test('legacy chart theme values migrate to the new modes', () => {
  assert.deepEqual(readChartThemePreferences('primary'), {
    mode: 'single',
    palettes: { single: 'blue', monotone: 'graphite', multicolor: 'classic' }
  });
  assert.equal(readChartThemePreferences('monotone').mode, 'monotone');
  assert.equal(readChartThemePreferences('rich').mode, 'multicolor');
});

test('stored palette selections preserve one choice per mode', () => {
  const preferences = {
    mode: 'monotone',
    palettes: { single: 'violet', monotone: 'forest', multicolor: 'sunset' }
  };
  const restored = readChartThemePreferences(serializeChartThemePreferences(preferences));
  assert.deepEqual(restored, preferences);
  const css = chartThemeCssVariables(restored);
  assert.equal(css['--chart-mark-strong'], '#1E6B45');
  assert.equal(css['--chart-series-3-fill'], '#D4EBDD');
});

test('invalid stored values fall back safely without inherited-key crashes', () => {
  assert.deepEqual(readChartThemePreferences('__proto__'), defaultChartThemePreferences);
  const invalid = readChartThemePreferences(JSON.stringify({
    mode: 'constructor',
    palettes: { single: 'toString', monotone: 'ocean', multicolor: 'classic' }
  }));
  assert.equal(invalid.mode, 'single');
  assert.equal(invalid.palettes.single, 'blue');
  assert.equal(invalid.palettes.monotone, 'ocean');
  assert.equal(invalid.palettes.multicolor, 'classic');
});
