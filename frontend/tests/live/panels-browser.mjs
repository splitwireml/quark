// Drives the stats panel, the category filter list and the cell finder in the real App against the Rust core.
import assert from "node:assert/strict";
import { runSteps, withLive } from "./harness.mjs";

const cities = ["Paris", "Berlin", "Paris", "Lisbon", "Madrid", "Paris", "Berlin", "Paris", "Lisbon", "Berlin", "Paris", "Lisbon"];
const amounts = [10, 20, 30, 40, 50, 60, 70, 80, "", "", 90, 100];
const csv = ["amount,city", ...cities.map((city, index) => `${amounts[index]},${city}`)].join("\n") + "\n";

await withLive({ name: "orders.csv", csv }, async ({ page, api, url }) => {
  const counts = text => text.replace(/,/g, "");
  const summary = async label => (await page.locator(`.inspector dl.summary div:has(dt:text-is("${label}")) dd`).innerText()).trim();
  const replay = request => {
    const target = new URL(request.url());
    return api.post(target.pathname + target.search, request.postDataJSON());
  };
  const total = async () => (await page.locator(".pagination .range").innerText()).match(/of ([\d,]+)/)[1].replace(/,/g, "");
  const closeInspector = async () => {
    await page.getByRole("button", { name: "Close inspector" }).first().click();
    await page.locator(".inspector").waitFor({ state: "detached" });
  };

  await page.goto(url);
  await page.getByRole("button", { name: /Live/ }).click();
  await page.locator("aside .row", { hasText: "orders.csv" }).click();
  await page.locator("aside button.row").nth(1).click();
  await page.locator('th[data-column="city"]').waitFor();
  assert.equal(await total(), String(cities.length), "the grid shows every fixture row");

  await runSteps({
    async stats_panel_matches_the_api() {
      const numeric = page.waitForRequest(request => request.url().endsWith("/columns/amount/stats"));
      await page.getByRole("button", { name: "Profile column amount" }).click();
      const expected = await replay(await numeric);
      await page.locator('.inspector dl.summary dt:text-is("Completeness")').waitFor();
      assert.equal(expected.kind, "numeric");
      assert.equal(expected.non_null_count, 10);
      assert.equal(expected.null_count, 2);
      assert.equal(counts(await summary("Completeness")), `${expected.non_null_count} non-null · ${expected.null_count} null`);
      assert.match(await summary("Range"), new RegExp(`^${expected.min}\\s+—\\s+${expected.max}$`));
      assert.equal(await page.locator(".histogram header span").innerText(), `${expected.histogram.length} bins`);
      await closeInspector();

      const text = page.waitForRequest(request => request.url().endsWith("/columns/city/stats"));
      await page.getByRole("button", { name: "Profile column city" }).click();
      const categories = await replay(await text);
      await page.locator('.inspector dl.summary dt:text-is("Distinct values")').waitFor();
      assert.equal(categories.kind, "categorical");
      assert.equal(categories.distinct_count, 4);
      assert.equal(counts(await summary("Distinct values")), String(categories.distinct_count));
      assert.equal(counts(await summary("Completeness")), `${categories.non_null_count} non-null · ${categories.null_count} null`);
      const rows = page.locator(".inspector .values > :not(header)");
      assert.equal(await rows.count(), categories.top_values.length);
      const top = categories.top_values[0];
      assert.equal(top.value, "Paris");
      assert.match(await rows.first().innerText(), new RegExp(`Paris\\s+${top.count}`));
      await closeInspector();
    },

    async filter_values_search_and_apply() {
      const values = page.waitForRequest(request => request.url().includes("/columns/city/values"));
      await page.getByRole("button", { name: "Filter city" }).click();
      await values;
      const rows = page.locator(".inspector .list .row");
      await rows.first().waitFor();
      assert.equal(await rows.count(), 4, "every distinct city is listed");
      assert.match(await page.locator(".inspector .page-row span").first().innerText(), /4 \/ 4/);

      const searched = page.waitForRequest(request => request.url().includes("search=lis"));
      await page.getByLabel("Find a category").fill("lis");
      const response = await replay(await searched);
      assert.deepEqual(response.values.map(item => item.value), ["Lisbon"]);
      await page.waitForFunction(() => document.querySelectorAll(".inspector .list .row").length === 1);
      assert.match(await rows.first().innerText(), /Lisbon\s+3/);

      await rows.first().click();
      assert.match(await page.locator(".inspector .actions span").innerText(), /^1 selected/);
      await page.getByRole("button", { name: "Apply categories" }).click();
      await page.locator(".inspector").waitFor({ state: "detached" });
      await page.waitForFunction(() => /of 3$/.test(document.querySelector(".pagination .range")?.textContent ?? ""));
      assert.equal(await total(), "3", "the grid keeps only the Lisbon rows");
      const shown = (await page.locator('td[data-column="city"][data-row]').allInnerTexts()).map(text => text.trim());
      assert.deepEqual(shown, ["Lisbon", "Lisbon", "Lisbon"], "only Lisbon rows remain");
    },

    async find_next_previous_and_wrap() {
      await page.getByRole("button", { name: "Clear conditions" }).click();
      await page.waitForFunction(total => new RegExp(`of ${total}$`).test(document.querySelector(".pagination .range")?.textContent ?? ""), cities.length);
      await page.locator("body").click({ position: { x: 5, y: 5 } });
      await page.keyboard.press("Control+f");
      await page.keyboard.press("f");
      const finder = page.locator(".finder input");
      await finder.waitFor();
      await finder.fill("Berlin");
      const status = page.locator(".finder [role=status]");
      const expectRow = async (row, how) => {
        await page.waitForFunction(text => document.querySelector(".finder [role=status]")?.textContent === text, `Row ${row} · city`);
        assert.equal(await status.innerText(), `Row ${row} · city`, how);
      };
      const next = page.locator(".finder").getByRole("button", { name: "Next", exact: true });
      const previous = page.locator(".finder").getByRole("button", { name: "Previous", exact: true });
      await next.click(); await expectRow(2, "first match");
      await next.click(); await expectRow(7, "next match");
      await next.click(); await expectRow(10, "last match");
      await next.click(); await expectRow(2, "next wraps to the first match");
      await previous.click(); await expectRow(10, "previous wraps to the last match");
      await previous.click(); await expectRow(7, "previous steps back");
      await finder.fill("Atlantis");
      await next.click();
      await page.waitForFunction(() => document.querySelector(".finder [role=status]")?.textContent === "No matching values in this View.");
    }
  });
});
