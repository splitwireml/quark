import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { execFileSync } from 'node:child_process';
import { isThemePreference, readThemePreference, resolveScheme, themeStorageKey } from '../src/lib/theme.ts';

test('an unknown or absent stored preference falls back to following the system', () => {
  assert.equal(readThemePreference(null), 'system');
  assert.equal(readThemePreference(''), 'system');
  assert.equal(readThemePreference('sepia'), 'system');
  assert.equal(readThemePreference('dark'), 'dark');
  assert.equal(readThemePreference('light'), 'light');
  assert.equal(readThemePreference('system'), 'system');
  assert.equal(isThemePreference('system'), true);
  assert.equal(isThemePreference(2), false);
  assert.equal(themeStorageKey, 'quark.color-scheme');
});

test('an explicit choice outranks the system, and system follows it', () => {
  assert.equal(resolveScheme('system', true), 'dark');
  assert.equal(resolveScheme('system', false), 'light');
  assert.equal(resolveScheme('light', true), 'light');
  assert.equal(resolveScheme('dark', false), 'dark');
});

/* The design contract is the same in both schemes: three text levels, each at or above
   4.5:1 on every surface, and glyphs at or above the 3:1 non-text minimum. Parsing the
   tokens out of the stylesheet is what keeps a later colour tweak from quietly failing it. */
const css = readFileSync(new URL('../src/app.css', import.meta.url), 'utf8');

function bodyOf(selector) {
  const block = css.slice(css.indexOf(selector) + selector.length);
  return block.slice(0, block.indexOf('}'));
}

