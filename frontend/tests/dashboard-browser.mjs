// Run with PLAYWRIGHT_MODULE=<playwright entry> node tests/dashboard-browser.mjs.
// API calls are mocked; this verifies dashboard interaction without touching user data.
import assert from 'node:assert/strict';
import { realpathSync } from 'node:fs';
import { createServer } from 'vite';

const { chromium } = await import(process.env.PLAYWRIGHT_MODULE || 'playwright');
const root = new URL('..', import.meta.url).pathname;
const server = await createServer({ root, server: { host: '127.0.0.1', port: 0, fs: { allow: [root, realpathSync(new URL('../node_modules', import.meta.url))] } } });
await server.listen();

const browser = await chromium.launch({ headless: true });
const page = await browser.newPage({ viewport: { width: 1280, height: 900 }, reducedMotion: 'reduce' });
page.setDefaultTimeout(5000);
const errors = [];
const visualizeRequests = [];
page.on('pageerror', (error) => errors.push(error.message));

const columns = [
  { name: 'region', type: 'VARCHAR', numeric: false, null_fraction: 0, profile_kind: 'categorical' },
  { name: 'x', type: 'DOUBLE', numeric: true, null_fraction: 0, profile_kind: 'numeric' },
  { name: 'y', type: 'DOUBLE', numeric: true, null_fraction: 0, profile_kind: 'numeric' }
];
const view = { id: 'view', project_id: 'test', source_id: 'source', source_name: 'sales.csv', node_id: 'node', name: 'sales', schema: 'main', type: 'VIEW', columns: columns.map(({ name }) => name), sql: 'SELECT * FROM sales' };

