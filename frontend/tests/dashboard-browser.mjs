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
        : request.spec.chart === 'histogram'
          ? { chart: 'histogram', bins: [{ lower: 1, upper: 4, count: 2 }], elapsed_ms: 1 }
        : request.spec.chart === 'metric'
          ? { chart: 'metric', value: 42, rows: 3, elapsed_ms: 1 }
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
  assert.equal(await page.getByRole('textbox', { name: 'New dashboard name' }).count(), 0);
  assert.equal(await page.evaluate(() => JSON.parse(localStorage.getItem('quark.dashboards.v1'))[0].tabs[0].name), 'Dashboard_1');

  await page.locator('button.field').filter({ hasText: /^region/ }).click();
  await page.locator('button.field').filter({ hasText: /^x/ }).click();
  await page.locator('button.field').filter({ hasText: /^y/ }).click();
  await page.getByRole('img', { name: 'Scatter plot' }).waitFor();
  await page.getByRole('button', { name: 'Add to dashboard' }).click();

  await page.getByRole('button', { name: 'Dashboard', exact: true }).click();
  await page.getByRole('complementary', { name: 'Chart options' }).waitFor();
  await page.getByRole('tab', { name: 'Dashboard_1', exact: true }).dblclick();
  await page.getByRole('textbox', { name: 'Dashboard name', exact: true }).fill('Overview');
  await page.getByRole('textbox', { name: 'Dashboard name', exact: true }).press('Enter');
  await page.getByRole('tab', { name: 'Overview', exact: true }).waitFor();

  await page.locator('article.tile').filter({ hasText: 'Bar · region' }).waitFor();
  await page.locator('article.tile').filter({ hasText: 'Scatter · x × y' }).waitFor();
  await page.getByRole('button', { name: 'Move Bar · region', exact: true }).dblclick();
  await page.getByRole('textbox', { name: 'Chart title', exact: true }).fill('Regional sales');
  await page.getByRole('textbox', { name: 'Chart title', exact: true }).press('Enter');
  assert.equal(await page.evaluate(() => JSON.parse(localStorage.getItem('quark.dashboards.v1'))[0].charts[0].title), 'Regional sales');
  await page.getByRole('button', { name: 'Move Regional sales', exact: true }).press('F2');
  await page.getByRole('textbox', { name: 'Chart title', exact: true }).fill('Discard this');
  await page.getByRole('textbox', { name: 'Chart title', exact: true }).press('Escape');
  await page.getByRole('button', { name: 'Move Regional sales', exact: true }).press('F2');
  await page.getByRole('textbox', { name: 'Chart title', exact: true }).fill('Bar · region');
  await page.getByRole('textbox', { name: 'Chart title', exact: true }).press('Enter');


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
  await page.getByRole('tab', { name: 'Dashboard_1' }).dblclick();
  await page.getByRole('textbox', { name: 'Dashboard name', exact: true }).fill('Sales detail');
  await page.getByRole('textbox', { name: 'Dashboard name', exact: true }).press('Enter');
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

  // A selected tile copies, pastes as an offset twin that shares its definition, and clears.
  await move.click();
  await page.locator('article.tile.selected').waitFor();
  await page.keyboard.press('ControlOrMeta+c');
  await page.keyboard.press('ControlOrMeta+v');
  await page.waitForFunction(() => document.querySelectorAll('article.tile').length === 2);
  assert.equal(await page.locator('article.tile.selected').count(), 1, 'the pasted copy is the selected tile');
  const pasted = await page.evaluate(() => JSON.parse(localStorage.getItem('quark.dashboards.v1'))[0].tabs.at(-1).placements);
  assert.equal(pasted.at(-1).chartId, pasted.at(-2).chartId, 'a copy shares the chart definition until one tile is edited');
  assert.ok(pasted.at(-1).x > pasted.at(-2).x && pasted.at(-1).y > pasted.at(-2).y, 'the copy lands offset from its source');
  await page.keyboard.press('Delete');
  await page.waitForFunction(() => document.querySelectorAll('article.tile').length === 1);

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

  await page.getByRole('button', { name: 'New dashboard', exact: true }).click();
  const palette = page.getByRole('complementary', { name: 'Chart options' });
  const board = page.locator('.board');
  const boardBounds = await board.boundingBox();
  const viewport = await dashboardScroller.boundingBox();
  assert.ok(Math.abs(boardBounds.width - viewport.width) < 2, 'canvas fills the viewport behind the palette');

  for (const colorScheme of ['light', 'dark']) {
    await page.emulateMedia({ colorScheme });
    await page.getByRole('button', { name: 'Minimize chart palette' }).hover();
    await page.getByRole('tooltip').waitFor();
    const tooltip = await page.getByRole('tooltip').boundingBox();
    const panel = await palette.boundingBox();
    assert.ok(tooltip.y < panel.y, 'tooltip escapes the top of the floating palette');
    assert.equal(await page.getByRole('tooltip').evaluate((node) => node.matches(':popover-open')), true);
    if (process.env.SCREENSHOT) await page.screenshot({ path: process.env.SCREENSHOT.replace('.png', `-${colorScheme}.png`) });
    await page.mouse.move(10, 10);
  }

  const handle = page.getByRole('button', { name: 'Move chart palette' });
  const beforePalette = await palette.boundingBox();
  const handleBounds = await handle.boundingBox();
  await page.mouse.move(handleBounds.x + 40, handleBounds.y + 15);
  await page.mouse.down();
  await page.mouse.move(handleBounds.x - 160, handleBounds.y + 65, { steps: 6 });
  await page.mouse.up();
  const movedPalette = await palette.boundingBox();
  assert.ok(Math.abs(movedPalette.x - beforePalette.x + 200) < 2 && movedPalette.y > beforePalette.y, 'palette is freely draggable');

  await page.emulateMedia({ reducedMotion: 'no-preference' });
  await page.getByRole('button', { name: 'Minimize chart palette' }).click();
  await page.getByRole('button', { name: 'Restore chart palette' }).waitFor();
  const dock = await palette.boundingBox();
  assert.equal(Math.round(dock.width), 40);
  assert.ok(Math.abs(dock.x + dock.width - viewport.x - viewport.width + 12) < 2, 'minimized palette docks at the right edge from any position');
  await page.getByRole('button', { name: 'Restore chart palette' }).click();
  await palette.evaluate(async (node) => { await Promise.all(node.getAnimations().map((animation) => animation.finished)); });
  const restoredPalette = await palette.boundingBox();
  assert.ok(Math.abs(restoredPalette.x - movedPalette.x) < 2 && Math.abs(restoredPalette.y - movedPalette.y) < 2, 'restore returns to the floating position');
  await page.emulateMedia({ reducedMotion: 'reduce' });
  await page.getByRole('button', { name: 'Minimize chart palette' }).click();
  await page.getByRole('button', { name: 'Restore chart palette' }).waitFor();
  assert.equal(await palette.evaluate((node) => node.getAnimations().length), 0, 'reduced motion docks immediately');

  async function drawTile(x, y, width, height) {
    const rect = await board.boundingBox();
    await page.mouse.move(rect.x + x, rect.y + y);
    await page.mouse.down();
    await page.mouse.move(rect.x + x + width, rect.y + y + height, { steps: 4 });
    await page.mouse.up();
  }
  await drawTile(30, 70, 173, 119);
  const smallTile = page.locator('article.tile').first();
  const smallBounds = await smallTile.boundingBox();
  assert.equal(smallBounds.width, 173, 'dragged width is preserved below the old minimum');
  assert.equal(smallBounds.height, 119, 'dragged height is preserved below the old minimum');
  await drawTile(330, 70, 220, 210);
  const grip = await smallTile.locator('.resize').boundingBox();
  await page.mouse.move(grip.x + 14, grip.y + 14);
  await page.mouse.down();
  await page.mouse.move(grip.x + 14 + 125, grip.y + 14 + 89);
  assert.equal(await page.locator('.snap-guide').count(), 2, 'resize displays both neighboring alignment guides');
  if (process.env.SCREENSHOT) await page.screenshot({ path: process.env.SCREENSHOT });
  await page.mouse.up();
  const snappedBounds = await smallTile.boundingBox();
  assert.equal(snappedBounds.width, 300, 'right edge snaps to the neighboring left edge');
  assert.equal(snappedBounds.height, 210, 'bottom edge snaps to the neighboring bottom edge');
  assert.equal(await page.locator('.snap-guide').count(), 0, 'guides disappear after resize');

  const dragHandle = await smallTile.locator('.move').boundingBox();
  await page.mouse.move(dragHandle.x + 12, dragHandle.y + 15);
  await page.mouse.down();
  await page.mouse.move(dragHandle.x + 12 + 258, dragHandle.y + 18);
  assert.equal(await page.locator('.snap-guide').count(), 2, 'moving displays alignment guides');
  assert.equal(await page.locator('.snap-guide').first().evaluate((node) => Number.parseFloat(node.style.left)), 440, 'moving aligns chart centers');
  if (process.env.SCREENSHOT) await page.screenshot({ path: process.env.SCREENSHOT.replace('.png', '-move.png') });
  await page.mouse.up();
  const movedBounds = await smallTile.boundingBox();
  assert.equal(movedBounds.width, snappedBounds.width);
  assert.equal(movedBounds.height, snappedBounds.height);
  assert.equal(Math.round(movedBounds.x - viewport.x), 290);
  assert.equal(await page.locator('.snap-guide').count(), 0);

  const editedTile = page.locator('article.tile').nth(1);
  const beforeEditing = await editedTile.boundingBox();
  const requestCount = visualizeRequests.length;
  await editedTile.locator('button.hit').click({ modifiers: ['Shift'] });
  await palette.getByText('Editing this chart', { exact: true }).waitFor();
  assert.equal(await editedTile.evaluate((node) => node.classList.contains('editing')), true);
  assert.equal(visualizeRequests.length, requestCount, 'Shift-click selects without brushing, filtering, or requesting data');
  assert.equal(await palette.locator('button.field').filter({ hasText: /^x/ }).getAttribute('aria-pressed'), 'true');
  assert.equal(await palette.locator('button.field').filter({ hasText: /^y/ }).getAttribute('aria-pressed'), 'true');
  await palette.locator('button.field').filter({ hasText: /^y/ }).click();
  await editedTile.getByRole('button', { name: 'Move Histogram · x', exact: true }).waitFor();
  assert.equal(await page.locator('article.tile').count(), 2, 'editing updates the tile instead of spawning another');
  assert.equal(await smallTile.getByRole('button', { name: 'Move Scatter · x × y', exact: true }).count(), 1, 'a reused chart in another tile stays unchanged');
  await editedTile.locator('.move').dblclick({ position: { x: 12, y: 15 } });
  await page.getByRole('textbox', { name: 'Chart title', exact: true }).fill('Focused chart');
  await page.getByRole('textbox', { name: 'Chart title', exact: true }).press('Enter');
  await palette.locator('button.field').filter({ hasText: /^region/ }).click();
  await palette.getByRole('group', { name: 'Chart type' }).getByRole('button', { name: 'Bar', exact: true }).click();
  await palette.getByRole('group', { name: 'Aggregation' }).getByRole('button', { name: 'Median', exact: true }).click();
  const savedEditedChart = () => page.evaluate(() => {
    const document = JSON.parse(localStorage.getItem('quark.dashboards.v1'))[0];
    const tab = document.tabs.find((item) => item.id === document.activeTabId);
    return document.charts.find((chart) => chart.id === tab.placements[1].chartId);
  });
  assert.equal((await savedEditedChart()).spec.metric, 'median', 'aggregation edits persist');
  assert.equal((await savedEditedChart()).title, 'Focused chart', 'edits preserve custom names');
  await palette.getByRole('group', { name: 'Chart type' }).getByRole('button', { name: 'Histogram', exact: true }).click();
  assert.equal((await savedEditedChart()).spec.chart, 'histogram', 'chart type edits persist');
  assert.equal(await page.locator('article.tile').count(), 2);
  await palette.locator('button.field').filter({ hasText: /^region/ }).click();
  const validSpec = (await savedEditedChart()).spec;
  await palette.locator('button.field').filter({ hasText: /^x/ }).click();
  await palette.getByText('Choose columns for a supported chart.', { exact: false }).waitFor();
  assert.deepEqual((await savedEditedChart()).spec, validSpec, 'incomplete edits keep the last valid chart');
  if (process.env.SCREENSHOT) await page.screenshot({ path: process.env.SCREENSHOT.replace('.png', '-editing.png') });
  await palette.getByRole('button', { name: 'Done', exact: true }).click();
  await editedTile.locator('.move').press('Shift+Enter');
  await palette.getByText('Editing this chart', { exact: true }).waitFor();
  assert.equal(await palette.locator('button.field').filter({ hasText: /^x/ }).getAttribute('aria-pressed'), 'true', 'reopening loads the saved chart');
  await page.keyboard.press('Escape');
  assert.equal(await palette.getByText('Editing this chart', { exact: true }).count(), 0);
  assert.deepEqual(await editedTile.boundingBox(), beforeEditing, 'editing preserves chart position and size');

  // Palette overlays charts without consuming or shifting canvas space.
  await drawTile(viewport.width - 290, 430, 270, 130);
  const behind = await page.locator('article.tile').last().boundingBox();
  const over = await palette.boundingBox();
  assert.ok(behind.x + behind.width > over.x, 'charts can occupy the space behind the palette');
  await page.setViewportSize({ width: 900, height: 700 });
  const narrowPalette = await palette.boundingBox();
  assert.ok(narrowPalette.x >= 0 && narrowPalette.x + narrowPalette.width <= 900, 'floating palette stays reachable at narrower sizes');

  assert.deepEqual(errors, []);
  console.log('PASS Shift-click editing, independent saved charts, move/resize snapping, floating palette, inline renaming, and cross-filtering');
} finally {
  await browser.close();
  await server.close();
}
