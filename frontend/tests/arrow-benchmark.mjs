// Read-only benchmark against a running backend. Defaults to AllSpecs in any project.
// Run with npm run benchmark:arrow (same PLAYWRIGHT_MODULE option as the scroll suite).
import assert from 'node:assert/strict';
import { gzipSync } from 'node:zlib';
import { createServer } from 'vite';
const { chromium } = await import(process.env.PLAYWRIGHT_MODULE || 'playwright');
const backend = process.env.QUARK_API_URL || 'http://127.0.0.1:8000';
const sourceName = process.env.QUARK_BENCH_SOURCE || 'AllSpecs';
const arrow = 'application/vnd.apache.arrow.stream';
const median = values => [...values].sort((a, b) => a - b)[Math.floor(values.length / 2)];
const round = value => Math.round(value * 1000) / 1000;
const get = async path => {
  const response = await fetch(backend + path);
  assert.ok(response.ok, `GET ${path}: ${response.status}`);
  return response.json();
};
async function visibleLoaded(grid) {
  await grid.waitForFunction(() => {
    const host = document.querySelector('.table-scroll').getBoundingClientRect();
    const visible = [...document.querySelectorAll('tbody tr:not(.spacer)')].filter(row => {
      const rect = row.getBoundingClientRect();
      return rect.bottom > host.top + 48 && rect.top < host.bottom;
    });
    return visible.length > 0 && visible.every(row => !row.classList.contains('pending'));
  });
}
let project, view;
for (const candidate of await get('/api/projects')) {
  const views = await get(`/api/projects/${candidate.id}/views`);
  view = views.find(item => item.source_name.toLowerCase().includes(sourceName.toLowerCase()));
  if (view) { project = candidate; break; }
}
assert.ok(view, `No source matching ${sourceName}`);
const source = await get(`/api/projects/${project.id}/sources/${view.source_id}`);
const server = await createServer({ root: new URL('..', import.meta.url).pathname, server: { host: '127.0.0.1', port: 0, proxy: { '/api': backend } } });
await server.listen();
const browser = await chromium.launch({ headless: true });
const url = server.resolvedUrls.local[0];
const errors = [];
try {
  const page = await browser.newPage();
  page.on('pageerror', error => errors.push(error.message));
  await page.goto(url);
  for (const width of [...new Set([view.columns.length, 50])]) {
    const names = Array.from({ length: width }, (_, i) => i < view.columns.length ? view.columns[i] : `${view.columns[i % view.columns.length]}_${i}`);
    const quote = name => `"${name.replaceAll('"', '""')}"`;
    const sql = width === view.columns.length ? view.sql : `SELECT ${names.map((name, i) => `${quote(view.columns[i % view.columns.length])} AS ${quote(name)}`).join(', ')} FROM (${view.sql}) AS sample`;
    const measured = await page.evaluate(async ({ node, sql, names, arrow }) => {
      const { decodeQueryResponse } = await import('/src/lib/table-rows.ts');
      const samples = { json: [], arrow: [] }, payloads = {};
      const headers = format => ({ 'Content-Type': 'application/json', Accept: format === 'arrow' ? arrow : 'application/json' });
      for (let iteration = 0; iteration < 12; iteration++) {
        for (const format of iteration % 2 ? ['arrow', 'json'] : ['json', 'arrow']) {
          const start = performance.now();
          const response = await fetch(`/api/nodes/${node}/sql`, { method: 'POST', headers: headers(format), body: JSON.stringify({ sql, page: 4, page_size: 100 }) });
          if (!response.ok) throw new Error(await response.text());
          if (format === 'arrow' && !response.headers.get('content-type')?.includes(arrow)) throw new Error('Backend did not return Arrow; restart with the current code');
          const bytes = new Uint8Array(await response.arrayBuffer());
          if (iteration > 1) samples[format].push(performance.now() - start);
          payloads[format] = bytes;
        }
      }
      const decoded = {}, timings = { json: [], arrow: [] };
      for (let iteration = 0; iteration < 100; iteration++) {
        for (const format of iteration % 2 ? ['arrow', 'json'] : ['json', 'arrow']) {
          const start = performance.now();
          const result = await decodeQueryResponse(new Response(payloads[format], { headers: { 'Content-Type': format === 'arrow' ? arrow : 'application/json' } }));
          // Read a viewport, as rendering does, without materializing cached rows.
          for (let row = 0; row < Math.min(30, result.rows.length); row++) for (const name of names) result.rows.cell(row, name);
          if (iteration > 9) timings[format].push(performance.now() - start);
          decoded[format] = result;
        }
      }
      const cells = result => Array.from({ length: result.rows.length }, (_, row) => names.map(name => result.rows.cell(row, name)));
      if (JSON.stringify(cells(decoded.json)) !== JSON.stringify(cells(decoded.arrow))) throw new Error('Arrow/JSON cell mismatch');
      return { samples, timings, payloads: Object.fromEntries(Object.entries(payloads).map(([key, bytes]) => [key, Array.from(bytes)])), rows: decoded.arrow.rows.length, total: decoded.arrow.total_rows };
    }, { node: view.node_id, sql, names, arrow });

    for (const format of ['json', 'arrow']) {
      const grid = await browser.newPage({ viewport: { width: 1440, height: 900 } });
      grid.on('pageerror', error => errors.push(error.message));
      const projected = { ...view, sql, columns: names };
      await grid.route('**/api/**', async route => {
        const path = new URL(route.request().url()).pathname;
        if (path === '/api/projects') return route.fulfill({ json: [project] });
        if (path.endsWith('/views')) return route.fulfill({ json: [projected] });
        if (path.endsWith('/sources')) return route.fulfill({ json: [{ id: source.id, name: source.name }] });
        if (path.endsWith(`/sources/${source.id}`)) return route.fulfill({ json: { ...source, views: [projected] } });
        return route.continue({ headers: { ...route.request().headers(), accept: format === 'arrow' ? arrow : 'application/json' } });
      });
      await grid.goto(url);
      await grid.getByRole('button', { name: new RegExp(`^${project.name} `) }).click();
      await grid.getByRole('button', { name: source.name, exact: true }).first().click();
      await grid.locator('tbody td[data-column]').first().waitFor();
      const paint = [];
      for (const row of [1000, 3000, 5000, 7000, 9000, 11000]) {
        assert.ok(row < Number(measured.total), 'Benchmark needs more than 11,000 rows');
        paint.push(await grid.locator('.table-scroll').evaluate(async (element, row) => {
          const start = performance.now();
          element.scrollTop = row * 34;
          const frame = () => new Promise(resolve => requestAnimationFrame(resolve));
          while (!element.querySelector(`tbody tr:not(.pending)[aria-rowindex="${row + 2}"]`)) {
            if (performance.now() - start > 10000) throw new Error('Destination rows did not render');
            await frame();
          }
          await frame(); await frame();
          return performance.now() - start;
        }, row));
        assert.ok(await grid.locator('tbody tr:not(.spacer)').count() < 60);
      }
      // Exercise the real backend's held preview and full-page release path too.
      const bar = await grid.getByRole('scrollbar').boundingBox();
      const preview = grid.waitForResponse(response => response.url().endsWith('/sql') && response.request().postDataJSON().page_size < 100);
      await grid.mouse.move(bar.x + bar.width / 2, bar.y + bar.height * 0.7);
      await grid.mouse.down();
      await grid.mouse.move(bar.x + bar.width / 2, bar.y + bar.height * 0.7 + 100, { steps: 5 });
      const stopped = performance.now();
      assert.ok((await preview).ok());
      await visibleLoaded(grid);
      const previewMs = performance.now() - stopped;
      const landing = grid.waitForResponse(response => response.url().endsWith('/sql') && response.request().postDataJSON().page_size === 100);
      await grid.mouse.up();
      assert.ok((await landing).ok());
      await visibleLoaded(grid);
      const bytes = Buffer.from(measured.payloads[format]);
      const result = { format, columns: width, rows: measured.rows, total_rows: measured.total, bytes: bytes.length, gzip_bytes: gzipSync(bytes).length, fetch_median_ms: round(median(measured.samples[format])), decode_viewport_median_ms: round(median(measured.timings[format])), scroll_paint_median_ms: round(median(paint)), held_preview_ms: round(previewMs) };
      console.log(JSON.stringify(result));
      if (process.env.QUARK_SCROLL_MAX_MS && format === 'arrow') {
        const limit = Number(process.env.QUARK_SCROLL_MAX_MS);
        assert.ok(result.scroll_paint_median_ms < limit, `Scroll median ${result.scroll_paint_median_ms} ms exceeds ${limit} ms`);
        assert.ok(result.held_preview_ms < limit, `Held preview ${result.held_preview_ms} ms exceeds ${limit} ms`);
      }
      await grid.close();
    }
  }
  assert.deepEqual(errors, []);
  console.log('PASS AllSpecs value parity, bounded grid, scroll/preview/release');
} finally {
  await browser.close();
  await server.close();
}