try {
  await page.route('**/api/**', async (route) => {
    const path = new URL(route.request().url()).pathname;
    let body;
    if (path === '/api/projects') body = [{ id: 'test', name: 'Sales', node_id: 'node', source_count: 1 }];
    else if (path.endsWith('/sources')) body = [{ id: 'source', name: 'sales.csv' }];
    else if (path.endsWith('/sources/source')) body = { id: 'source', name: 'sales.csv', kind: 'upload', project_id: 'test', views: [view] };
    else if (path.endsWith('/views')) body = [view];
    else if (path.endsWith('/sql')) {
      const request = route.request().postDataJSON();
      body = { columns, rows: [{ region: 'East', x: 2, y: 4 }], page: 1, page_size: request.page_size, total_rows: 3, total_pages: 1, elapsed_ms: 1, sql: request.sql };
    } else if (path.endsWith('/visualize')) {
      const request = route.request().postDataJSON();
      visualizeRequests.push(structuredClone(request));
      body = request.spec.chart === 'scatter'
        ? { chart: 'scatter', points: [{ x: 1, y: 2 }, { x: 3, y: 5 }, { x: 6, y: 8 }], total_points: 3, elapsed_ms: 1 }
        : { chart: 'bar', rows: [{ label: 'East', value: 2 }, { label: 'West', value: 1 }], other_count: 0, elapsed_ms: 1 };
    } else throw new Error(`Unexpected API: ${path}`);
    await route.fulfill({ json: body });
  });

  await page.goto(server.resolvedUrls.local[0]);
  await page.getByRole('button', { name: 'Sales 1 source' }).click();
  await page.getByRole('button', { name: /sales.csv/ }).first().click();
  await page.locator('tbody td[data-column]').first().waitFor();

  await page.getByRole('button', { name: 'Chart', exact: true }).click();
  await page.locator('button.field').filter({ hasText: /^region/ }).click();
  await page.getByRole('img', { name: 'Bar chart' }).waitFor();
  await page.getByRole('button', { name: 'Add to dashboard' }).click();
  await page.getByRole('textbox', { name: 'New dashboard name' }).fill('Overview');
  await page.getByRole('button', { name: 'Create' }).click();

  await page.locator('button.field').filter({ hasText: /^region/ }).click();
  await page.locator('button.field').filter({ hasText: /^x/ }).click();
  await page.locator('button.field').filter({ hasText: /^y/ }).click();
  await page.getByRole('img', { name: 'Scatter plot' }).waitFor();
  await page.getByRole('button', { name: 'Add to dashboard' }).click();
  await page.getByRole('button', { name: 'Overview', exact: true }).click();

  await page.getByRole('button', { name: 'Dashboard', exact: true }).click();
  await page.getByRole('complementary', { name: 'Chart options' }).waitFor();
  await page.locator('article.tile').filter({ hasText: 'Bar · region' }).waitFor();
  await page.locator('article.tile').filter({ hasText: 'Scatter · x × y' }).waitFor();

  await page.locator('article.tile').filter({ hasText: 'Bar · region' }).locator('rect.bar').first().click();
  await page.getByRole('button', { name: 'Remove filter region' }).waitFor();
  await page.waitForFunction(() => document.querySelectorAll('article.tile').length === 2);
  const barAfterBar = visualizeRequests.filter((request) => request.spec.chart === 'bar').at(-1);
  const scatterAfterBar = visualizeRequests.filter((request) => request.spec.chart === 'scatter').at(-1);
  assert.equal(barAfterBar.filters.some((filter) => filter.column === 'region'), false, 'source bar keeps its unfiltered context');
  assert.equal(scatterAfterBar.filters.some((filter) => filter.column === 'region'), true, 'other charts receive the bar selection');

  const scatterHit = page.locator('article.tile').filter({ hasText: 'Scatter · x × y' }).locator('button.hit');
  const bounds = await scatterHit.boundingBox();
  assert.ok(bounds);
  await page.mouse.move(bounds.x + bounds.width * 0.2, bounds.y + bounds.height * 0.2);
  await page.mouse.down();
  await page.mouse.move(bounds.x + bounds.width * 0.7, bounds.y + bounds.height * 0.7);
  await page.mouse.up();
  await page.getByRole('button', { name: 'Filter', exact: true }).click();
  await page.getByRole('button', { name: 'Remove filter x' }).first().waitFor();
  await page.waitForTimeout(100);
  const barAfterScatter = visualizeRequests.filter((request) => request.spec.chart === 'bar').at(-1);
  const scatterAfterScatter = visualizeRequests.filter((request) => request.spec.chart === 'scatter').at(-1);
  assert.equal(barAfterScatter.filters.filter((filter) => filter.column === 'x' || filter.column === 'y').length, 4, 'bar receives the scatter range');
  assert.deepEqual(scatterAfterScatter.filters.map((filter) => filter.column), ['region'], 'scatter omits its own range but keeps the bar selection');

  await page.getByRole('button', { name: 'Remove filter region' }).click();
  await page.getByRole('button', { name: 'Remove filter region' }).waitFor({ state: 'detached' });
  assert.equal(await page.getByRole('button', { name: 'Remove filter x' }).count(), 2, 'removing one source selection leaves the other');
  await page.getByRole('button', { name: 'Remove filter x' }).first().click();
  await page.getByRole('button', { name: 'Remove filter x' }).first().waitFor({ state: 'detached' });
  assert.equal(await page.getByRole('button', { name: 'Remove filter y' }).count(), 0, 'closing one range pill clears its whole chart selection');

  await scatterHit.focus();
  await page.keyboard.press('Enter');
  await page.keyboard.press('ArrowRight');
  await page.keyboard.press('Shift+ArrowDown');
  await page.keyboard.press('Enter');
  await page.getByRole('button', { name: 'Remove filter x' }).first().waitFor();
  await page.getByRole('button', { name: 'Remove filter x' }).first().click();
  await page.getByRole('button', { name: 'Remove filter x' }).first().waitFor({ state: 'detached' });

  await page.getByRole('button', { name: 'New dashboard' }).click();
  await page.getByRole('textbox', { name: 'New dashboard name' }).fill('Sales detail');
  await page.getByRole('textbox', { name: 'New dashboard name' }).press('Enter');
  assert.equal(await page.locator('article.tile').count(), 0, 'new tabs start with an independent layout');

  await page.getByRole('group', { name: 'Chart type' }).getByRole('button', { name: 'Scatter' }).click();
  await page.locator('article.tile').filter({ hasText: 'Scatter · x × y' }).waitFor();
  await page.getByRole('button', { name: 'Remove Scatter · x × y' }).click();
  assert.equal(await page.locator('article.tile').count(), 0, 'icon remove clears the tile without leaving the dashboard');

  const surface = page.locator('.canvas-surface');
  await surface.click({ position: { x: 100, y: 100 } });
  assert.equal(await page.locator('article.tile').count(), 0, 'a single click does not create a rectangle');
  assert.equal(await page.getByRole('region', { name: 'Choose a chart' }).count(), 0, 'sketching does not open a chart picker');
  await surface.dblclick({ position: { x: 300, y: 300 } });
  await page.waitForFunction(() => document.querySelectorAll('article.tile').length === 1);
  assert.equal(await page.getByRole('region', { name: 'Choose a chart' }).count(), 0);

  const move = page.getByRole('button', { name: 'Move Scatter · x × y', exact: true });
  const beforeMove = await move.boundingBox();
  assert.ok(beforeMove);
  await page.mouse.move(beforeMove.x + 30, beforeMove.y + 15);
  await page.mouse.down();
  await page.mouse.move(beforeMove.x + 70, beforeMove.y + 55);
  await page.mouse.up();
  const afterMove = await move.boundingBox();
  assert.ok(afterMove && afterMove.x > beforeMove.x && afterMove.y > beforeMove.y, 'tile moves through real pointer input');

  const dashboardScroller = page.locator('.workspace > .scroller');
  await dashboardScroller.evaluate((node) => { node.scrollTop = 200; });
  const resize = page.getByRole('button', { name: 'Resize Scatter · x × y' });
  const tile = page.locator('article.tile');
  const beforeResize = await tile.boundingBox();
  const resizeBounds = await resize.boundingBox();
  assert.ok(beforeResize && resizeBounds);
  await page.mouse.move(resizeBounds.x + 5, resizeBounds.y + 5);
  await page.mouse.down();
  await page.mouse.move(resizeBounds.x + 85, resizeBounds.y + 55);
  await page.mouse.up();
  const afterResize = await tile.boundingBox();
  assert.ok(afterResize && afterResize.width > beforeResize.width && afterResize.height > beforeResize.height, 'tile resizes and its chart reflows');

  await dashboardScroller.evaluate((node) => { node.scrollTop = 700; });
  const scrollerBounds = await dashboardScroller.boundingBox();
  assert.ok(scrollerBounds);
  await page.mouse.move(scrollerBounds.x + scrollerBounds.width * 0.55, scrollerBounds.y + 120);
  await page.mouse.down();
  await page.mouse.move(scrollerBounds.x + scrollerBounds.width * 0.8, scrollerBounds.y + 340);
  await page.mouse.up();
  await page.waitForFunction(() => document.querySelectorAll('article.tile').length === 2);
  assert.equal(await page.getByRole('region', { name: 'Choose a chart' }).count(), 0);

  await page.getByRole('tab', { name: 'Overview' }).click();
  assert.equal(await page.locator('article.tile').count(), 2, 'tab layouts remain independent');
  if (process.env.SCREENSHOT) await page.screenshot({ path: process.env.SCREENSHOT });
  assert.deepEqual(errors, []);
  console.log('PASS dashboard placement, tabs, cross-filter intersections, source context, and pill clearing');
} finally {
  await browser.close();
  await server.close();
}
