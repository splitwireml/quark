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

test('a single palette borrows the closest monotone ramp for group series', () => {
  const css = chartThemeCssVariables({ mode: 'single', palettes: { single: 'blue', monotone: 'graphite', multicolor: 'classic' } });
  // The accent itself stays the single palette's blue.
  assert.equal(css['--chart-mark-strong'], '#1155F5');
  // Ungrouped charts keep the flat accent across every series slot.
  assert.equal(css['--chart-series-1'], css['--chart-series-6']);
  // Group series borrow Ocean, so consecutive slots differ.
  assert.notEqual(css['--chart-group-1'], css['--chart-group-6']);
  assert.equal(css['--chart-group-1'], '#174EA6');
});

test('teal and amber borrow warm and green ramps, violet borrows ocean', () => {
  const group = (single) => chartThemeCssVariables({ mode: 'single', palettes: { single, monotone: 'graphite', multicolor: 'classic' } })['--chart-group-1'];
  assert.equal(group('teal'), '#1E6B45');
  assert.equal(group('amber'), '#A64716');
  assert.equal(group('violet'), '#174EA6');
});

test('monotone and multicolor drive group series from their own palette', () => {
  const mono = chartThemeCssVariables({ mode: 'monotone', palettes: { single: 'blue', monotone: 'forest', multicolor: 'classic' } });
  assert.equal(mono['--chart-group-1'], mono['--chart-series-1']);
  assert.notEqual(mono['--chart-group-1'], mono['--chart-group-4']);
  const multi = chartThemeCssVariables({ mode: 'multicolor', palettes: { single: 'blue', monotone: 'graphite', multicolor: 'classic' } });
  assert.equal(multi['--chart-group-2'], '#0F9D8A');
});
