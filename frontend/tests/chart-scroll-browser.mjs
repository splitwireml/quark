// Run with PLAYWRIGHT_MODULE=<playwright entry> node tests/chart-scroll-browser.mjs.
// Every API is mocked: this exercises the real chart layout without user data.
// Set MINIMAL=1 for the one-column, 20-bar boundary case.
// Also checks grouped/single box plots with extreme outliers.
import assert from "node:assert/strict";
import { realpathSync } from "node:fs";
import { createServer } from "vite";

const { chromium } = await import(process.env.PLAYWRIGHT_MODULE || "playwright");
const root = new URL("..", import.meta.url).pathname;
const server = await createServer({
  root,
  server: { host: "127.0.0.1", port: 0, fs: { allow: [root, realpathSync(new URL("../node_modules", import.meta.url))] } }
});
await server.listen();

const browser = await chromium.launch({ headless: true });
const page = await browser.newPage({ viewport: { width: 1280, height: 900 }, reducedMotion: "reduce" });
page.setDefaultTimeout(3000);
page.setDefaultNavigationTimeout(15000);
const errors = [];
let crashed = false;
page.on("pageerror", (error) => errors.push(error.message));
page.on("crash", () => { crashed = true; });

const minimal = process.env.MINIMAL === "1";
const allColumns = [
  { name: "name", type: "VARCHAR", numeric: false, null_fraction: 0, profile_kind: "categorical" },
  { name: "host_name", type: "VARCHAR", numeric: false, null_fraction: 0, profile_kind: "categorical" },
  { name: "neighbourhood_group", type: "VARCHAR", numeric: false, null_fraction: 0, profile_kind: "categorical" },
  { name: "neighbourhood", type: "VARCHAR", numeric: false, null_fraction: 0, profile_kind: "categorical" },
  { name: "room_type", type: "VARCHAR", numeric: false, null_fraction: 0, profile_kind: "categorical" },
  { name: "id", type: "BIGINT", numeric: true, null_fraction: 0, profile_kind: "numeric" },
  { name: "host_id", type: "BIGINT", numeric: true, null_fraction: 0, profile_kind: "numeric" },
  { name: "price", type: "BIGINT", numeric: true, null_fraction: 0, profile_kind: "numeric" },
  { name: "minimum_nights", type: "BIGINT", numeric: true, null_fraction: 0, profile_kind: "numeric" },
  { name: "number_of_reviews", type: "BIGINT", numeric: true, null_fraction: 0, profile_kind: "numeric" },
  { name: "calculated_host_listings_count", type: "BIGINT", numeric: true, null_fraction: 0, profile_kind: "numeric" },
  { name: "availability_365", type: "BIGINT", numeric: true, null_fraction: 0, profile_kind: "numeric" },
  { name: "latitude", type: "DOUBLE", numeric: true, null_fraction: 0, profile_kind: "numeric" },
  { name: "longitude", type: "DOUBLE", numeric: true, null_fraction: 0, profile_kind: "numeric" }
];
const columns = minimal ? allColumns.filter((column) => column.name === "price") : allColumns;
const view = {
  id: "view", project_id: "test", source_id: "source", source_name: "ab_nyc_2019.csv",
  node_id: "node", name: "ab_nyc_2019", schema: "main", type: "VIEW",
  columns: columns.map((column) => column.name), sql: "SELECT * FROM ab_nyc_2019"
};
const bars = Array.from({ length: minimal ? 20 : 30 }, (_, index) => ({ label: 100 + index * 5, value: 2050 - index * 55 }));
const boxGroups = Array.from({ length: minimal ? 1 : 12 }, (_, index) => ({
  label: minimal ? 'all' : `Neighbourhood ${index + 1}`,
  whisker_low: -20, p25: 20, median: 40, p75: 60, whisker_high: 100,
  outliers: [-10000, 10000], count: 1000
}));