function tokensOf(selector) {
  return Object.fromEntries([...bodyOf(selector).matchAll(/(--[\w-]+):\s*(#[0-9A-Fa-f]{6})/g)].map(([, name, value]) => [name, value]));
}

/* Every token whose value is a literal colour, hex or rgb() alike. A dark counterpart that
   is simply missing is the quiet failure: the light value stays in place and nobody notices
   until it is a white tile in a dark window. */
function colourTokensOf(selector) {
  return [...bodyOf(selector).matchAll(/(--[\w-]+):\s*(#[0-9A-Fa-f]{3,8}|rgba?\()/g)].map(([, name]) => name);
}

function channel(part) {
  const c = part / 255;
  return c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
}

function luminance(hex) {
  const [r, g, b] = [1, 3, 5].map((i) => parseInt(hex.slice(i, i + 2), 16));
  return 0.2126 * channel(r) + 0.7152 * channel(g) + 0.0722 * channel(b);
}

function contrast(a, b) {
  const [hi, lo] = [luminance(a), luminance(b)].sort((x, y) => y - x);
  return (hi + 0.05) / (lo + 0.05);
}

const surfaces = ['--canvas', '--surface', '--surface-2', '--surface-3', '--surface-inset', '--surface-hover'];

for (const [scheme, selector] of [['light', ':root {'], ['dark', ":root[data-theme='dark'] {"]]) {
  test(`${scheme} text clears 4.5:1 and glyphs 3:1 on every surface`, () => {
    const token = tokensOf(selector);
    for (const surface of surfaces) {
      assert.ok(token[surface], `${scheme} is missing ${surface}`);
      for (const text of ['--ink', '--ink-2', '--muted', '--faint']) {
        const ratio = contrast(token[text], token[surface]);
        assert.ok(ratio >= 4.5, `${scheme} ${text} on ${surface} is ${ratio.toFixed(2)}:1`);
      }
      const glyph = contrast(token['--glyph'], token[surface]);
      assert.ok(glyph >= 3, `${scheme} --glyph on ${surface} is ${glyph.toFixed(2)}:1`);
    }
    /* SQL sits on --surface, and syntax colour is text like any other. */
    for (const code of ['--code-keyword', '--code-string', '--code-number', '--code-name', '--code-comment']) {
      const ratio = contrast(token[code], token['--surface']);
      assert.ok(ratio >= 4.5, `${scheme} ${code} on --surface is ${ratio.toFixed(2)}:1`);
    }

    const onFill = contrast(token['--on-fill'], token['--ink-fill']);
    assert.ok(onFill >= 4.5, `${scheme} --on-fill on --ink-fill is ${onFill.toFixed(2)}:1`);
    const accent = contrast(token['--action-dark'], token['--action-tint']);
    assert.ok(accent >= 4.5, `${scheme} --action-dark on --action-tint is ${accent.toFixed(2)}:1`);
  });
}

test('both schemes define the same colour tokens', () => {
  /* --chart-* is written onto the root element at runtime from the palette, not from here. */
  const light = colourTokensOf(':root {').filter((name) => !name.startsWith('--chart-')).sort();
  const dark = colourTokensOf(":root[data-theme='dark'] {").sort();
  assert.deepEqual(dark, light);
});

/* The reason a dark scheme leaks: a component states a colour of its own, and it is right in
   exactly one scheme. `#fff` on a filled button was invisible to the first sweep here, which
   only looked for six-digit hex, and it shipped a white label on a white chip. */
test('no component hardcodes a light or dark colour', () => {
  const offenders = [];
  const files = execFileSync('git', ['ls-files', 'src'], { cwd: new URL('..', import.meta.url), encoding: 'utf8' })
    .split('\n')
    .filter((name) => name.endsWith('.svelte'));
  for (const file of files) {
    const source = readFileSync(new URL(`../${file}`, import.meta.url), 'utf8');
    source.split('\n').forEach((line, index) => {
      if (!/(color|background|fill|stroke|border[\w-]*|box-shadow|outline)\s*:/.test(line)) return;
      const literal = line.match(/#[0-9A-Fa-f]{3,8}\b|\brgba?\(\s*255[\s,]+255[\s,]+255|:\s*(white|black)\b/);
      if (literal) offenders.push(`${file}:${index + 1}: ${literal[0].trim()}`);
    });
  }
  assert.deepEqual(offenders, [], `hardcoded colours belong in app.css as tokens:\n${offenders.join('\n')}`);
});

/* The light palettes set their own visibility: some are deliberately soft, and Okabe-Ito is
   reproduced as published for hue separation rather than luminance. So what the dark variants
   owe is not an absolute floor invented here — it is that each one stands off its own surface
   at least as well as the light original does off white. That is exactly the way a dark
   palette fails: colours carried over unchanged sink into the dark canvas. */
import { chartPalettes } from '../src/lib/chartThemes.ts';

test('every dark palette stands off its surface as well as its light original does', () => {
  const light = tokensOf(':root {')['--surface'];
  const dark = tokensOf(":root[data-theme='dark'] {")['--surface'];
  for (const mode of Object.keys(chartPalettes)) {
    for (const palette of chartPalettes[mode]) {
      const named = [
        ['strong', palette.colors.strong, palette.dark.strong],
        ['mark', palette.colors.mark, palette.dark.mark],
        ...palette.colors.series.map((color, index) => [`series ${index + 1}`, color, palette.dark.series[index]]),
        ...palette.colors.seriesFill.map((color, index) => [`fill ${index + 1}`, color, palette.dark.seriesFill[index]])
      ];
      for (const [name, lightColor, darkColor] of named) {
        assert.ok(darkColor, `${palette.id} is missing a dark ${name}`);
        const was = contrast(lightColor, light);
        const now = contrast(darkColor, dark);
        /* Visibility saturates: a light palette's near-black mark reads at 10:1 on white, and
           matching that on a dark canvas would take a near-white mark and flatten the hue.
           Above 5:1 the requirement is simply to stay comfortably legible. */
        const floor = Math.min(was, 5) * 0.9;
        assert.ok(now >= floor, `${palette.id} dark ${name} (${darkColor}) reads at ${now.toFixed(2)}:1 where light read ${was.toFixed(2)}:1`);
      }
    }
  }
});

test('both schemes keep six series, six fills, and readable tinted text', () => {
  for (const [scheme, key] of [['light', 'colors'], ['dark', 'dark']]) {
    for (const mode of Object.keys(chartPalettes)) {
      for (const palette of chartPalettes[mode]) {
        const colors = palette[key];
        assert.equal(colors.series.length, 6, `${scheme} ${palette.id} needs six series colours`);
        assert.equal(colors.seriesFill.length, 6, `${scheme} ${palette.id} needs six fills`);
        const ink = contrast(colors.ink, colors.fill);
        assert.ok(ink >= 4.5, `${scheme} ${palette.id} ink on fill is ${ink.toFixed(2)}:1`);
        if (mode === 'multicolor') {
          assert.equal(new Set(colors.series).size, 6, `${scheme} ${palette.id} repeats a series colour`);
          continue;
        }
        if (mode === 'single') {
          assert.equal(new Set(colors.series).size, 1, `${scheme} ${palette.id} is a single-hue palette`);
          continue;
        }
        /* A shade scale is read by its ramp, so consecutive steps must stay apart. */
        colors.series.slice(1).forEach((color, index) => {
          const step = contrast(color, colors.series[index]);
          assert.ok(step >= 1.2, `${scheme} ${palette.id} steps ${index + 1}\u2192${index + 2} by only ${step.toFixed(2)}:1`);
        });
      }
    }
  }
});
