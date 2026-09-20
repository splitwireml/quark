// Focused browser check for grouped scatter palette rendering.
import assert from "node:assert/strict";
import { createServer } from "vite";
import { realpathSync } from "node:fs";

const { chromium } = await import(process.env.PLAYWRIGHT_MODULE || "playwright");
const root = new URL("..", import.meta.url).pathname;
const server = await createServer({
  root,
  server: { host: "127.0.0.1", port: 0, fs: { allow: [root, realpathSync(new URL("../node_modules", import.meta.url))] } }
});
await server.listen();

const browser = await chromium.launch({ headless: true });
const page = await browser.newPage({ viewport: { width: 1280, height: 900 } });
page.setDefaultTimeout(3000);
page.setDefaultNavigationTimeout(15000);
await page.addInitScript(() => {
  window.__scatterFillStyles = [];
  const fill = CanvasRenderingContext2D.prototype.fill;
  CanvasRenderingContext2D.prototype.fill = function (...args) {
    if (this.canvas?.classList.contains("dots")) window.__scatterFillStyles.push(String(this.fillStyle));
    return fill.apply(this, args);
  };
});

const errors = [];
page.on("pageerror", (error) => errors.push(error.message));
const columns = [
  { name: "x", type: "DOUBLE", numeric: true, null_fraction: 0, profile_kind: "numeric" },
  { name: "y", type: "DOUBLE", numeric: true, null_fraction: 0, profile_kind: "numeric" },
  { name: "group", type: "VARCHAR", numeric: false, null_fraction: 0, profile_kind: "categorical" }
];
const groupedPoints = [
  { x: 1, y: 1, color: "A" },
  { x: 2, y: 2, color: "A" },
  { x: 3, y: 3, color: "B" },
  { x: 4, y: 4, color: "B" }
];
const view = { id: "view", project_id: "test", source_id: "source", source_name: "Points.csv", node_id: "node", name: "Points", schema: "main", type: "VIEW", columns: columns.map((column) => column.name), sql: "SELECT * FROM points" };
const requests = [];

async function waitFor(predicate, timeout = 2000) {
  const started = Date.now();
  while (!predicate()) {
    if (Date.now() - started > timeout) throw new Error("Timed out waiting for chart request");
    await new Promise((resolve) => setTimeout(resolve, 20));
  }
}

async function waitForFills(count) {
  await page.waitForFunction((minimum) => (window.__scatterFillStyles?.length ?? 0) >= minimum, count);
}

async function latestFills() {
  return page.evaluate(() => window.__scatterFillStyles.slice(-4));
}

try {
  await page.route("**/api/**", async (route) => {
    const path = new URL(route.request().url()).pathname;
    let body;
    if (path === "/api/projects") body = [{ id: "test", name: "Scatter test", node_id: "node", source_count: 1 }];
    else if (path.endsWith("/sources")) body = [{ id: "source", name: "Points.csv" }];
    else if (path.endsWith("/sources/source")) body = { id: "source", name: "Points.csv", kind: "upload", project_id: "test", views: [view] };
    else if (path.endsWith("/views")) body = [view];
    else if (path.endsWith("/sql")) {
      const query = route.request().postDataJSON();
      body = { columns, rows: [{ x: 1, y: 1, group: "A" }], page: query.page, page_size: query.page_size, total_rows: 1, total_pages: 1, elapsed_ms: 1, sql: query.sql };
    } else if (path.endsWith("/visualize")) {
      const query = route.request().postDataJSON();
      requests.push(query);
      if (query.spec.chart === "scatter") {
        const color = query.spec.encodings.color === "group";
        body = { chart: "scatter", points: color ? groupedPoints : groupedPoints.map(({ x, y }) => ({ x, y })), total_points: 4, elapsed_ms: 1 };
      } else {
        body = { chart: "histogram", bins: [{ lower: 1, upper: 2, count: 2 }], elapsed_ms: 1 };
      }
    } else throw new Error(`Unexpected API: ${path}`);
    await route.fulfill({ json: body }).catch(() => {});
  });

  await page.goto(server.resolvedUrls.local[0]);
  await page.getByRole("button", { name: "Scatter test 1 source" }).click();
  await page.getByRole("button", { name: /Points.csv/ }).first().click();
  await page.locator("tbody td[data-column]").first().waitFor();
  await page.getByRole("button", { name: "Chart", exact: true }).click();
  await page.locator("button.field").filter({ hasText: /^x/ }).click();
  await page.locator("button.field").filter({ hasText: /^y/ }).click();
  await waitFor(() => requests.some((query) => query.spec.chart === "scatter"));

  await page.getByRole("button", { name: "Settings", exact: true }).click();
  await page.locator("label:has(input[value='multicolor'])").click();
  assert.equal(await page.locator("input[value='multicolor']").isChecked(), true);
  await page.getByRole("button", { name: "← Back to View", exact: true }).click();

  await page.evaluate(() => { window.__scatterFillStyles = []; });
  await page.locator("button.field").filter({ hasText: /^group/ }).click();
  await waitFor(() => requests.some((query) => query.spec.chart === "scatter" && query.spec.encodings.color === "group"));
  await waitForFills(4);
  const groupedFills = await latestFills();
  assert.equal(groupedFills[0], groupedFills[1], "repeated category points should share a fill");
  assert.equal(groupedFills[2], groupedFills[3], "repeated category points should share a fill");
  assert.notEqual(groupedFills[0], groupedFills[2], "different categories should receive different fills");

  const requestCount = requests.length;
  await page.getByRole("button", { name: "Settings", exact: true }).click();
  await page.evaluate(() => { window.__scatterFillStyles = []; });
  await page.locator("label:has(input[value='colorblind'])").click();
  await page.getByRole("button", { name: "← Back to View", exact: true }).click();
  await waitForFills(4);
  const recoloredFills = await latestFills();
  assert.notDeepEqual(recoloredFills, groupedFills, "switching palettes should repaint scatter marks");
  assert.equal(requests.length, requestCount, "switching palettes must not request chart data");
  assert.equal(await page.evaluate(() => document.documentElement.dataset.chartMode), "multicolor");
  assert.equal(await page.evaluate(() => document.documentElement.dataset.chartPalette), "colorblind");
  assert.deepEqual(errors, []);
  console.log(JSON.stringify({ groupedFills, recoloredFills, requests: requests.length }));
  console.log("PASS grouped scatter palettes are stable, distinct, and repaint without a data request");
} finally {
  await browser.close();
  await server.close();
}
