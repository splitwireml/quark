// Every API is mocked; this runs the real App without touching user data.
// Set SCATTER_SINGLE=1, SCATTER_EMPTY=1, or SCATTER_ERROR=1 for focused cases.
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
const errors = [];
page.on("pageerror", e => errors.push(e.message));
const columns = [
  { name: "x", type: "DOUBLE", numeric: true, null_fraction: 0, profile_kind: "numeric" },
  { name: "y", type: "DOUBLE", numeric: true, null_fraction: 0, profile_kind: "numeric" },
  { name: "size", type: "DOUBLE", numeric: true, null_fraction: 0, profile_kind: "numeric" },
  { name: "group", type: "VARCHAR", numeric: false, null_fraction: 0, profile_kind: "categorical" }
];
const rows = [{ x: 1, y: 2, size: 3, group: "A" }];
const points = process.env.SCATTER_EMPTY ? [] : process.env.SCATTER_SINGLE ? [{ x: 1, y: 2 }] : [{ x: 1, y: 2 }, { x: 2, y: 3 }, { x: 3, y: 1 }];
const view = { id: "view", project_id: "test", source_id: "source", source_name: "Points.csv", node_id: "node", name: "Points", schema: "main", type: "VIEW", columns: columns.map(c => c.name), sql: "SELECT * FROM points" };
const requests = [];
async function waitFor(predicate, timeout = 2000) {
  const started = Date.now();
  while (!predicate()) {
    if (Date.now() - started > timeout) throw new Error("Timed out waiting for scatter request");
    await new Promise(resolve => setTimeout(resolve, 20));
  }
}
async function canvasState() {
  return page.locator("canvas.dots").evaluate(canvas => {
    const ctx = canvas.getContext("2d");
    const pixels = ctx?.getImageData(0, 0, canvas.width, canvas.height).data ?? [];
    let painted = 0;
    for (let index = 3; index < pixels.length; index += 4) if (pixels[index] > 0) painted += 1;
    return { width: canvas.width, height: canvas.height, painted };
  });
}
async function chartGeometry() {
  return page.locator("canvas.dots").evaluate(canvas => {
    const frame = document.querySelector(".frame");
    const svg = frame?.querySelector("svg");
    const hit = frame?.querySelector("rect.hit");
    if (!frame || !svg || !hit) return null;
    const frameRect = frame.getBoundingClientRect();
    const svgRect = svg.getBoundingClientRect();
    const canvasRect = canvas.getBoundingClientRect();
    const hitRect = hit.getBoundingClientRect();
    const close = (left, right) => Math.abs(left - right) <= 1;
    return {
      frame: { width: frameRect.width, height: frameRect.height },
      svg: { width: svgRect.width, height: svgRect.height },
      canvas: { left: canvasRect.left, top: canvasRect.top, width: canvasRect.width, height: canvasRect.height },
      hit: { left: hitRect.left, top: hitRect.top, width: hitRect.width, height: hitRect.height },
      aligned: close(frameRect.width, svgRect.width) && close(frameRect.height, svgRect.height)
        && close(canvasRect.left, hitRect.left) && close(canvasRect.top, hitRect.top)
        && close(canvasRect.width, hitRect.width) && close(canvasRect.height, hitRect.height)
    };
  });
}
async function waitForChartGeometry() {
  await page.waitForFunction(() => {
    const frame = document.querySelector(".frame");
    const svg = frame?.querySelector("svg");
    const canvas = document.querySelector("canvas.dots");
    const hit = frame?.querySelector("rect.hit");
    if (!frame || !svg || !canvas || !hit || canvas.width < 1 || canvas.height < 1) return false;
    const frameRect = frame.getBoundingClientRect();
    const svgRect = svg.getBoundingClientRect();
    const canvasRect = canvas.getBoundingClientRect();
    const hitRect = hit.getBoundingClientRect();
    const close = (left, right) => Math.abs(left - right) <= 1;
    return close(frameRect.width, svgRect.width) && close(frameRect.height, svgRect.height)
      && close(canvasRect.left, hitRect.left) && close(canvasRect.top, hitRect.top)
      && close(canvasRect.width, hitRect.width) && close(canvasRect.height, hitRect.height);
  });
}
async function waitForSettledCanvas() {
  await page.waitForFunction(() => !document.body.textContent?.includes("Computing chart…"));
  await waitForChartGeometry();
  await page.waitForFunction(() => {
    const canvas = document.querySelector("canvas.dots");
    if (!canvas || canvas.width < 1 || canvas.height < 1) return false;
    const pixels = canvas.getContext("2d")?.getImageData(0, 0, canvas.width, canvas.height).data ?? [];
    return pixels.some((value, index) => index % 4 === 3 && value > 0);
  });
}
try {
  await page.route("**/api/**", async route => {
    const path = new URL(route.request().url()).pathname;
    let body;
    if (path === "/api/projects") body = [{ id: "test", name: "Scatter test", node_id: "node", source_count: 1 }];
    else if (path.endsWith("/sources")) body = [{ id: "source", name: "Points.csv" }];
    else if (path.endsWith("/sources/source")) body = { id: "source", name: "Points.csv", kind: "upload", project_id: "test", views: [view] };
    else if (path.endsWith("/views")) body = [view];
    else if (path.endsWith("/sql")) {
      const q = route.request().postDataJSON();
      body = { columns, rows, page: q.page, page_size: q.page_size, total_rows: 1, total_pages: 1, elapsed_ms: 1, sql: q.sql };
    } else if (path.endsWith("/visualize")) {
      const q = route.request().postDataJSON();
      requests.push(q);
      if (process.env.SCATTER_ERROR && q.spec.chart === "scatter") {
        await route.fulfill({ status: 503, json: { detail: "Scatter request failed" } }).catch(() => {});
        return;
      }
      body = q.spec.chart === "scatter"
        ? { chart: "scatter", points, total_points: points.length, elapsed_ms: 1 }
        : { chart: "histogram", bins: [{ lower: 1, upper: 2, count: 2 }], elapsed_ms: 1 };
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
  await waitFor(() => requests.some(q => q.spec.chart === "scatter"));
  if (process.env.SCATTER_ERROR) {
    await page.getByText("Chart unavailable", { exact: true }).waitFor();
    assert.equal(await page.getByText("Computing chart…", { exact: true }).count(), 0, "scatter errors must leave Computing");
    assert.equal(await page.locator("canvas.dots").count(), 0, "failed scatter must not render a canvas");
    assert.deepEqual(errors, [], "scatter errors must render without runtime errors");
    console.log("PASS scatter error leaves Computing and reports failure");
  } else if (process.env.SCATTER_EMPTY) {
    await page.getByText("No values to chart.", { exact: true }).waitFor();
    assert.equal(await page.getByText("Computing chart…", { exact: true }).count(), 0, "empty scatter must leave Computing");
    assert.equal(await page.locator("canvas.dots").count(), 0, "empty scatter must not render a canvas");
    assert.deepEqual(errors, [], "empty scatter must render without runtime errors");
    console.log("PASS empty scatter leaves Computing and renders its empty state");
  } else {
    await waitForSettledCanvas();
    const initialCanvas = await canvasState();
    const initialGeometry = await chartGeometry();
    assert.ok(initialCanvas.width > 0 && initialCanvas.height > 0, "scatter canvas must have a drawable size");
    assert.ok(initialCanvas.painted > 0, "scatter canvas must contain painted pixels");
    assert.ok(initialGeometry?.aligned, "scatter canvas must align with the plotted geometry");

    await page.locator("button.field").filter({ hasText: /^group/ }).click();
    await waitFor(() => requests.some(q => q.spec.chart === "scatter" && q.spec.encodings.color === "group"));
    await waitForSettledCanvas();
    await page.locator("button.field").filter({ hasText: /^group/ }).click();
    await page.locator("button.field").filter({ hasText: /^size/ }).click();
    await waitFor(() => requests.some(q => q.spec.chart === "scatter" && q.spec.encodings.size === "size"));
    await waitForSettledCanvas();

    const beforeResize = await canvasState();
    const beforeResizeGeometry = await chartGeometry();
    assert.ok(beforeResizeGeometry?.aligned, "scatter geometry must align before resize");
    await page.setViewportSize({ width: 1280, height: 700 });
    await page.waitForFunction((height) => {
      const canvas = document.querySelector("canvas.dots");
      if (!canvas || canvas.width < 1 || canvas.height < 1 || canvas.height === height) return false;
      const pixels = canvas.getContext("2d")?.getImageData(0, 0, canvas.width, canvas.height).data ?? [];
      return pixels.some((value, index) => index % 4 === 3 && value > 0);
    }, beforeResize.height);
    await waitForChartGeometry();
    const resizedCanvas = await canvasState();
    const resizedGeometry = await chartGeometry();
    assert.notEqual(resizedCanvas.height, beforeResize.height, "scatter canvas must follow chart resize");
    assert.ok(resizedCanvas.painted > 0, "resized scatter canvas must remain painted");
    assert.ok(resizedGeometry?.aligned, "resized scatter canvas must align with the plotted geometry");

    const hit = page.getByRole("application", { name: "Scatter plot. Drag to select a region. Double-click to reset zoom." });
    const box = await hit.boundingBox();
    assert.ok(box, "scatter plot must expose a selectable region");
    const ticks = () => page.locator('.frame .tick').allTextContents();
    const originalTicks = await ticks();
    const requestCount = requests.length;
    async function dragRegion() {
      await page.mouse.move(box.x + box.width * 0.25, box.y + box.height * 0.25);
      await page.mouse.down();
      await page.mouse.move(box.x + box.width * 0.6, box.y + box.height * 0.6, { steps: 4 });
      await page.mouse.up();
      await page.getByRole('button', { name: 'Zoom', exact: true }).waitFor();
    }
    await dragRegion();
    await page.getByRole('button', { name: 'Zoom', exact: true }).click();
    assert.notDeepEqual(await ticks(), originalTicks, 'zoom must change the axis range');
    await page.mouse.dblclick(box.x + box.width * 0.4, box.y + box.height * 0.4);
    assert.deepEqual(await ticks(), originalTicks, 'double-click must restore the full axis range');

    await dragRegion();
    await page.getByRole('button', { name: 'Zoom', exact: true }).click();
    await dragRegion();
    await page.mouse.dblclick(box.x + box.width * 0.4, box.y + box.height * 0.4);
    assert.deepEqual(await ticks(), originalTicks, 'double-click through a selection must reset zoom');
    assert.equal(await page.locator('rect.brush').count(), 0, 'reset must clear the selection');
    assert.equal(await page.locator('.choice').count(), 0, 'reset must dismiss selection actions');
    assert.equal(requests.length, requestCount, 'zoom and reset must not filter or request new data');

    await page.mouse.move(box.x + box.width * 0.1, box.y + box.height * 0.1);
    await page.mouse.down();
    await page.mouse.move(box.x + box.width * 0.9, box.y + box.height * 0.9, { steps: 4 });
    const brushStyle = await page.locator('rect.brush').evaluate((rect) => {
      const style = getComputedStyle(rect);
      return { fill: style.fill, stroke: style.stroke, pointerEvents: style.pointerEvents };
    });
    assert.equal(brushStyle.fill, 'none', 'drag rectangle must leave points unobscured');
    assert.notEqual(brushStyle.stroke, 'none', 'drag rectangle must retain its visible outline');
    assert.equal(brushStyle.pointerEvents, 'none', 'drag rectangle must not intercept plot input');
    await page.mouse.up();
    await page.getByRole("button", { name: "Filter", exact: true }).click();
    await waitFor(() => requests.some(q => q.spec.chart === "scatter" && q.filters?.length === 4));
    await waitForSettledCanvas();
    const filteredCanvas = await canvasState();
    assert.ok(filteredCanvas.painted > 0, "scatter must remain painted after region selection");
    console.log(JSON.stringify({ charts: requests.map(q => q.spec.chart), encodings: requests.filter(q => q.spec.chart === "scatter").map(q => q.spec.encodings), errors, computing: await page.getByText("Computing chart…", { exact: true }).count(), canvas: filteredCanvas }));
    assert.equal(await page.getByText("Computing chart…", { exact: true }).count(), 0, "scatter must leave Computing after the request resolves");
    assert.equal(await page.locator("canvas.dots").count(), 1, "scatter must render its canvas");
    assert.deepEqual(errors, [], "scatter must render without runtime errors");
    console.log("PASS scatter paints, resizes, filters, resets zoom on double-click, and keeps the brush transparent");
  }
} finally {
  await browser.close();
  await server.close();
}