try {
  await page.route("**/api/**", async (route) => {
    const path = new URL(route.request().url()).pathname;
    let body;
    if (path === "/api/projects") body = [{ id: "test", name: "NYC", node_id: "node", source_count: 1 }];
    else if (path.endsWith("/sources")) body = [{ id: "source", name: "ab_nyc_2019.csv" }];
    else if (path.endsWith("/sources/source")) body = { id: "source", name: "ab_nyc_2019.csv", kind: "upload", project_id: "test", views: [view] };
    else if (path.endsWith("/views")) body = [view];
    else if (path.endsWith("/sql")) {
      const query = route.request().postDataJSON();
      const row = Object.fromEntries(columns.map((column) => [column.name, column.name === "price" ? 100 : column.numeric ? 1 : "value"]));
      body = { columns, rows: [row], page: query.page, page_size: query.page_size, total_rows: 48895, total_pages: 489, elapsed_ms: 1, sql: query.sql };
    } else if (path.endsWith("/visualize")) {
      body = route.request().postDataJSON().spec.chart === 'box'
        ? { chart: 'box', groups: boxGroups, elapsed_ms: 1 }
        : { chart: "bar", rows: bars, other_count: 20208, elapsed_ms: 120 };
    } else throw new Error(`Unexpected API: ${path}`);
    await route.fulfill({ json: body }).catch(() => {});
  });

  await page.goto(server.resolvedUrls.local[0]);
  await page.getByRole("button", { name: "NYC 1 source" }).click();
  await page.getByRole("button", { name: /ab_nyc_2019.csv/ }).first().click();
  await page.locator("tbody td[data-column]").first().waitFor();
  await page.getByRole("button", { name: "Chart", exact: true }).click();
  await page.locator("button.field").filter({ hasText: /^price/ }).click();
  await page.getByRole("img", { name: "Bar chart" }).waitFor();

  const readLayout = () => page.evaluate(() => {
    const scroller = document.querySelector(".scroller");
    const pane = document.querySelector(".pane");
    if (!(scroller instanceof HTMLElement) || !(pane instanceof HTMLElement)) throw new Error("Chart layout missing");
    return {
      viewport: window.innerWidth,
      pageScrollLeft: window.scrollX,
      documentWidth: document.documentElement.scrollWidth,
      paneRight: pane.getBoundingClientRect().right,
      chartClientWidth: scroller.clientWidth,
      chartScrollWidth: scroller.scrollWidth,
      chartScrollLeft: scroller.scrollLeft
    };
  });
  const before = await readLayout();
  assert.ok(before.chartScrollWidth > before.chartClientWidth, "wide bars must overflow inside the chart scroller");
  await page.locator(".scroller").hover();
  await page.mouse.wheel(500, 0);
  await page.waitForFunction((scrollLeft) => {
    const scroller = document.querySelector(".scroller");
    return scroller instanceof HTMLElement && scroller.scrollLeft > scrollLeft;
  }, before.chartScrollLeft);
  const after = await readLayout();
  console.log(JSON.stringify({ before, after, crashed, errors }));
  assert.ok(after.chartScrollLeft > before.chartScrollLeft, "horizontal wheel input must scroll the chart");
  assert.equal(after.pageScrollLeft, before.pageScrollLeft, "horizontal wheel input must not scroll the page");
  assert.ok(Math.abs(after.paneRight - before.paneRight) <= 1, "chart options must stay fixed while the chart scrolls");
  assert.ok(before.documentWidth <= before.viewport, `chart must not widen the page (${before.documentWidth}px > ${before.viewport}px)`);
  assert.ok(before.paneRight <= before.viewport, `chart options must stay in the viewport (${before.paneRight}px > ${before.viewport}px)`);
  assert.equal(crashed, false, "chart interaction must not crash the page");
  assert.deepEqual(errors, [], "chart must not raise a browser runtime error");
  console.log("PASS wide bar chart scrolls internally without widening the page");

  if (!minimal) await page.locator('button.field').filter({ hasText: /^neighbourhood\s+VARCHAR/ }).click();
  await page.locator('.types button').filter({ hasText: 'Box' }).click();
  await page.getByRole('img', { name: 'Box plot' }).waitFor();
  const boxes = page.locator('svg g.box');
  const boxHeight = () => boxes.first().locator('rect').evaluate((rect) => rect.getBoundingClientRect().height);
  assert.ok(await boxHeight() > 40, 'extreme outliers must not flatten the default boxes');
  assert.equal(await boxes.locator('circle').count(), 0, 'outlier marks start hidden');

  const focused = await readLayout();
  if (!minimal) {
    assert.ok(focused.chartScrollWidth > focused.chartClientWidth, 'grouped boxes must have room and scroll internally');
    const axis = await page.locator('.y-lock').boundingBox();
    await page.locator('.scroller').hover();
    await page.mouse.wheel(500, 0);
    await page.waitForFunction(() => document.querySelector('.scroller').scrollLeft > 0);
    assert.deepEqual(await page.locator('.y-lock').boundingBox(), axis, 'vertical axis stays fixed');
    const scrolled = await readLayout();
    assert.equal(scrolled.pageScrollLeft, 0);
    assert.ok(scrolled.documentWidth <= scrolled.viewport, 'boxes must not widen the page');
    assert.equal(scrolled.paneRight, focused.paneRight, 'box options stay fixed');
  } else {
    assert.equal(focused.chartScrollWidth, focused.chartClientWidth, 'a single box fits without scrolling');
  }

  const outlierToggle = page.getByRole('checkbox', { name: 'Show outliers', exact: true });
  await outlierToggle.focus();
  await page.keyboard.press('Space');
  await page.waitForFunction(() => document.querySelectorAll('svg g.box circle').length > 0);
  assert.equal(await boxes.locator('circle').count(), boxGroups.length * 2);
  assert.ok(await boxHeight() < 10, 'showing outliers restores their full scale');
  await page.keyboard.press('Space');
  await page.waitForFunction(() => document.querySelectorAll('svg g.box circle').length === 0);
  assert.ok(await boxHeight() > 40, 'hiding outliers restores the readable scale');
  await page.locator('.scroller').evaluate((node) => { node.scrollLeft = 0; });
  await boxes.first().focus();
  assert.match(await page.locator('.hover').innerText(), /Median 40/);
  assert.match(await page.locator('.hover').innerText(), /2 outliers/);
  if (process.env.SCREENSHOT) await page.screenshot({ path: process.env.SCREENSHOT });
  assert.deepEqual(errors, []);
  console.log('PASS box plots focus on whiskers, toggle outliers with the keyboard, and scroll internally');
} finally {
  await browser.close();
  await server.close();
}
