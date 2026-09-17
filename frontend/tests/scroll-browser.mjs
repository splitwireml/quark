// Run with npm run test:scroll-browser. Use PLAYWRIGHT_MODULE for a shared install.
// Every API is mocked: this exercises the real App without touching user data.
import assert from "node:assert/strict";
import { tableFromArrays, tableToIPC } from "apache-arrow";
import { createServer } from "vite";
const { chromium } = await import(process.env.PLAYWRIGHT_MODULE || "playwright");
const server = await createServer({ root: new URL("..", import.meta.url).pathname, server: { host: "127.0.0.1", port: 0 } });
await server.listen();
const browser = await chromium.launch({ headless: true });
const url = server.resolvedUrls.local[0];
const errors = [];

async function openGrid(total = 10000, delay = () => 100, fail = () => false, width = 1, format = "arrow") {
  const page = await browser.newPage({ viewport: { width: 1280, height: 900 } });
  const requests = [];
  page.on("pageerror", e => errors.push(e.message));
  const view = { id: "view", project_id: "test", source_id: "source", source_name: "Rows.csv", node_id: "node", name: "Rows", schema: "main", type: "VIEW", columns: ["id", ...Array.from({ length: width - 1 }, (_, i) => `column_${i}`)], sql: "SELECT * FROM rows" };
  await page.route("**/api/**", async route => {
    const path = new URL(route.request().url()).pathname;
    let body;
    if (path === "/api/projects") body = [{ id: "test", name: "Scroll test", node_id: "node", source_count: 1 }];
    else if (path.endsWith("/sources")) body = [{ id: "source", name: "Rows.csv" }];
    else if (path.endsWith("/sources/source")) body = { id: "source", name: "Rows.csv", kind: "upload", project_id: "test", views: [view] };
    else if (path.endsWith("/views")) body = [view];
    else if (path.endsWith("/sql")) {
      assert.equal(route.request().headers().accept, "application/vnd.apache.arrow.stream");
      const q = route.request().postDataJSON();
      requests.push(q);
      await new Promise(resolve => setTimeout(resolve, delay(q)));
      if (fail(q)) { await route.fulfill({ status: 503, json: { detail: "Temporary page failure" } }); return; }
      const start = (q.page - 1) * q.page_size;
      body = {
        columns: [{ name: "id", type: "BIGINT", numeric: true, null_fraction: 0, profile_kind: "numeric" }],
        rows: Array.from({ length: Math.max(0, Math.min(q.page_size, total - start)) }, (_, i) => ({ id: q.sorts.length ? total - 1 - start - i : start + i })),
        page: q.page, page_size: q.page_size, total_rows: total, total_pages: Math.ceil(total / q.page_size), elapsed_ms: 100, sql: q.sql,
      };
      for (const name of view.columns.slice(1)) {
        body.columns.push({ name, type: "VARCHAR", numeric: false, null_fraction: 0, profile_kind: "categorical" });
        for (const row of body.rows) row[name] = `value-${row.id}`;
      }
      if (format === "arrow") {
        const { rows, ...metadata } = body;
        const table = tableFromArrays(Object.fromEntries(view.columns.map(name => [name, rows.map(row => row[name])])));
        table.schema.metadata.set("quark", JSON.stringify(metadata));
        await route.fulfill({ contentType: "application/vnd.apache.arrow.stream", body: Buffer.from(tableToIPC(table)) }).catch(() => {});
        return;
      }
    } else throw new Error(`Unexpected API: ${path}`);
    await route.fulfill({ json: body }).catch(() => {}); // A canceled request can outlive its tab.
  });
  await page.goto(url);
  await page.getByRole("button", { name: "Scroll test 1 source" }).click();
  await page.getByRole("button", { name: /Rows.csv/ }).first().click();
  await page.locator("tbody td[data-column]").first().waitFor();
  await page.waitForTimeout(250);
  return { page, requests };
}
async function scroll(page, row, total = 10000) {
  await page.locator(".table-scroll").evaluate((el, { row, total }) => {
    const height = 34, viewport = el.clientHeight - 48;
    const step = (Math.min(10000000, total * height) - viewport) / (total - viewport / height);
    el.scrollTop = row * step;
  }, { row, total });
}
async function loaded(page, id) {
  await page.waitForFunction(id => [...document.querySelectorAll("tbody tr:not(.pending) td[data-column]")].some(el => Number(el.textContent) === id), id);
}
async function visibleRows(page) {
  return page.locator("tbody tr:not(.spacer)").evaluateAll(rows => {
    const host = document.querySelector(".table-scroll").getBoundingClientRect();
    return rows.filter(row => { const rect = row.getBoundingClientRect(); return rect.bottom > host.top + 48 && rect.top < host.bottom; }).map(row => ({ pending: row.classList.contains("pending"), value: Number(row.querySelector("td[data-column]")?.textContent) }));
  });
}
try {
  {
    const { page, requests } = await openGrid(10000, () => 100, () => false, 50);
    assert.equal(requests.length, 1, "idle opening must not prefetch");
    await page.context().grantPermissions(['clipboard-read', 'clipboard-write']);
    await page.locator('td[data-row="0"][data-column="id"]').click();
    await page.keyboard.press('Control+c');
    await page.waitForFunction(async () => await navigator.clipboard.readText() === '0');
    await page.keyboard.press('Enter');
    assert.equal(await page.getByRole('textbox', { name: 'Edit row 1, id', exact: true }).inputValue(), '0', 'editing must read an Arrow cell');
    await page.keyboard.press('Escape');
    const retainedCell = await page.locator('tr[aria-rowindex="12"] td[data-column="id"]').elementHandle();
    // A slow scan remains well before the critical point.
    for (let row = 1; row <= 12; row++) { await scroll(page, row); await page.waitForTimeout(180); }
    assert.equal(requests.length, 1, "slow scan must wait for the critical point");
    assert.ok(await retainedCell.evaluate(cell => cell.isConnected), "overlapping visible cells must survive scrolling");
    await scroll(page, 90);
    await loaded(page, 100);
    const promotedCell = await page.locator('tr[aria-rowindex="107"] td[data-column="id"]').elementHandle();
    await scroll(page, 110); await page.waitForTimeout(450);
    assert.ok(await promotedCell.evaluate(cell => cell.isConnected), "cache promotion must update cells without rebuilding them");
    const beforeReturn = requests.filter(q => q.page === 1).length;
    await scroll(page, 70); await loaded(page, 70); await page.waitForTimeout(250);
    assert.equal(requests.filter(q => q.page === 1).length, beforeReturn, "the previous page must remain cached");
    await page.locator("td[data-row][data-column]").first().click();
    await scroll(page, 1010); await loaded(page, 1010);
    assert.ok(await page.locator("tbody tr:not(.spacer)").count() < 60, "selection must not expand the virtual window");
    assert.ok((await visibleRows(page)).every(row => !row.pending), "settled viewport must be fully loaded");
    await scroll(page, 2010); await loaded(page, 2010);
    const oldRequests = requests.filter(q => q.page === 1).length;
    await scroll(page, 0); await loaded(page, 0);
    assert.equal(requests.filter(q => q.page === 1).length, oldRequests + 1, "older pages must be evicted");
    console.log("PASS slow threshold, page crossing, previous-page cache, eviction, selected-cell virtualization");
    await page.close();
  }
  {
    const { page, requests } = await openGrid(10000, q => q.page_size === 100 ? 200 : 300);
    const box = await page.getByRole("scrollbar").boundingBox();
    await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2); await page.mouse.down();
    await page.mouse.move(box.x + box.width / 2, box.y + 300, { steps: 5 });
    await page.waitForTimeout(30);
    assert.ok((await visibleRows(page)).length > 10, "moving thumb must retain visible rows");
    assert.equal(requests.length, 1, "moving held thumb must not fetch full pages");
    await page.waitForTimeout(170);
    assert.ok(requests.length > 1, "held preview must start within 200 ms of stopping");
    // A delayed settle/resize report must not cancel a preview for the same held position.
    await page.setViewportSize({ width: 1281, height: 900 });
    await page.waitForTimeout(400);
    assert.ok(requests.length > 1 && requests.slice(1).every(q => q.page_size < 100), "held rest must fetch only small snapshots");
    assert.ok((await visibleRows(page)).every(row => !row.pending), "snapshot must cover the entire viewport across slice boundaries");
    const heldHead = Number(await page.getByRole("scrollbar").getAttribute("aria-valuenow"));
    await page.mouse.up(); await page.waitForTimeout(500);
    assert.ok(requests.slice(1).some(q => q.page_size === 100), "release must fetch the landing page");
    assert.ok(Math.abs(Number(await page.getByRole("scrollbar").getAttribute("aria-valuenow")) - heldHead) <= 1, "snapshot swap must preserve scroll position");
    assert.ok((await visibleRows(page)).every(row => !row.pending));
    console.log("PASS moving thumb, held snapshot, full-page release, stable position");
    await page.close();
  }
  {
    const { page, requests } = await openGrid(10000000);
    await scroll(page, 5000050, 10000000); await loaded(page, 5000050);
    assert.ok(await page.locator("tbody tr:not(.spacer)").count() < 60, "ten million rows must still render only a viewport");
    assert.ok((await visibleRows(page)).every(row => !row.pending));
    const beforeWheel = Number(await page.getByRole("scrollbar").getAttribute("aria-valuenow"));
    await page.locator(".table-scroll").hover();
    await page.mouse.wheel(0, 120); await page.waitForTimeout(250);
    const afterWheel = Number(await page.getByRole("scrollbar").getAttribute("aria-valuenow"));
    assert.ok(afterWheel > beforeWheel && afterWheel - beforeWheel <= 5, "wheel speed must stay natural on compressed canvases");
    await page.getByRole("scrollbar").press("End");
    await loaded(page, 9999999);
    assert.ok((await visibleRows(page)).some(row => row.value === 9999999), "the final row must be reachable");
    assert.ok(requests.length < 12, "a distant jump must not load intermediate pages");
    console.log("PASS ten-million-row canvas, bounded DOM, final row, bounded requests");
    await page.close();
  }
  {
    const { page } = await openGrid(10000, q => q.page > 1 && !q.sorts.length ? 600 : 80);
    await scroll(page, 1050); await page.waitForTimeout(60);
    await page.getByRole("button", { name: "Sort id", exact: true }).click();
    await page.waitForTimeout(900);
    const values = await visibleRows(page);
    assert.ok(values.length > 10 && values.every(row => !row.pending && row.value > 9800), "late old-query data must not enter the sorted viewport");
    console.log("PASS query change cancels stale page responses");
    await page.close();
  }
  {
    let failPage = true;
    const { page, requests } = await openGrid(10000, () => 100, q => q.page === 11 && failPage);
    await scroll(page, 1050);
    await page.getByRole("alert").filter({ hasText: "Temporary page failure" }).waitFor();
    await page.waitForTimeout(300);
    assert.equal(requests.filter(q => q.page === 11).length, 1, "failed pages must not create a retry storm");
    assert.ok((await visibleRows(page)).length > 10, "failure must retain rows on screen");
    failPage = false;
    await page.getByRole("button", { name: "Retry", exact: true }).click();
    await loaded(page, 1050);
    await page.waitForTimeout(200);
    assert.ok((await visibleRows(page)).every(row => !row.pending));
    assert.equal(requests.filter(q => q.page === 11).length, 2, "explicit retry must bypass the cooldown");
    console.log("PASS page failure, retained rows, bounded retries, recovery");
    await page.close();
  }
  {
    const { page } = await openGrid(10000, () => 100, () => false, 1, "json");
    await page.locator('td[data-row="0"][data-column="id"]').click();
    for (let i = 0; i < 3; i++) {
      await page.keyboard.press("PageDown");
      await page.waitForTimeout(50);
    }
    const focused = await page.evaluate(() => Number(document.activeElement?.getAttribute("data-row")));
    assert.ok(focused > 30 && focused < 100, "keyboard navigation must reveal virtualized cells");
    assert.ok(await page.locator("tbody tr:not(.spacer)").count() < 60);
    console.log("PASS keyboard navigation through virtualized rows");
    await page.close();
  }
  assert.deepEqual(errors, [], "no browser runtime errors");
} finally {
  await browser.close();
  await server.close();
}
